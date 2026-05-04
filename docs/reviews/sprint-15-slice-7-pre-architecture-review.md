# Sprint 15 Slice 7 - Pre-Architecture Review

## 1) Scope recap

Ревью выполнено до реализации по направлению:

- ClickHouse redesign (`db per network`, `table per cluster/domain`, `1 row = 1 block`, checkpoint every 100).
- JsonFile epoch split (1000-block files + summary `pwm-data.json`).
- Runtime memory cap 1000 blocks.
- Explorer-friendly fields (`tx_count`, `shard_balance`).
- Wave-based rollout with tests and benchmarks.

Проверенные источники:

- `docs/AGENT_PROMPT_review.md`
- `docs/AGENT_PROMPT_orchestrator.md`
- `docs/reviews/sprint-15-ch-data-model-scaling-review.md`
- `docs/reviews/sprint-15-ch-storage-architecture-review.md`
- `tasks/20260503-s15-slice-7-incremental-storage-architecture.json`
- `crates/pwmd/src/lifecycle.rs`
- `crates/pwmd/src/snapshot/{mod.rs,store.rs,ch_http.rs,io.rs,types.rs,genesis.rs}`
- `crates/pwmd/src/{config.rs,state.rs,api/common.rs,api/handlers_tx.rs,api/handlers_roaming.rs,api/handlers_status.rs}`
- `crates/pwm-core/src/chain.rs`

Ключевой факт: autosnapshot сейчас пишется на каждом sealed block, а checkpoint(100) только логируется.

## 2) Requirements fit

### Внутренняя согласованность с текущими flow `pwmd/pwm-core`

`PARTIAL` (нужны обязательные корректировки дизайна).

- `pwm-core::Chain` считает высоту как `blocks.len()`. При hard cap 1000 без отдельного canonical height сломается рост высоты и producer rotation.
- Текущий snapshot-контракт хранит полные `blocks + state` и валидирует replay всей цепи. Для incremental-пути нужен новый явно зафиксированный replay contract.
- CH row key сейчас включает `cluster_id/node_id`; это уже признано избыточным в прошлых ревью.

### Где может сломаться replay/correctness при памяти <= 1000 blocks

`HIGH` риск:

- `tip_h()` не должен зависеть от длины tail-cache.
- Исторические операции по `blocks[i]` должны быть переведены на canonical source.
- Валидация должна стать `checkpoint root + replay tail`, а не "весь blocks[] из snapshot".
- Нужны дополнительные guards для continuity height/hash.

### Bootstrap/load через epochs + checkpoint

Рекомендуемый безопасный алгоритм:

1. Определить `target_tip_height` и `genesis_digest`.
2. Найти последний checkpoint `<= tip` (кратный 100).
3. Загрузить `state_checkpoint` + metadata (`checkpoint_height`, `state_root`, `genesis_digest`, schema version).
4. Подтянуть epoch-файлы блоков `checkpoint_height+1..tip`.
5. Replay с проверкой непрерывности высот, `prev_hash`, `tx_root/state_root`, сигнатур.
6. В память загрузить только tail (`<=1000`) + отдельно хранить `canonical_height` и `tip_hash`.
7. При пропаже epoch или mismatch - fail-fast.

### Crash consistency / atomicity для Json epochs

`REQUEST CHANGES (HIGH)`:

- Нужен двухфазный commit данных и метаданных.
- Протокол: `tmp write -> fsync -> atomic rename -> manifest update (tmp+rename) -> dir fsync`.
- Recovery после crash: cleanup/ignore незакоммиченных `.tmp`, проверка hash/height диапазонов.

### Минимальные безопасные схемы ClickHouse

Рекомендован минимум v1:

- DB: `pwm_<network_id>`
- `blocks__0xHH`:
  - `height UInt64`
  - `block_hash String`
  - `prev_hash String`
  - `ts UInt64`
  - `prod_idx UInt32`
  - `tx_count UInt32`
  - `state_root String`
  - `payload_json String`
  - `inserted_at DateTime64(3)`
  - `ENGINE ReplacingMergeTree(inserted_at) ORDER BY (height)`
- `checkpoints__0xHH`:
  - `checkpoint_height UInt64`
  - `genesis_digest String`
  - `state_root String`
  - `state_json String`
  - `roaming_json String`
  - `cross_shard_json String`
  - `inserted_at DateTime64(3)`
  - `ENGINE ReplacingMergeTree(inserted_at) ORDER BY (genesis_digest, checkpoint_height)`

Критично: canonical identity не должна включать `node_id`.

### Какие поля сделать first-class для explorer

`v1 mandatory`:

- `height`, `block_hash`, `prev_hash`, `ts`, `prod_idx`
- `tx_count`
- `state_root`
- `inserted_at`

`v1 optional` (только с зафиксированной семантикой):

- `shard_balance`
- `account_count`

Остальное пока оставить в JSON payload.

### Что тестировать/бенчмаркать по волнам

Wave 1 (data contract + JSON storage primitives):

- epoch mapping unit tests
- checkpoint cadence=100 tests
- crash/torn write tests
- fsync/write latency benchmark

Wave 2 (bootstrap/replay correctness):

- checkpoint+epochs replay -> deterministic state root
- negative cases: missing epoch, bad chain continuity, wrong genesis digest
- legacy fallback load compatibility tests
- cold-start benchmark vs full replay baseline

Wave 3 (memory tail cap):

- `canonical_height` monotonic tests with eviction
- e2e after height > 1000
- API regressions (`/v1/head`, tx flows, roaming)
- RSS boundedness benchmark

Wave 4 (ClickHouse incremental):

- per-block insert + per-100 checkpoint integration tests
- multi-node duplicate writer behavior and deterministic read
- schema/version guards
- inserts/sec and parts pressure benchmarks

Wave 5 (explorer/readiness + rollout):

- range query perf
- migration tests old->new
- restart during epoch rotation
- rollback via feature flag

### Запреты (антипаттерны) для `pwm-coding`

- Нельзя считать `tip_h` из `tail_cache.len()`.
- Нельзя писать epochs без atomic manifest protocol.
- Нельзя silently skip corruption on bootstrap.
- Нельзя хранить canonical CH identity с `node_id`.
- Нельзя делать full state/blob insert per block в CH.
- Нельзя менять storage schema без versioned decode path.

## 3) Style and module shape

Для pre-implementation scope новых style violation не выявлено. Для Slice 7 закрепить:

- все новые production/test identifiers <= 5 snake_case segments,
- `//!` для новых нетривиальных модулей,
- не раздувать `snapshot/*` и `lifecycle` в god-modules.

## 4) Safety

Приоритетные риски:

- `HIGH`: потеря корректной canonical height при naive block truncation.
- `HIGH`: partial commit при epoch rotation без manifest atomicity.
- `HIGH`: bootstrap на поврежденном наборе epochs/checkpoints без fail-fast.
- `MEDIUM`: CH multi-writer duplicates без deterministic latest semantics.
- `MEDIUM`: schema/version drift между legacy/new форматом.

## 5) Tests status

Текущее покрытие хорошо для полного snapshot replay, но недостаточно для incremental architecture.
Нужны новые тесты для checkpoint+epochs+tail-cap и migration compatibility.

## 6) Verdict

`REQUEST CHANGES (HIGH)`

Направление верное, но перед `pwm-coding` обязателен formal design lock на:

- separate canonical height,
- atomic epoch commit protocol,
- strict bootstrap/replay contract,
- minimal CH schema + explorer columns policy.

## 7) Participation / token estimate

```json
{
  "agent": "pwm-review",
  "result": "PARTIAL",
  "artifacts": "docs/reviews/sprint-15-slice-7-pre-architecture-review.md",
  "token_usage": {
    "source": "estimate",
    "input": 78000,
    "output": 5200,
    "total": 83200,
    "confidence": "medium"
  }
}
```
