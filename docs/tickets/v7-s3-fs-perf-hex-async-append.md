---
ticket: v7-s3-fs-perf-hex-async-append
priority: high
sprint: V7-S3
status: open
created: 2026-06-27
---

# V7-S3: Оптимизация записи блоков — hex encoding + O(1) append + async queue

## Мотивация

Бенчмарк после rotation-fix: level=68 ok=68 fail=0 slip=1256ms, stop=block_dt_overrun.
Две независимые причины slip:

1. **Вычислительная (постоянная):** `Chain::seal` × 68 `apply_tx_with_ctx` = ~1256ms под write lock.
   Debug build, release даст -10..15%. Не предмет этого тикета.

2. **I/O (периодическая, каждые 100 блоков):** `append_block_for_epoch` читает весь
   epoch файл (~750KB+), перестраивает в памяти, пишет всё + `sync_all()`.
   При 100-блочном flush: O(N²) суммарная запись. Плюс сами данные раздуты:
   `sig [42,58,15,...]` = 297 байт вместо 130 байт hex.

### Замеры (block_e300.json, 700 блоков):

| Поле | Тип | Текущий JSON | Hex JSON | Экономия |
|------|-----|-------------|---------|---------|
| `tx.signature` | [u8;64] | 297 B | 130 B | 56% |
| `tx.signer_pk` | [u8;32] | 142 B | 66 B | 53% |
| `tx.body.*.to` (AccountId) | [u8;32] | 139 B | 66 B | 52% |
| `hdr.sig` | [u8;64] | 287 B | 130 B | 54% |
| **Итого на tx** | | **733 B** | **~417 B** | **~43%** |

При 68 tx/block: **~21 KB экономии на блок**. Epoch файл уменьшится вдвое →
autosnap I/O в 2 раза быстрее.

---

## Задача 1 — Hex encoding для бинарных полей (pwm-core)

### Что менять

**`crates/pwm-core/src/ser_bin.rs`** — исправить `sig64`:

```rust
// БЫЛО: serialize_bytes → JSON [42, 58, ...]
pub mod sig64 {
    pub fn serialize<S>(bytes: &[u8; 64], ser: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        ser.serialize_bytes(bytes.as_slice())  // ← ПЛОХО
    }
    // deserialize принимает только Vec<u8> (массив)
}

// НАДО: как hex32, но для 64 байт
pub mod sig64 {
    pub fn serialize<S>(bytes: &[u8; 64], ser: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        if ser.is_human_readable() {
            ser.serialize_str(&hex::encode(bytes))
        } else {
            bytes.serialize(ser)
        }
    }

    // deserialize: принять hex-строку ИЛИ legacy массив байт
    pub fn deserialize<'de, D>(de: D) -> Result<[u8; 64], D::Error>
    where D: Deserializer<'de> {
        if de.is_human_readable() {
            de.deserialize_any(Sig64Visitor)
        } else {
            <[u8; 64]>::deserialize(de)
        }
    }
}
// Sig64Visitor: visit_str → parse hex, visit_seq → legacy byte array
```

**`crates/pwm-core/src/tx.rs`** — добавить `#[serde(with = ...)]` к полям:

```rust
// SignedTx:
pub signer_pk: [u8; 32],           // добавить: #[serde(with = "crate::ser_bin::hex32")]
#[serde(with = "crate::ser_bin::sig64")]
pub signature: [u8; 64],           // уже есть, но sig64 нужно поправить выше

// InitAccount:
pub company_metadata_commitment: [u8; 32],  // #[serde(with = "crate::ser_bin::hex32")]
pub rescue_address: Option<AccountId>,       // #[serde(with = "crate::ser_bin::opt_hex32")]

// TxBody enum variants:
Transfer { to: AccountId, ... }             // #[serde(with = "crate::ser_bin::hex32")]
Stake { beneficiary: Option<AccountId> }    // #[serde(with = "crate::ser_bin::opt_hex32")]
ClaimIpv4Batch { batch_root: [u8;32], registry_sig: [u8;64], to: AccountId }
Export { to: AccountId, export_id: [u8;32] }
ImportClaim { target_account: AccountId }
```

Нужно добавить `opt_hex32` модуль в `ser_bin.rs` для `Option<AccountId>`:
```rust
pub mod opt_hex32 {
    // serialize: None → null, Some(v) → hex string
    // deserialize: null → None, hex str → Some(...), legacy array → Some(...)
}
```

### Backward compatibility

Десериализация существующих epoch файлов (legacy массивы) должна работать.
`hex32` уже поддерживает `visit_seq` — проверить что `sig64` и новые поля тоже поддерживают.

Существующие epoch файлы не надо перемигрировать: при следующем autosnap checkpoint
ещё не-трогнутые блоки перечитываются через `sync_epoch_to_tip` → десериализуются
из старого формата → сохраняются в новом.

### Тесты

- `hdr_json_hex_str` в `block.rs` — уже проверяет hex для `hdr.*`. Добавить аналогичные для `sig64`.
- `hdr_json_legacy_arr` — проверяет backward compat. Расширить на `sig64`.
- Добавить тест `tx_json_hex_round_trip` и `tx_json_legacy_arr_compat` в `tx.rs`.

---

## Задача 2 — O(1) append в `append_block_for_epoch`

**Файл:** `crates/pwmd/src/snapshot/incremental.rs`, строки 19–90

### Проблема

Текущий алгоритм: читает весь файл → перестраивает в памяти → пишет весь файл.
O(file_size) на каждый append. При autosnap: O(N × file_size) за 100 итераций.

### Решение

```rust
pub(crate) fn append_block_for_epoch(summary_path: &Path, blk: &Block) -> Result<(), String> {
    let h = blk.hdr.height;
    let eidx = epoch_idx(h)?;
    let epoch_path = epoch_file_path(summary_path, eidx);
    let er = epoch_range(eidx);

    if let Some(dir) = epoch_path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("epochs mkdir: {e}"))?;
    }

    // Проверяем continuity: читаем только хвост файла вместо всего содержимого
    let prev_height = read_last_block_height(&epoch_path)?;
    match prev_height {
        None => {
            // Новый файл: height должен быть первым в epoch
            if h != er.first_h {
                return Err(format!("epoch append: first height must be {}, got {}", er.first_h, h));
            }
        }
        Some(last_h) => {
            if last_h != h - 1 {
                return Err(format!("epoch append: want prev {}, got {}", h - 1, last_h));
            }
        }
    }

    let line = serde_json::to_string(blk).map_err(|e| format!("encode block: {e}"))?;

    // Истинный append: открываем в режиме добавления, пишем одну строку
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&epoch_path)
        .map_err(|e| format!("epoch open append: {e}"))?;
    f.write_all(line.as_bytes()).map_err(|e| format!("epoch write: {e}"))?;
    f.write_all(b"\n").map_err(|e| format!("epoch write nl: {e}"))?;
    f.sync_all().map_err(|e| format!("epoch fsync: {e}"))?;

    // Обновляем manifest
    update_epoch_manifest(summary_path, eidx, h, &blk.hdr)?;

    Ok(())
}

/// Читает height последнего блока в файле, сканируя с конца.
/// O(last_line_size), не O(file_size).
fn read_last_block_height(path: &Path) -> Result<Option<u64>, String> {
    if !path.exists() {
        return Ok(None);
    }
    // Читаем последние N байт для поиска последней строки
    let mut f = fs::File::open(path).map_err(|e| format!("epoch open: {e}"))?;
    let file_len = f.metadata().map_err(|e| format!("epoch meta: {e}"))?.len();
    if file_len == 0 {
        return Ok(None);
    }
    // Последняя строка JSONL не длиннее ~100KB (даже для блока с 1000 tx)
    const TAIL_READ: u64 = 131_072;  // 128 KB
    let seek_pos = file_len.saturating_sub(TAIL_READ);
    use std::io::{Read, Seek, SeekFrom};
    f.seek(SeekFrom::Start(seek_pos)).map_err(|e| format!("epoch seek: {e}"))?;
    let mut buf = String::new();
    f.read_to_string(&mut buf).map_err(|e| format!("epoch read tail: {e}"))?;
    // Последняя непустая строка
    let last_line = buf.lines().rev().find(|l| !l.trim().is_empty()).ok_or("epoch: empty file")?;
    let blk: Block = serde_json::from_str(last_line)
        .map_err(|e| format!("epoch tail parse: {e}"))?;
    Ok(Some(blk.hdr.height))
}
```

Manifest обновляется отдельно — нет необходимости читать все предыдущие блоки.

### Влияние на производительность

Autosnap burst (100 блоков): вместо 100 × O(file_size) = ~75 MB read/write,
получаем 100 × O(one_line) = 100 × write(~500B) + 100 fsync.
При fsync ~5ms: 100 × 5ms = 500ms (было ~800ms, и без O(N²) роста).

---

## Задача 3 — Async write queue (decoupling I/O from seal loop)

После Задачи 2 autosnap burst всё ещё блокирует seal на ~500ms каждые 100 блоков.
Async queue выносит весь file I/O из critical path.

### Архитектура

```
seal loop (tokio)
  │
  ├── chain.seal_entries()        ← computation, нельзя убрать
  ├── hot_index.refresh()         ← fast ArcSwap, нельзя убрать
  │
  └── block_write_tx.try_send(Arc::clone(blk))  ← ~1µs, non-blocking
        │
        │  SyncChannel<Arc<Block>> capacity=200
        │
        └── [OS thread: block_writer]
              loop { rx.recv() → append_block_for_epoch() }
```

### Реализация

**`crates/pwmd/src/app.rs`** (или `bootstrap.rs`):
```rust
pub struct App {
    // ...
    /// None если snapshot backend отсутствует
    pub block_write_tx: Option<std::sync::mpsc::SyncSender<Arc<Block>>>,
}
```

**`crates/pwmd/src/bootstrap.rs`** — при инициализации:
```rust
fn spawn_block_writer(path: PathBuf) -> std::sync::mpsc::SyncSender<Arc<Block>> {
    let (tx, rx) = std::sync::mpsc::sync_channel::<Arc<Block>>(200);
    std::thread::spawn(move || {
        while let Ok(blk) = rx.recv() {
            if let Err(e) = append_block_for_epoch(&path, &blk) {
                error!("block_writer: {e}");
            }
        }
    });
    tx
}
```

**`crates/pwmd/src/lifecycle.rs`** — заменяем `periodic_snap_save` per-block вызов:
```rust
// После chain.seal_entries() успех:
if let Some(blk) = g.chain.blocks.back() {
    if let Some(ref tx) = app.block_write_tx {
        // Bounded channel: при заполнении (200 блоков) — fallback на sync write
        if tx.try_send(Arc::new(blk.clone())).is_err() {
            warn!("block_write queue full — writing synchronously");
            // sync fallback
        }
    }
}
// periodic_snap_save оставляем ТОЛЬКО для checkpoint summary (каждые 100 блоков)
// но убираем sync_epoch_to_tip из него — writer уже записал все блоки
```

### Важные инварианты

- Writer — единственный writer epoch файлов. Seal loop только отправляет блоки.
- При graceful shutdown: `drop(block_write_tx)` → writer drain `rx` → thread exit.
  Добавить `writer_thread.join()` в shutdown path.
- Autosnap checkpoint (каждые 100 блоков) должен дождаться flush writer'а перед
  сохранением summary. Варианты: flush-сигнал через отдельный channel, или просто
  убрать `sync_epoch_to_tip` из checkpoint (блоки уже записаны writer'ом).
- Если writer упал (паника/ошибка), seal loop продолжает — блоки не теряются
  из in-memory `chain.blocks`, recovery при следующем старте через `sync_epoch_to_tip`.

---

## Порядок реализации

1. **Hex encoding (Задача 1)** — изолированное изменение, только `ser_bin.rs` + `tx.rs`.
   Не затрагивает I/O path. Сначала тесты, потом код.

2. **O(1) append (Задача 2)** — рефакторинг `incremental.rs`. После Задачи 1 файлы
   станут вдвое меньше, что ускорит и старый и новый append.

3. **Async queue (Задача 3)** — после Задачи 2, чтобы не смешивать изменения.
   Требует аккуратного shutdown path.

## Ожидаемые результаты

| Метрика | До | После Задачи 1 | После Задачи 2 | После Задачи 3 |
|---------|-----|---------------|---------------|----------------|
| Epoch файл (1000 блоков при 68 tx) | ~15 MB | ~8.5 MB | ~8.5 MB | ~8.5 MB |
| I/O на autosnap (100 блоков) | ~800ms burst | ~450ms burst | ~100ms burst | ~0ms (async) |
| seal slip при level=68 | 1256ms | 1256ms | 1256ms (без burst) | 1256ms |
| Пиковый slip (autosnap hit) | ~2000ms | ~1700ms | ~1356ms | 1256ms |

Вычислительный bottleneck (apply_tx × 68 = ~1256ms) — следующий отдельный тикет.
