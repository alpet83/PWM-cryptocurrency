# Sprint 15 — ClickHouse snapshot storage: архитектура записи и масштабирование

**Scope:** Расширенный архитектурный анализ ClickHouse snapshot storage в `pwmd`: паттерн записи, оптимальность DDL, масштабирование при конкурентных записях от N нод одного шарда. Ревью-only; продакшн-код не правится.

---

## 1. Scope recap

Текущий CH-бэкенд (`crates/pwmd/src/snapshot/ch_http.rs`, feature `clickhouse-snapshot`) реализует персистенцию полного snapshot состояния ноды в ClickHouse через HTTP Interface. DDL:

```sql
CREATE TABLE IF NOT EXISTS pwm_snapshots.node_snapshot (
    row_key       String,
    inserted_at   DateTime64(3) DEFAULT now64(3),
    snapshot_json String
) ENGINE = MergeTree ORDER BY (row_key, inserted_at);
```

Предыдущий обзор (`sprint-15-ch-data-model-scaling-review.md`) зафиксировал проблему `row_key` (избыточность `cluster_id`/`node_id`) и рекомендовал сокращение до `{network_id}|0x{domain_hi}|{genesis_digest}`. Настоящий обзор фокусируется на **паттерне записи**, **структуре таблицы**, **семантическом несоответствии** и **масштабировании**.

---

## 2. Requirements fit

### A. Write frequency — КАЖДЫЙ БЛОК, не периодически

**Подтверждено из кода.** В `lifecycle.rs:164-175`, `seal_loop`:

```rust
if let Some(ref backend) = app.autosnapshot_backend {
    let periodic_hit = h > 0 && h % AUTOSNAPSHOT_BLOCK_INTERVAL == 0;
    if periodic_hit {
        info!("autosnapshot checkpoint hit ...");
    }
    save_result = Some((backend.init_state_path(), backend.save(&g)));
}
```

`periodic_hit` управляет **только** info-логированием. `backend.save(&g)` вызывается **безусловно** на каждый sealed block. `AUTOSNAPSHOT_BLOCK_INTERVAL = 100` — не интервал записи, а интервал лог-чекпоинта.

**Последствия при CH-бэкенде:**

- `ch_save` → `encode_inner_snap_json(inner)` сериализует **полный** `SnapshotData` (ВСЕ блоки + полное состояние + roaming + cross_shard) как pretty-printed JSON V2 wire.
- Каждый блок → один HTTP `INSERT` с полным JSON.
- Seal loop тикает каждые 2 секунды; при наличии tx → seal → write. Реалистичная частота: **1 write / 2-5s**.
- С ростом цепочки размер JSON растёт линейно: при 1000 блоков — ~5-15 MB JSON на каждый INSERT (зависит от tx density и числа аккаунтов).

**Оценка:** Данный паттерн **категорически не подходит** для MergeTree в продакшне. Это квадратичный рост хранилища — на высоте H записано H × (средний размер JSON при высоте до H) / 2 ≈ O(H²) суммарно.

### B. Blocking HTTP клиент в async tokio runtime

**Критическая проблема:** `ch_http.rs:18` использует `reqwest::blocking::Client`. Этот вызов происходит внутри `tokio::spawn` в `spawn_seal_loop`, то есть **блокирующий HTTP-запрос выполняется на tokio worker thread**. При таймауте 30s или сетевых задержках это может полностью заблокировать один из worker threads runtime'а, влияя на все async-задачи (RPC, transport, federation).

Текущий `JsonFile` бэкенд (`io::save_snapshot`) тоже блокирующий (sync fs), но файловая запись в tmpdir обычно субмиллисекундная. HTTP к внешнему ClickHouse — совсем другой порядок латентности.

### C. Monolithic blob — всё или ничего

`SnapshotData` в `types.rs:21-33`:
- `blocks: Vec<Block>` — **полная цепочка**, растёт неограниченно
- `state: ChainState` — полное состояние аккаунтов, fee_pool, marks_quota, imported_set, exported_registry
- `roaming: SnapshotRoamingWire`
- `cross_shard: CrossShardLedger`

Всё это сериализуется через `data_to_v2` → `serde_json::to_string_pretty` — pretty-print добавляет ~30-40% объёма по сравнению с compact JSON. Для CH, где `snapshot_json` — opaque String-столбец без колоночной индексации, pretty vs compact не даёт пользы, но увеличивает I/O и storage.

---

## 3. Анализ вопросов

### A. Write frequency impact (MergeTree pressure)

**Количественная модель** при block_time=2s, N=3 ноды, chain с 1000 блоков:

| Метрика | Значение |
|---------|----------|
| Writes/sec/node | 0.5 |
| Writes/sec/cluster | 1.5 |
| JSON size @ h=1000 (est.) | 5-15 MB |
| Raw INSERT throughput | 7.5-22.5 MB/s sustained |
| Parts created/min | ~90 (3 nodes × 30 writes/min) |
| MergeTree merge load | EXTREME — сотни мелких parts/min при одном ORDER BY |

MergeTree оптимизирован для batch INSERT (thousands/millions rows per INSERT, infrequent). Паттерн «1 row INSERT каждые 2 секунды» — worst case: каждый INSERT создаёт отдельный part; background merge thread постоянно работает, объединяя мелкие parts. При 90+ parts/min ClickHouse может отвечать `Too many parts (N). Merges are processing significantly slower than inserts` и отклонять INSERT.

**Storage growth** за сутки при h=1000→h=43200 (при 2s blocks):

- Каждый блок записывает весь JSON; JSON растёт от ~0.5 MB (h=1) до ~40 MB (h=43200)
- Суммарный объём за сутки ≈ Σ(json_size(h)) для h=1..43200 × N_nodes
- Грубая оценка: ~100-300 GB/day/node для зрелой цепочки. Это **неприемлемо**.

### B. Table-per-cluster vs shared table

| Критерий | Shared table (текущая) | Table-per-domain |
|----------|----------------------|------------------|
| Write isolation | Плохая: все домены в одном MergeTree, merge interference | Хорошая: изолированные part trees |
| DDL simplicity | Одна CREATE TABLE | Динамическая DDL: `CREATE TABLE ... cluster_snap__0x2c` при регистрации домена |
| Query patterns | WHERE row_key LIKE... или prefix scan | Прямой SELECT без фильтра |
| Part management | Один pool; больше parts, сложнее merge | Мелкие таблицы, быстрый merge |
| Operational burden | Минимальный | Нужен lifecycle: создание, мониторинг, cleanup по N таблиц |

**Рекомендация:** при текущей модели «один monolithic JSON per write» разница несущественна — проблема не в table layout, а в самом паттерне записи. Если перейти к нормальной append-only модели (см. раздел C), table-per-domain становится **предпочтительным**: explorer работает per-domain, merge изолирован, naming естественно: `cluster_blocks__0x2c` (если хранит блоки) или `cluster_snap__0x2c` (если checkpoints). **Формат имени:** `pwm_{network}__0x{domain_hi}` — database-level partitioning; или `pwm_blocks.domain_0x2c` — table-level в общей database. Для testnet допустима и shared table с partition key.

### C. Semantic mismatch: "snapshot" vs "block log"

Текущая семантика: **overwrite-style snapshot** — одна «актуальная» строка per row_key, `ch_load` берёт `ORDER BY inserted_at DESC LIMIT 1`. Но MergeTree **не удаляет** старые версии — они накапливаются.

Три модели:

| Модель | Описание | ClickHouse fit |
|--------|----------|----------------|
| **Current** (monolithic snapshot) | Полный JSON per block, overwrite semantics | ПЛОХО: O(H²) storage, opaque blob, no columnar benefit |
| **Append-only block log** | Одна строка per block (height, block_json или колонки) | ХОРОШО: O(H) storage, INSERT-optimized, columnar compress |
| **Checkpoint + WAL** | Periodic full snapshot + per-block deltas | ХОРОШО: O(H) storage, bounded checkpoint cost, rebuild flexibility |

**Рекомендуемая модель: Checkpoint + WAL (или чистый block log с materialized state).**

ClickHouse идеально подходит для append-only block log:
- Append-only INSERT — native paradigm
- Columnar compression на `height`, `prev_hash`, `state_root` — отличная компрессия
- `ORDER BY (domain_hi, height)` — мгновенные range-запросы
- State можно хранить отдельно как periodic checkpoint (каждые N=100-1000 блоков)

### D. Write contention between nodes of same shard

При N нод одного шарда, все пишут одинаковые данные:

- **MergeTree**: нет row-level locking, concurrent INSERT creates separate parts. Корректность сохраняется.
- **Проблема**: N × size I/O waste. 3 ноды × 10 MB JSON = 30 MB INSERT на один блок.
- **Решения (по возрастанию сложности):**

1. **Write-once-per-cluster (leader election)**: Один writer per shard-domain. Самое эффективное. Для testnet — `PWM_CH_SNAPSHOT_WRITER=true` на одной ноде.

2. **Conditional INSERT (IF NOT EXISTS)**: ClickHouse не имеет native `INSERT IF NOT EXISTS`, но `ReplacingMergeTree` + `FINAL` + dedup по `(row_key, height)` даёт eventual dedup.

3. **Idempotent upsert через ReplacingMergeTree**: при совпадающем ORDER BY key ClickHouse оставит одну версию при merge. Не гарантирует мгновенную dedup, но bounded storage.

### E. Data retention / cleanup

**Текущее состояние: нет retention policy.** Каждый INSERT добавляет строку навсегда.

**Рекомендации:**

1. **TTL clause**: `TTL inserted_at + INTERVAL 7 DAY` — автоматическая очистка старых версий snapshot.

2. **ReplacingMergeTree** (для checkpoint table): `ENGINE = ReplacingMergeTree(inserted_at) ORDER BY (row_key)` — при merge оставляет только последнюю версию. `SELECT ... FINAL` для guaranteed-latest при read.

3. **Partition by month/week**: `PARTITION BY toYYYYMM(inserted_at)` — быстрый `ALTER TABLE DROP PARTITION`.

### F. Proposed optimal architecture

#### 1. Two-table design

**Table 1: Block log (append-only, основная)**

```sql
CREATE TABLE pwm_blocks.domain_0x2c (
    height        UInt64,
    block_ts      DateTime64(3),
    prev_hash     FixedString(32),
    tx_root       FixedString(32),
    state_root    FixedString(32),
    prod_idx      UInt32,
    sig           FixedString(64),
    txs_json      String,
    writer_node   LowCardinality(String) DEFAULT '',
    inserted_at   DateTime64(3) DEFAULT now64(3)
) ENGINE = ReplacingMergeTree(inserted_at)
ORDER BY (height)
PARTITION BY intDiv(height, 100000);
```

- `ReplacingMergeTree` дедуплицирует записи от N нод по `height` (ORDER BY key)
- Columnar storage: `height`, `prev_hash`, `state_root` отлично сжимаются
- Append-only: один INSERT per block per node, O(1) payload size

**Table 2: State checkpoint (periodic)**

```sql
CREATE TABLE pwm_checkpoints.domain_0x2c (
    height          UInt64,
    genesis_digest  String,
    state_json      String,
    roaming_json    String DEFAULT '',
    cross_shard_json String DEFAULT '',
    inserted_at     DateTime64(3) DEFAULT now64(3)
) ENGINE = ReplacingMergeTree(inserted_at)
ORDER BY (genesis_digest, height)
TTL inserted_at + INTERVAL 30 DAY;
```

- Пишется **только** на checkpoint heights (каждые 100 блоков, `AUTOSNAPSHOT_BLOCK_INTERVAL`)
- `ReplacingMergeTree` дедуплицирует при multi-node writes
- TTL удаляет старые checkpoints автоматически
- При загрузке: берём последний checkpoint + replay блоков после него

#### 2. Table naming strategy

- **Database-per-network**: `pwm_testnet`, `pwm_mainnet`
- **Table-per-domain**: `blocks__0x2c`, `checkpoints__0x2c`
- **Полное имя**: `pwm_testnet.blocks__0x2c`
- **Fallback shared table** (для dev/test): `pwm_snapshots.blocks` с колонкой `domain_hi UInt8` в ORDER BY

#### 3. Engine choice

| Таблица | Engine | Обоснование |
|---------|--------|-------------|
| Block log | `ReplacingMergeTree(inserted_at)` | Dedup при multi-node writes по `height` |
| State checkpoint | `ReplacingMergeTree(inserted_at)` + TTL | Dedup + автоочистка старых |
| **Не использовать** | Plain `MergeTree` | Нет dedup → unbound growth |

#### 4. Write strategy

- **Немедленная (v1):** конфигурационный flag `PWM_CH_WRITER=1` — только одна нода в shard пишет. Остальные — читатели.
- **Целевая (v2):** write на каждой ноде, `ReplacingMergeTree` дедуплицирует. Проще для ops, чем leader election.
- **Checkpoint cadence:** `AUTOSNAPSHOT_BLOCK_INTERVAL` (100) — сохранить только для checkpoints, блоки пишутся every-block.
- **Block log write:** каждый sealed block INSERT-ит **только новый блок** (не всю цепочку).

#### 5. Retention policy

- Block log: **без TTL** (append-only, canonical record, storage = O(H))
- State checkpoints: **TTL 30 days** или `PARTITION BY toYYYYMM(...)` + manual DROP
- **Current monolithic table** (переходный период): `TTL inserted_at + INTERVAL 3 DAY`

#### 6. Migration path

**Phase 0 (hotfix):**
- Добавить `periodic_hit` guard на `backend.save()` — писать CH snapshot только каждые N блоков
- Заменить `reqwest::blocking` на async client (или `spawn_blocking`)
- Добавить TTL к текущей DDL

**Phase 1 (block log):**
- Новая DDL с block log table
- `ch_save` пишет только последний блок (height, txs, header fields)
- State checkpoint на `AUTOSNAPSHOT_BLOCK_INTERVAL`
- `ch_load` → load last checkpoint + replay blocks after it

**Phase 2 (multi-node dedup):**
- `ReplacingMergeTree` на обеих таблицах
- Убрать leader election requirement (если был в Phase 1)

**Phase 3 (table-per-domain):**
- Dynamic CREATE TABLE при первом write для нового domain
- Database-per-network

---

## 4. Safety

### 4.1. Blocking HTTP in async runtime (HIGH)

`reqwest::blocking::Client` вызывается внутри `tokio::spawn` (`lifecycle.rs:132`). Документация reqwest прямо запрещает использование `blocking` API внутри async runtime. При 30s timeout это может парализовать worker thread.

**Рекомендация:** использовать `reqwest::Client` (async) или обернуть в `tokio::task::spawn_blocking`.

### 4.2. Pretty-print JSON (LOW)

`serde_json::to_string_pretty` (`io.rs:204`) увеличивает payload на 30-40%. Для wire format к CH нет причин использовать pretty.

### 4.3. Unbounded chain in memory (MEDIUM)

`inner.chain.blocks.clone()` при каждом `ch_save` клонирует весь `Vec<Block>`. При 10,000 блоков — потенциально десятки MB аллокации + сериализации на каждый sealed block.

### 4.4. No retry / backpressure (MEDIUM)

`ch_insert_snapshot_json` делает один attempt. При сетевой ошибке — `Err`, переход в `ready_degraded`. Нет exponential backoff, нет circuit breaker.

### 4.5. SQL injection через database/table (LOW)

`snap_ch_sql_id` валидирует `[a-zA-Z0-9_]`, backtick-escaping в `ch_insert_snapshot_json`. Достаточно для текущего scope.

---

## 5. Tests

- **`autosnap_mod100_ok`** — проверяет только значение константы, не реальную cadence записи CH.
- **`ch_ping_env`** — smoke-test доступности CH, только при наличии env var.
- **`snap_ch_wire_jsonfile_mock`** — byte equality JsonFile vs CH load. Хороший тест, но не покрывает write contention.

**Отсутствует:**
- Тест на поведение при CH unavailability (retry, degraded state)
- Тест на concurrent writes от нескольких нод
- Benchmark: measure JSON encode time vs chain height

---

## 6. Verdict

**Request changes — HIGH priority.**

Текущий ClickHouse snapshot backend имеет фундаментальные архитектурные проблемы, делающие его непригодным для продакшна без существенных изменений:

| # | Severity | Issue |
|---|----------|-------|
| 1 | **CRITICAL** | O(H²×N) storage growth — monolithic blob per block, unbound accumulation |
| 2 | **HIGH** | Blocking HTTP client в async runtime — потенциальная деградация всего процесса |
| 3 | **HIGH** | Every-block write semantics — MergeTree part pressure, 90+ parts/min при N=3 |
| 4 | **MEDIUM** | Отсутствие retention/TTL — бесконечный рост без cleanup |
| 5 | **MEDIUM** | N×redundant writes при multi-node — нет dedup, нет leader election |
| 6 | **MEDIUM** | Pretty-print JSON — 30-40% overhead без пользы |
| 7 | **LOW** | Semantic mismatch naming (snapshot vs block log) |

**Минимальный immediate fix (Phase 0):** guard CH write за `periodic_hit` check, async HTTP client, TTL на DDL. Целевая архитектура — block log + state checkpoint (Phase 1-3).

---

## 7. Participation

```json
{
  "agent": "pwm-review",
  "result": "PARTIAL",
  "artifacts": "docs/reviews/sprint-15-ch-storage-architecture-review.md",
  "token_usage": {
    "source": "estimate",
    "input": 42000,
    "output": 6500,
    "total": 48500,
    "confidence": "medium"
  }
}
```
