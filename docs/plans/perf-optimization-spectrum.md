---
name: PWM Performance Optimization Spectrum
status: working-document
related:
  - docs/plans/mvp_v7.md
  - docs/plans/mvp_v7s1.md
  - docs/adr/0013-tx-pipeline-seda.md
---

# PWM: Спектр оптимизации производительности

Документ фиксирует три тира оптимизации для преодоления лимита ~3 tx/s и достижения цели ≥50 tx/s для публичного testnet. Тиры независимы и аддитивны: каждый следующий наслаивается поверх предыдущего.

---

## Тир 1: SEDA Pipeline (V7-S1 — текущий спринт)

**Цель:** ≥50 tx/s за счёт параллельной pre-processing до seal, без смены хранилища.

**Суть:** Вынести `validate_tx_shape`, `evaluate_policy` из критического пути `Chain::seal` в пул OS-потоков. Seal остаётся минимальным атомарным шагом.

**Потолок тира:** Если профилирование (Slice 0) покажет, что узкое место I/O-bound (запись снапшота, fsync), а не CPU — тир 1 может не дать ≥50 tx/s. В этом случае обязательна эскалация к оркестратору до перехода в Slice 1, и план корректируется.

**Нормативный документ:** `docs/adr/0013-tx-pipeline-seda.md`.

---

## Тир 2: ClickHouse как high-performance read/write backend (V7-4+)

### Зачем

Текущее файловое хранилище (снапшоты + блоки) имеет два ограничения для производительности:
1. **Write amplification при seal:** snapshot serialization → fsync блокирует seal даже при параллельной pre-processing.
2. **Read latency для RPC:** `/v1/account`, `/v1/balance` читают из in-memory State, которое при росте занимает больше RAM и медленнее обновляется.

ClickHouse уже частично в проекте (`clickhouse-snapshot` feature, `ch_snap_import.rs`, `tools/docker/sql/clickhouse_pwm_snapshots.sql`). Тир 2 переводит его из аналитического слоя в **primary read backend** и добавляет **write path** (INSERT батча после каждого seal).

### Архитектура тира 2

```
Chain::seal  →  [prepared_batch]  →  in-memory State (source of truth)
                                  →  INSERT в CH: accounts, txs, balances  (async, post-seal)

RPC read paths:
  /v1/account/:id    →  CH query (fast column scan по account_id)
  /v1/balance        →  CH query
  /v1/tx/:id         →  CH query
  Chain::seal input  →  ВСЕГДА из in-memory State (не из CH — для детерминизма)
```

**Инвариант:** CH — это **читаемый производный слой**, не источник истины для консенсуса. Все решения по `apply_tx`, `evaluate_policy`, `seal` принимаются только на основе in-memory State. CH может отставать на N блоков.

### Сертифицированная таблица состояния (Certified State Cache)

#### ShardStateCert — подпись на уровне шарды (per-block)

Нода подписывает агрегированное состояние шарды после каждого блока. Это дёшево: `state_root` уже вычисляется в `pwm_core::state::digest()` при каждом `seal` — одна подпись на блок, не на аккаунт.

```rust
#[derive(Debug, Clone)]
pub struct ShardStateCert {
    pub format_version: u8,
    pub shard_domain: u16,
    pub block_height: u64,
    pub account_count: u64,
    pub total_supply_raw: u128,
    pub state_root: [u8; 32],     // BLAKE3 digest State из BlockHdr
}
```

Подписывается каноническая бинарная структура (big-endian, без padding) ключом node validator (Ed25519). INSERT в CH таблицу `pwm_state_certs` — одна строка на блок.

#### Схема таблицы аккаунтов: привязка к блоку и снапшоту

Каждая строка в таблице состояния аккаунта явно указывает на контекст своей актуальности — без этого строка в CH не имеет доказуемого происхождения:

```sql
CREATE TABLE pwm_account_state (
    account_id       UInt64,
    shard_domain     UInt16,
    block_height     UInt64,          -- блок, в котором зафиксировано состояние
    bootstrap_snap_height UInt64,     -- высота последнего bootstrap snapshot шарды
    balance_raw      UInt128,
    nonce            UInt64,
    flags            UInt32,
    -- подпись присутствует только при крупных операциях (threshold в конфиге ноды)
    cert_signature   Nullable(FixedString(64)),  -- Ed25519, NULL если ниже порога
    node_pubkey      FixedString(32),
    updated_at       DateTime DEFAULT now()
) ENGINE = ReplacingMergeTree(block_height)
  ORDER BY (shard_domain, account_id);
```

`bootstrap_snap_height` позволяет клиенту или аудитору привязать состояние аккаунта к известному checkpoint шарды и к конкретному `ShardStateCert`.

#### Подпись на уровне аккаунта — только выше порога

Подписывать каждое изменение состояния аккаунта **нецелесообразно**: Ed25519 sign — это ~50–100 μs, при 50 tx/s и 50 аккаунтах это 2 500 подписей/сек только на запись. Для мелких переводов выгода не оправдывает стоимость.

**Правило:** `cert_signature` выставляется только при изменении баланса выше `account_cert_threshold_raw` (настройка конфига ноды, например 1 000 PWM).

**Модель угрозы, которую это закрывает:** злоумышленник, получивший доступ к БД, не сможет произвольно восстановить баланс аккаунта для сценария двойной траты — строка выше порога защищена подписью ноды, и подделать её без приватного ключа валидатора невозможно. Ниже порога риск двойной траты ограничен малой суммой.

**Что это НЕ закрывает:** верификация состояния клиентами и blockchain explorer'ами. Для этого нода подписывает ответ в реальном времени при выдаче — хранить подпись в БД для этой цели избыточно. `ShardStateCert` (уровень шарды) достаточен для light-client верификации.

#### JSON API

```json
GET /v1/shard/state-cert/latest
{
  "format_version": 1,
  "shard_domain": "0x0001",
  "block_height": 12345,
  "bootstrap_snap_height": 12000,
  "state_root": "a3f2...hex...",
  "node_pubkey": "ed25519:...",
  "signature": "hex..."
}
```

### Влияние CometBFT на тир 2

При принятии CometBFT (ADR V7-6) его storage (LevelDB/RocksDB) заменяет файловый blockchain, но **CH-слой остаётся** как:
- Read/analytics backend (ABCI `CommitInfo` → INSERT в CH).
- `ShardStateCert` подписывается тем же механизмом, но `state_root` берётся из `AppHash` ABCI response.
- CometBFT сам обеспечивает BFT-finality, поэтому `ShardStateCert` получает дополнительный `commit_hash` из CometBFT block header.

Таким образом, тир 2 совместим с обоими путями (PoA V7 и CometBFT post-V7).

---

## Тир 3: BFT + распределённое хранилище (post-V7 / Фаза 4)

**Предусловие:** ADR V7-6 принят, выбран путь (CometBFT / custom Rust BFT).

**Суть изменений:**
- `Chain::seal` → ABCI `DeliverTx` + `EndBlock` + `Commit`.
- Файловое хранилище блоков → LevelDB/RocksDB (через CometBFT) или эквивалент.
- In-memory State сохраняется как ABCI application state.
- CH-слой (тир 2) продолжает работать как read/analytics replica.

**Throughput ожидания при CometBFT:**
CometBFT на практике даёт 1 000–4 000 tx/s на одну ноду при лёгких `DeliverTx` (только verify + state mutation). С SEDA pipeline (тир 1) как pre-processing слоем перед ABCI — потолок выше.

**Открытые вопросы тира 3 (к ADR V7-6):**
- Как SEDA pipeline интегрируется с ABCI flow? `DeliverTx` должен быть синхронным для CometBFT — нужен ли pre-processing буфер перед ABCI?
- Как `ShardStateCert` (тир 2) соотносится с `AppHash` в CometBFT block header?
- Нужны ли cross-shard routing изменения при переходе на BFT?

---

## Матрица решений

| Критерий | Тир 1 (SEDA) | Тир 2 (CH backend) | Тир 3 (BFT) |
|----------|-------------|---------------------|-------------|
| Целевой throughput | ≥50 tx/s (single node) | ≥200 tx/s read; write — зависит от CH insert latency | ≥1000 tx/s (multi-node BFT) |
| Сложность | Средняя | Высокая (новый write path) | Очень высокая |
| Сохраняет `Chain::seal` | Да | Да (CH — дополнение) | Нет (заменяется ABCI) |
| Риск детерминизма | Высокий (новые потоки) | Низкий (CH не в консенсусе) | Средний (ABCI ordering) |
| Когда | V7-S1 | V7-4+ | Фаза 4 |
| Зависит от | Slice 0 profiling | Тир 1 стабилен | ADR V7-6 Accepted |

---

## Если тир 1 не даёт ≥50 tx/s (эскалация)

Если после Slice 4 baseline measurement < 50 tx/s:

1. **I/O-bound root cause** (найдено в Slice 0): переход к тиру 2 без полного тира 1 — async INSERT в CH заменяет fsync снапшота на горячем пути. Тир 1 (pipeline) при этом всё равно полезен для CPU параллелизма.

2. **Архитектурный потолок** (CPU-bound, но недостаточно воркеров): увеличить пул воркеров, проверить affinity binding, рассмотреть `io_uring` для сетевого ingress (если tokio версия поддерживает).

3. **State contention** (много читателей State при параллельных воркерах): ввести `Arc<RwLock<State>>` с copy-on-read для dry-run воркеров, сохранив единственный writer (orchestrator при seal).

4. **Радикальная мера** (root cause в `apply_tx` мутациях): переход к immutable State с persistent data structures (например, `im` crate — Hash Array Mapped Trie). Это **отдельный ADR**, не V7-S1.

---

## Действия для включения в план

- [ ] **V7-S1 Slice 0:** зафиксировать I/O vs CPU breakdown как условие для выбора пути эскалации.
- [ ] **V7-4:** добавить "CH as primary read backend" в scope и завести тикет.
- [ ] **V7-4:** реализовать `ShardStateCert` + CH таблицу + `/v1/shard/state-cert/latest` endpoint.
- [ ] **V7-6 ADR:** включить вопрос совместимости SEDA + ABCI и `ShardStateCert` → `AppHash` как обязательные пункты оценки.
