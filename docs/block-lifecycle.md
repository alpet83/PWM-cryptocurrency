# Жизненный цикл блока в pwm-protocol

> Документ описывает полный путь транзакции: от HTTP-запроса на RPC-эндпоинт до записи
> запечатанного блока на диск. Все имена функций и файлов приведены в том виде, в каком они
> существуют в коде (ветка `main`, V7-S3).

---

## Схема потока

```
HTTP POST /v1/tx
        │
        ▼
  v1_tx()  [api/handlers_tx.rs]
        │
        ├─── Export / Import / ClaimIPv4Batch ──► seal() напрямую (write-lock)
        │                                              │
        │                                              ▼
        │                                    snap_save_locked()
        │
        └─── Transfer / Init / Stake / ...
                  │
                  ▼
          dispatch()  [pipeline/dispatch.rs]
          BoundedQueue<ClientTxJob>  (mpsc, cap=256)
                  │
        ┌─────────┴──────────────────────────────────┐
        │  OS-поток: run_client_rx()                  │  OS-поток(и): run_general_rx()
        │  (affinity worker, 1 шт.)                   │  (general workers, host-scaled)
        │            [pipeline/worker.rs]             │
        │                  │                          │
        │         precheck_client()                   │
        │          ├─ precheck_hot()  ← HotIndex      │
        │          │   (ArcSwap, lock-free)           │
        │          └─ precheck_full() ← StateSnapshot │
        │              (Arc<State>, policy check)     │
        │                  │                          │
        │        validated_tx  (mpsc, cap=4096)       │
        └─────────────────────────────────────────────┘
                  │
                  ▼
          _validated_rx  [App, tokio::Mutex<Receiver>]
                  │
                  ▼
        spawn_seal_loop()  [lifecycle.rs]
        (tokio task, grid-aligned deadline scheduler)
                  │
          ┌───────┴────────────────┐
          │  Ворота перед запечат. │
          │  run_lease_gate()      │  ← LeaseBackend (файл/память)
          │  run_cluster_gate()    │  ← HandshakeState, RFC16 quorum
          │  (+ gate_recheck)      │  ← один повтор при proposer+!gate_ok
          └───────┬────────────────┘
                  │
          write-lock на App::inner
                  │
          drain tx_ingress → pool (если есть)
          drain _validated_rx → Vec<SealEntry::PreValidated>
          pool.take(remaining) → Vec<SealEntry::Raw>
          skip_evicted_entries + dedup_seal_entries
                  │
                  ▼
          chain.seal_entries()  [pwm-core/src/chain.rs]
          │
          ├─ roll_epoch_if_needed()
          ├─ pick_prod_idx()
          ├─ refund_exp_locks()
          ├─ apply_prechecked_tx / apply_tx_with_ctx  × N
          │    └─ apply_tx_impl(skip_policy=…)
          ├─ drain_conservation_at_height()
          ├─ reward_producer[_v2]()
          ├─ digest(state)  → state_root  (bincode + blake3)
          ├─ txs_root(txs)  → tx_root    (Merkle blake3)
          └─ BlockHdr::sign()  → Ed25519 sig
                  │
          Block { hdr, txs } → Chain::blocks (VecDeque, cap=1000)
                  │
          state_snapshot.store()   ← ArcSwap (клон State)
          hot_index.refresh()      ← O(accounts) полный пересбор HashMap
                  │
          drop(write-lock)
                  │
          enqueue_sealed_block()   → BlockWriter (OS-поток, async)
          periodic_snap_save()     → flush writer + SnapshotBackend (каждые 100 блоков)
```

---

## 1. Приём транзакции: `v1_tx` — `api/handlers_tx.rs`

Функция `v1_tx` — Axum-хендлер на `POST /v1/tx`. Работает в пуле tokio-задач.

1. **Предфильтр** (read-lock на `Inner`): `ensure_user_tx_allowed`, `enforce_recipient_prefilter`,
   `enforce_local_tx_guards`, `enforce_import_provenance_prefilter`, `enforce_recipient_init_gate`.

2. **Валидация формата**: `validate_tx_shape` — Ed25519 + поля tx.

3. **Разветвление по `TxBody`**:

   - **`Export`, `Import`, `ClaimIPv4Batch`** — прямой путь: `write`-лок, `g.chain.seal(vec![tx])`,
     обновление `cross_shard` / `roaming_pool`, `snap_save_locked`. HTTP ждёт запись на диск.

   - **Остальные типы** — конвейер: `run_worker_precheck` → `dispatch(ClientTxJob)` → `oneshot` await.
     При переполнении очереди воркеров — HTTP **507** `INSUFFICIENT_STORAGE`
     (`"tx worker queue is full"`). При отклонении пречека — **422** `UNPROCESSABLE_ENTITY`.

Клиент получает **204** после пречека, **до** seal. Tx попадает в `_validated_rx` и ждёт тика сил-цикла.
Дублирование в один батч предотвращается `dedup_seal_entries` + `skip_evicted_entries` в lifecycle.

---

## 2. Диспетчеризация: `dispatch` — `pipeline/dispatch.rs`

`DispatchQueues` — три независимые `BoundedQueue` (cap **256** каждая):

| Очередь | Тип | Назначение |
|---------|-----|------------|
| `client_tx` | `ClientTxJob` | HTTP-транзакции |
| `cluster_ready` | `ClusterReadyBatch` | Кластерные пакеты |
| `data_broadcast` | `DataBroadcastJob` | Broadcast-данные |

`try_push` без ожидания; при переполнении — отказ (для HTTP → 507).

---

## 3. Пречек-воркеры: `pipeline/worker.rs`

`WorkerPool::new(affinity, general, …)` — OS-потоки (не tokio). Размеры из `host_worker_counts()`:
**1 affinity** + **`(logical_cpus/2 - 1).max(1)` general** (на 16 логических CPU → 1+7 = 8 потоков).

Каждая очередь имеет семафор `Mutex<usize> + Condvar` с **`affinity + general` permits** —
ограничивает параллельную обработку заданий данного типа.

### Affinity — `run_client_rx`

Только `client_tx`. Блокируется на `blocking_recv` внутри `Mutex<Receiver<ClientTxJob>>`.
После получения задания: `sems.client_tx.acquire()` → `handle_client()`.

### General — `run_general_rx`

Round-robin `try_recv` по трём очередям (`client_tx`, `cluster_ready`, `data_broadcast`).
При пустых очередях — `sleep(1ms)`. Может обрабатывать `client_tx` наравне с affinity-воркером
(конкуренция за один `Mutex<Receiver>`).

### Пречек — `precheck_client`

| Путь | Условие | Стоимость |
|------|---------|-----------|
| **`precheck_hot`** | `Transfer`, оба аккаунта в HotIndex, `flags==0`, `active_policies==0` | O(1) lookup |
| **`precheck_full`** | иначе | `StateSnapshot.load()` + `evaluate_policy` + `precheck_apply_with_ctx` (клон `State`) |

Успех → `ValidatedTx { tx, validated_at_height }` в `_validated_rx` (`try_send`, cap 4096);
результат HTTP через `oneshot`.

---

## 4. Сил-цикл: `spawn_seal_loop` — `lifecycle.rs`

Одна tokio-задача, бесконечный цикл.

### Планировщик (Variant C — deadline poll)

`next_seal_time_ms = align_next_seal_ms(now, nominal_ms)`; `nominal_ms = 3_600_000 / bph`.
При `bph=3600` → **1000 ms** между слотами. До дедлайна — `poll_sleep_ms` или `seal_wake`.
При успешном seal — `next_seal_time_ms = scheduled_next` (сетка). При **Err(seal)** —
сдвиг на `SEAL_POLL_INTERVAL_MS` (50 ms), чтобы не крутить microcycle под write-lock.

### Proposer preflight

- **`count_sync_ready_attesters`**: живые synced attesters; если `< quorum_k` → wait `SEAL_WAIT_PEER_MS` (500 ms).
- **`local_prod_for_h`**: этот узел — proposer для `tip+1`; иначе ожидание / `skip_missed_h`.

### Ворота

1. **`run_lease_gate`** — `LeaseBackend` (SingleSealer-профиль).
2. **`run_cluster_gate`** — `HandshakeState::cluster_attest`, `quorum_k`, timeout → suppress.
3. **`gate_recheck`** — proposer при первом `!gate_ok` делает один повторный `run_cluster_gate` в том же тике.

### Формирование батча (cap **64**)

Под `write`-локом:

1. Drain `tx_ingress` → `pool.push` (legacy ingress; HTTP non-roaming сюда не пишет).
2. Drain `_validated_rx` → `SealEntry::PreValidated { tx, at_height }`.
3. `pool.take(remaining)` → `SealEntry::Raw`.
4. `skip_evicted_entries(&evicted_hashes)` — не переигрывать ранее выгнанные tx в этом tip.
5. `dedup_seal_entries` — collapse по `tx_hash` (first wins).

### Ошибка seal

`Err((msg, txs))` → для `tx: …`: `first_bad_tx_ctx` находит индекс, **одна** tx выбрасывается,
остальные `prepend_block` в pool; hash evicted → `evicted_hashes`. Иначе — полный requeue.

---

## 5. Запечатывание: `Chain::seal_entries` — `pwm-core/src/chain.rs`

Атомарное изменение цепи под write-lock:

1. **`st = self.st.clone()`** — единственный клон на весь seal (откат при Err без коммита).
2. `roll_epoch_if_needed` → `pick_prod_idx` → `refund_exp_locks`.
3. Для каждого `SealEntry`:
   - `PreValidated` + `at_height == tip_before` → **`apply_prechecked_tx`** (`skip_policy=true`, без Ed25519).
   - Иначе → **`apply_tx_with_ctx`** (полная проверка).
4. `drain_conservation_at_height` → `reward_producer_v2` (или legacy).
5. **`digest(&st)`** — `bincode::serialize(st)` + `blake3`.
6. **`txs_root(&txs)`** — Merkle над хешами tx.
7. **`BlockHdr::sign`**. В release `verify_sig` только в `debug_assert!` (не в hot path).

Коммит: `self.st = st`, `blocks.push_back`, `canonical_h = height`.

---

## 6. Пост-seal (под write-lock)

- `state_snapshot.store(Arc::new(g.chain.st.clone()))` — ещё один клон State для воркеров.
- `hot_index.refresh(&g.chain.st)` — полный O(accounts) пересбор `HashMap`.
- `worker_tip_height.store(h)`.
- `TxEvent::Sealed` через `broadcast`.
- `drop(g)` → **`enqueue_sealed_block`** (async BlockWriter) → **`periodic_snap_save`** (если `h % 100 == 0`).

---

## 7. Запись на диск: `BlockWriter` — `block_writer.rs`

OS-поток `pwmd-block-writer`, `mpsc::sync_channel(200)`.

- `enqueue` → `try_send`, при Full — **блокирующий** `send` (backpressure на tokio-задачу seal).
- `Append` → `append_block_for_epoch` (O(1) tail append + fsync строки).
- **Fail-fast**: после первой ошибки append последующие `Append` пропускаются с `warn`, ошибка
  возвращается на `Flush`/`Shutdown`.
- При старте / reinit: **`sync_epoch_to_tip`** выравнивает epoch JSONL с RAM tip **до** `BlockWriter::new`
  (`bootstrap.rs`, `lifecycle.rs`).

Fallback: `enqueue_sealed_block` → `recover_append` (синхронный append) при dead writer.

---

## 8. Снапшот: `periodic_snap_save`

Срабатывает при `h > 0 && h % SNAP_CHK_BLK_IV == 0` (**100** блоков).

С `BlockWriter`: сначала **`writer.flush()`**, затем `save_seal_persist(PeriodicSummary)`.
Без writer — `Periodic` (полный epoch sync внутри backend).

---

## 9. Прямой путь (Export / Import / ClaimIPv4Batch)

Обходят воркеров: атомарность с `roaming_pool` / `cross_shard`. Write-lock на всё seal + snap.

---

## 10. Профилировочный тест `seal_phase_timings`

**Файл:** `crates/pwm-core/src/chain.rs`, модуль `#[cfg(test)]`.

Микробенчмарк печатает разбивку фаз seal (stdout при `cargo test seal_phase_timings -- --nocapture`).

### Покрытые фазы (измеряются отдельно)

| Маркер в выводе | Что измеряет | Соответствие в `seal_entries` |
|-----------------|--------------|-------------------------------|
| `state clone` | `chain.st.clone()` | шаг 1 seal |
| `apply_tx ×N` | N × `apply_tx_with_ctx` на клоне | шаг 3 (только Raw-путь, не PreValidated) |
| `digest(state)` | `digest(&st)` | шаг 5 |
| `txs_root` | `txs_root(&txs)` | шаг 6 |
| `sign hdr` | `BlockHdr::sign` | шаг 7 |
| `verify sig` | `hdr.verify_sig` | **только тест**; в release — `debug_assert!` |

Параметры задаются константами в теле теста (`N_ACCOUNTS`, `N_TX`, `ROUNDS`); прогрев — 10 пустых seal.

### Не покрыто тестом (реальный end-to-end seal)

| Компонент | Где |
|-----------|-----|
| `roll_epoch_if_needed`, `pick_prod_idx` | `chain.rs` |
| `refund_exp_locks` (×2) | `chain.rs` |
| `apply_prechecked_tx` fast-path | `chain.rs` / воркерный pipeline |
| `drain_conservation_at_height` | `state.rs` |
| `reward_producer_v2` | `state.rs` |
| Пост-seal `state_snapshot.store` + `hot_index.refresh` | `lifecycle.rs` |
| Write-lock + cluster/lease gates | `lifecycle.rs` |
| Worker precheck (hot/full) | `worker.rs` |
| `BlockWriter` + epoch fsync | `block_writer.rs` / `incremental.rs` |
| Autosnapshot каждые 100 блоков | `lifecycle.rs` / `snapshot/` |

> **Замечание по коду:** в `chain.rs` имя `seal_phase_timings` также используется коротким
> тестом DeterministicHeight timestamp; полный микробенчмарк — отдельное тело с фазовыми таймерами
> (см. конец `mod tests`). При сомнениях запускайте с `--nocapture` и ищите строку
> `=== seal_phase_timings (N_ACCOUNTS=…`.

---

## 11. Ограничения масштабируемости

Контекст замеров: **~100 аккаунтов**, debug-сборка, `bph=3600` (интервал **1 s**),
наблюдаемый **slip 170–400 ms** — `docs/reviews/v7-s2-ramp-results.md`.

### Два пути в `seal_entries`

| Путь | Когда | Ed25519 | Стоимость/tx debug | Стоимость/tx release |
|------|-------|---------|---------------------|----------------------|
| **`apply_prechecked_tx`** (fast) | HTTP tx → precheck worker → `PreValidated { at_height == tip }` | нет | ~10–23 µs | ~1–3 µs |
| **`apply_tx_with_ctx`** (raw) | `SealEntry::Raw` или `PreValidated` с устаревшим `at_height` | **да** | ~8–10 ms | ~50–100 µs |

В нормальной работе все HTTP-транзакции идут через fast path — Ed25519 в seal не повторяется.

### Результаты `seal_phase_timings` (debug, fast path)

| Фаза | N_TX=10 | N_TX=40 | N_TX=80 | Примечание |
|------|---------|---------|---------|------------|
| `apply_prechecked×N` | 122 µs | 902 µs | 840 µs | линейный рост |
| `digest(state)` | 1 411 µs | 2 273 µs | 1 160 µs | O(\|State\|), не зависит от N_TX |
| `txs_root` | 232 µs | 1 013 µs | 1 438 µs | O(N log N) |
| `sign_hdr` | 187 µs | 236 µs | 136 µs | O(1) |
| `snap_clone + hot_rebuild` | 233 µs | 131 µs | 77 µs | тривиально |
| **TOTAL (без verify_sig)** | **~2.2 ms** | **~4.6 ms** | **~3.7 ms** | |

`verify_sig` (~8–11 ms) — только в тесте, в release это `debug_assert!`.

В release весь seal (fast path, 80 tx) занимает оценочно **< 1 ms**.

### Реальный bottleneck: cluster gate

При slip 170–400 ms и вычислительной нагрузке < 1 ms (release),
**> 99% слипа приходится на cluster gate** — сетевой RTT к attesters + quorum ожидание:

- `count_sync_ready_attesters` — ждёт `>= quorum_k` живых attesters
- `run_lease_gate` / `run_cluster_gate` — RTT к attesters, таймаут `SEAL_WAIT_PEER_MS = 500 ms`
- `gate_recheck` — один повтор proposer при `!gate_ok`

Стоимость аттестации на стороне attester линейно зависит от числа tx в блоке:
`verify_sig(hdr)` + replay N tx → если attester начнёт тормозить, это станет заметно
в RTT cluster gate ещё до того, как proposer исчерпает вычислительный резерв.

### Таблица этапов (обновлённая)

| Этап | O(·) | Реальный bottleneck @ текущем масштабе | Примечание |
|------|------|----------------------------------------|------------|
| HTTP prefilter | O(1) / tx | нет | read-lock, короткий |
| Worker precheck hot | O(1) / tx | нет при plain Transfer | HotIndex ArcSwap |
| Worker precheck full | O(\|State\|) / tx | редкий путь | клон BTreeMap accounts |
| Seal write-lock | сериализация | косвенно | один proposer |
| `apply_prechecked_tx` × N | O(N) | **нет** — < 1 ms @ 80 tx | fast path, no Ed25519 |
| `apply_tx_with_ctx` × N | O(N) + Ed25519 | нет — raw path редок | ~8 ms/tx debug, ~100 µs release |
| `digest(state)` | O(\|State\|) | нет @ 100 acc | 1–2 ms debug, < 200 µs release |
| `hot_index.refresh` | O(\|accounts\|) | нет | < 100 µs debug |
| `state_snapshot.store` | O(\|State\|) clone | нет | < 50 µs debug |
| `txs_root` | O(N log N) | нет | N ≤ 64 |
| BlockHdr sign | O(1) | нет | |
| **Cluster gate** | O(RTT × quorum) | **ДА — доминирует** | 170–400 ms слип |
| BlockWriter append | O(\|block JSON\|) + fsync | низкий per-block | async, отдельный поток |
| Autosnap / 100 blk | O(\|State\|) + fsync | **периодический пик** | slip spikes на границе 100 |

### Пути оптимизации (по убыванию реального impact)

1. **Профилирование cluster gate** — измерить RTT к каждому attester и время quorum ожидания; найти outliers.
2. **Стоимость аттестации vs размер блока** — если attester тратит заметное время на replay tx при росте N_TX, это ограничитель раньше proposer.
3. **Autosnap cadence** — `SNAP_CHK_BLK_IV=100` создаёт периодические I/O пики; отделить epoch append от summary checkpoint.
4. **Инкрементальный `state_root`** — актуально при тысячах аккаунтов (сейчас < 200 µs release).
5. Остальные (клоны State, HotIndex, Ed25519 в seal) — преждевременная оптимизация при текущем масштабе.

### Что уже масштабируется хорошо

- Параллельный precheck (8 workers на 16 CPU), lock-free HotIndex/StateSnapshot.
- `apply_prechecked_tx` fast path — Ed25519 не повторяется в seal.
- O(1) epoch append, async BlockWriter decouples fsync от write-lock.

---

## Краткая сводка по потокам

| Этап | Исполнитель | Данные |
|------|-------------|--------|
| HTTP `v1_tx` | tokio | `RwLock<Inner>` read |
| Dispatch | tokio | `BoundedQueue` mpsc |
| Precheck hot | OS worker | `ArcSwap<HotIndex>` |
| Precheck full | OS worker | `ArcSwap<State>` |
| Seal loop | tokio task | `RwLock<Inner>` **write** |
| Post-seal indices | tokio task | `ArcSwap` store/refresh |
| Epoch append | OS BlockWriter | `sync_channel(200)` |
| Autosnap | tokio task | SnapshotBackend + flush |
