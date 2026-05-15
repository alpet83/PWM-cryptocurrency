# Debug controls and protocol versioning: design gate

Дата: 2026-05-09  
Тикет: `tasks/20260509-protocol-versioning-debug-controls.json`  
Роль: `pwm-debug`, design/debug gate без правок продуктового Rust.

## Вывод

Рекомендую разделить реализацию на 2 слайса.

1. **Slice A: compatibility + operator observability.** Build-control log, protocol semver guard, дисциплина инкремента версии в `pwm-coding`/`pwm-review`. Это низкий риск, изолировано в startup/handshake/transport и даёт быстрый сигнал о stale build и несовместимых пирах.
2. **Slice B: divergence debug + timing controls.** Debug dump `state/blocks/b{height}.json`, режимы dump-on-divergence и time-align seal. Это шире по storage/snapshot/seal-loop, требует аккуратных флагов и тестов на отсутствие побочных файлов при дефолтах.

Не рекомендую делать time-align как средство консенсуса. Уже есть `--debug-deterministic-seal-time` / `PWM_DEBUG_DETERMINISTIC_SEAL_TIME`, который даёт настоящую hash-parity для тестов. Выравнивание seal в середину секунды может снизить случайное расхождение при wall-clock `ts`, но не гарантирует одинаковую историю при одном validator key, потому что разные ноды могут иметь разные mempool batches, scheduling, snapshot load state и network apply order.

## Карта кода

### 1. Build control log

Точки входа:

- `crates/pwmd/src/main.rs`: `main()` парсит CLI, строит `LoggingConfig`, вызывает `init_logging`, затем `logger()`.
- `crates/pwmd/src/lifecycle.rs`: `run_with()` уже пишет startup/persist/listen логи через `tracing` и `crate::logger()`.
- `crates/pwmd/src/logging.rs`: форматтер и file sinks; дефолтный `EnvFilter` = `debug`, main/peer sink разделяются по target `pwmd::peer`.

MVP:

- После успешного `init_logging` и до `run_with()` залогировать один startup event: `binary_path`, `binary_mtime_unix_ms`, `binary_size_bytes`, `pid`, `cargo_pkg_version`, `git_sha` если доступен через build script/env.
- Флаг не нужен: это operator safety signal, не меняет поведение и мало шумит.
- Если `std::env::current_exe()` или metadata fails, логировать warn с явным `binary_mtime=unavailable`, но не падать.

### 2. Protocol versioning for peers

Точки входа:

- `crates/pwmd/src/handshake.rs`: `NodeHelloCapabilities.protocol_version`, `NodeHelloCapabilities::supports_sync_v1()`, `HandshakeRejectReason`.
- `crates/pwmd/src/transport/dial.rs`: `build_local_node_hello()` сейчас жёстко ставит `"0.1.0"`.
- `crates/pwmd/src/transport/incoming_hello.rs`: `process_incoming_peer_hello()` уже централизованно принимает/отклоняет hello, логирует `peer hello rejected`, пишет metrics/reason.
- `crates/pwmd/src/transport/peer_session/inbound.rs` и `crates/pwmd/src/transport/peer_session/seed/handshake.rs`: inbound/outbound превращают `Err(reason)` в `HelloAck { accepted: false, reason }`, close/reconnect records.
- `crates/pwmd/src/transport/peer_types.rs`: `PeerCloseReason::HandshakeRejected` уже подходит для major mismatch.

MVP:

- Вынести локальную wire/protocol версию в константу, например `handshake::PROTOCOL_VERSION: &str = env!("CARGO_PKG_VERSION")` или отдельную `PWM_PROTOCOL_VERSION`. Для MVP лучше отдельная константа `0.1.0`, чтобы review мог требовать явного bump при wire-incompatible изменениях.
- Добавить parser `ProtocolVersion { major, minor, patch }` без внешней зависимости. Невалидная версия remote hello = hard reject `protocol_version_malformed`.
- Проверка в `process_incoming_peer_hello()` после базового `validate_node_hello()`:
  - `remote.major != local.major` => reject / close: `protocol_version_major_mismatch expected=... received=...`.
  - `remote.major == local.major && (minor/patch differ)` => `warn!` only: `protocol_version_fractional_mismatch`, handshake continues.
- Для HTTP seed path `attempt_seed_connect()` не нужен отдельный guard, потому что TCP/stateful handshake всё равно проходит через `NodeHello`; если HTTP `/v1/status` начинает публиковать protocol_version, это можно добавить позже.
- Дисциплина инкремента: `pwm-coding` должен обновить `docs/AGENT_PROMPT_coding.md` или reviewer checklist, а `pwm-review` проверять, что любое изменение `NodeHello`, `PeerWireMsg`, sync wire limits/profile или block/snapshot wire semantics либо bump-ит `PWM_PROTOCOL_VERSION`, либо содержит явное обоснование "no wire compatibility impact".

### 3. Debug dump for persistent divergence

Точки входа:

- `crates/pwmd/src/transport/peer_session/sync_live.rs`: вероятная зона обнаружения tip/divergence в sync-v1, рядом с `SyncTipDivergence`.
- `crates/pwmd/src/transport/peer_types.rs`: есть close reason `SyncTipDivergence`.
- `crates/pwmd/src/lifecycle.rs`: seal loop знает `h`, `blk`, `st_before`, `chain.st`, backend path, и может писать per-block dump после seal.
- `crates/pwmd/src/state.rs`: `App` хранит `data_file`, `state_namespace`, runtime flags; сюда логично добавить debug config fields.
- `crates/pwmd/src/snapshot/io.rs`: уже есть canonical v2 block/snapshot JSON helpers; `SnapshotData`/`Block` serializable.
- `crates/pwm-core/src/block.rs`: `Block` и `BlockHdr` имеют `Serialize`/`Deserialize`; `hdr_hash()` даёт block hash.

MVP:

- Флаги, дефолт выключен:
  - `--debug-dump-blocks` / `PWM_DEBUG_DUMP_BLOCKS=1`: писать каждый sealed/applied block.
  - `--debug-dump-on-divergence` / `PWM_DEBUG_DUMP_ON_DIVERGENCE=1`: включать dump только при persistent divergence signal.
  - `--debug-dump-dir <DIR>` / `PWM_DEBUG_DUMP_DIR`, default: `<state_root>/<namespace>/blocks` или `data_file.parent()/blocks`.
- Формат: `state/blocks/b{height}.json`, pretty JSON с минимумом:
  - `height`, `hash`, `source` (`local_seal`, `sync_apply`, `divergence_probe`), `node_id`, `protocol_version`, `block`.
  - В `block` использовать canonical serde для `pwm_core::block::Block`.
- "Persistent divergence" для MVP: не пытаться строить reorg. При повторном `SyncTipDivergence` для того же remote node или при N последовательных mismatched tip announcements (N=2/3, configurable later) включить dump последнего local tip и, если remote block body доступен в sync frame, remote block в `b{height}.remote-{node}.json`.
- Не писать дампы по дефолту и не смешивать с autosnapshot `pwm-data.json`.

### 4. Time-align seal vs deterministic-time

Точки входа:

- `crates/pwm-core/src/chain.rs`: `SealTimeMode::{WallClock, DeterministicHeight}`, `next_apply_ctx()`, `seal()`, `BlockHdr.ts`.
- `crates/pwmd/src/main.rs`: CLI/env для `--debug-deterministic-seal-time`.
- `crates/pwmd/src/lifecycle.rs`: seal loop interval = 2s, вызывает `g.chain.seal(txs)`.
- `crates/pwm-core/src/block.rs`: `BlockHdr.ts` входит в signing payload и `hdr_hash()`, поэтому разные секунды дают разные подписи/hash.

Риск-оценка:

- **Time-align** снижает вероятность, что две ноды запечатают соседние секунды на границе, но не гарантирует одинаковую `ts`: ОС может задержать task, часы могут расходиться, interval может стартовать в разных фазах, а mempool может отличаться.
- **Deterministic-time** (`base + height`) гарантирует одинаковую `ts` при одинаковой высоте/tx/state и уже покрыт тестами `det_mode_stable_hdr_hash` / `det_mode_stable_hash_apps`, но искусственно меняет season/claim-time semantics. Его нельзя считать production consensus policy без отдельного RFC.
- Для MVP time-align должен быть debug/dev флагом, не default. Название: `--debug-align-seal-mid-second` / `PWM_DEBUG_ALIGN_SEAL_MID_SECOND=1`.
- Реализация должна жить в `lifecycle.rs` перед взятием write lock и seal: sleep до `subsec_millis` около 500 ms с bounded delay, например не больше 750 ms. Не менять `Chain::next_apply_ctx()` для всех callers.
- Если нужен настоящий same-shard deterministic consensus, это отдельный slice: leader/follower role, disable local seal on followers, sync apply path, deterministic ordering. Один общий validator key сам по себе не устраняет divergence.

### 5. Два узла с одним validator key

Текущая модель допускает разные истории при одном validator key:

- `Chain::seal()` выбирает producer по height и подписывает локальный header той же key, но `prev_hash`, `ts`, `tx_root`, `state_root` берутся из локального состояния.
- `spawn_seal_loop()` каждые 2 секунды независимо берёт до 64 tx из локального mempool (`g.pool.take(64)`) и seal-ит локально.
- Даже при одинаковом key подписи будут валидны для разных headers; сеть увидит competing tips без полноценного consensus/reorg/leader election.
- Флаг `--debug-disable-seal-loop` уже нужен для follower-mode harness: follower не должен сам produce-ить competing blocks, если цель теста - parity через sync.

## Acceptance criteria для coding

Slice A:

- Startup log содержит `binary_mtime_unix_ms` и `binary_path` в main log при обычном `pwmd`.
- При major mismatch remote `NodeHello.capabilities.protocol_version` handshake rejected/closed с явным reason `protocol_version_major_mismatch`; inbound отдаёт `HelloAck accepted=false`.
- При minor/patch mismatch handshake accepted, в peer log есть warn `protocol_version_fractional_mismatch`.
- Existing trust/genesis/network mismatch tests остаются зелёными.
- Добавлена reviewer/coding дисциплина protocol bump для wire changes.

Slice B:

- Все debug dump/time-align флаги default off; обычный запуск не создаёт `state/blocks`.
- При `--debug-dump-blocks` после seal появляется `state/blocks/b1.json` с block JSON и hash/source metadata.
- При `--debug-dump-on-divergence` dump включается только после simulated persistent divergence; одиночный transient mismatch не шумит.
- `--debug-align-seal-mid-second` логирует активацию и не меняет deterministic mode; если включён вместе с deterministic mode, deterministic mode wins или startup rejects combination с понятным текстом.
- `--debug-deterministic-seal-time` остаётся единственным режимом, который acceptance tests используют для hash-parity.

## Test plan для coding/testing

- `cargo test -p pwmd handshake` или точечные module tests:
  - semver parse ok/bad;
  - major mismatch reject;
  - patch/minor mismatch warn-only/accept.
- `cargo test -p pwmd transport_peer` / relevant transport tests:
  - inbound `HelloAck accepted=false` on major mismatch;
  - outbound seed path records `HandshakeRejected`.
- `cargo test -p pwmd logging` or new startup helper unit:
  - binary metadata helper handles unavailable metadata without panic.
- `cargo test -p pwmd lifecycle`:
  - dump default off;
  - dump writes `b{height}.json` under temp dir when enabled;
  - deterministic seal-time parity still passes.
- Host smoke via `cq_process_ctl`/Git Bash:
  - run two `pwmd` with same genesis and mismatched protocol override/test hook, assert peer log reason.
  - run with `--debug-stop-height 1 --debug-dump-blocks`, assert block dump exists and contains `hdr.height=1`.
- Review gate:
  - `cargo fmt --all -- --check`;
  - `cargo test -p pwm-core`;
  - `cargo test -p pwmd`;
  - `cargo check --workspace`.

## Actionable checklist for `pwm-coding`

1. Add `PWM_PROTOCOL_VERSION` constant and semver parser/checker near `handshake.rs`; keep symbol names within repo naming limits.
2. Replace hardcoded `"0.1.0"` in `build_local_node_hello()` with the constant.
3. Enforce protocol check in `process_incoming_peer_hello()`: major reject, fractional warn-only.
4. Add unit tests for protocol parse/check and inbound/outbound reject paths.
5. Add startup binary metadata helper and log it once after logging init.
6. Add protocol bump discipline to coding/review prompt or checklist used by this repo.
7. In a second slice, add `DebugDumpConfig` fields to `PwmdConfig`/`App`, CLI/env flags, and safe default-off path resolution.
8. Implement per-block JSON dump helper using `serde_json::to_writer_pretty` and atomic-ish temp-file rename.
9. Wire dump after local seal and after persistent sync divergence only when enabled.
10. Add `--debug-align-seal-mid-second` as debug/dev-only; do not use it for parity acceptance where deterministic time is required.
