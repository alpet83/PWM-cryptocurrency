# Sprint 7 Checklist: `pwmd` `lib.rs` Decomposition

Дата старта: 2026-04-25  
Фокус: decomposition-only, без изменения поведения.

## Цель спринта

Аккуратно разрезать тяжёлый `crates/pwmd/src/lib.rs` на устойчивые private submodules, сохранив внешний контракт crate `pwmd`, HTTP/API поведение, tx semantics и transport/status semantics.

Sprint 7 следует после Sprint 6 micro-slice optimization conveyor. Sprint 6 подготовил локальную структуру helper-зон, но намеренно не переносил код между модулями.

## Формат спринта

- Фиксированный объём: **8 slices total** (`0/8 ... 7/8`).
- Slice 0: deep review / freeze.
- Slices 1-6: один module-boundary move за slice.
- Slice 7: wrap-up / facade audit.
- Каждый coding slice проходит: `pwm-coding -> pwm-testing -> pwm-review -> orchestrator closeout`.
- Один slice = один отдельный commit.

## Sprint 7 Global No-Change Assertions

- [ ] Не менять HTTP routes, methods, status codes.
- [ ] Не менять JSON response fields и DTO contract.
- [ ] Не менять user-facing error messages.
- [ ] Не менять tx validation/routing/shard semantics.
- [ ] Не менять scheduler/backoff/churn/status transition semantics.
- [ ] Не добавлять новые endpoints, public feature flags или protocol behavior.
- [ ] Не расширять public API ради тестов; для внутренних переносов использовать `pub(crate)` или module-local tests.
- [ ] Сохранять root facade `pwmd::...` для текущих consumers, особенно `crates/pwmd/src/main.rs`.

## Baseline Public Surface To Preserve

- [ ] `parse_cluster_domain_hi(...)`
- [ ] `resolve_runtime_identity(...)`
- [ ] `RuntimeIdentityInput`
- [ ] `RuntimeIdentity`
- [ ] `GenesisSource`
- [ ] `PwmdConfig`
- [ ] `TransportConfig`
- [ ] `ShardId`
- [ ] `run_with(...)`
- [ ] `run(...)`
- [ ] `router(...)`
- [ ] `App`
- [ ] `Inner`
- [ ] `V1_TX_BODY_LIMIT`
- [ ] `app_from_dev_net(...)` / `app_from_dev_net_with_config(...)`
- [ ] `app_from_genesis(...)` / related genesis/bootstrap constructors
- [ ] `load_genesis_bundle(...)`
- [ ] `cors_for_listen(...)`
- [ ] public output/snapshot/peer DTOs currently reachable from crate root

## Standard Gates Per Slice

- [ ] `cargo fmt --check`
- [ ] `cargo check -p pwmd`
- [ ] `cargo test -p pwmd`
- [ ] `pwm-review` semantic gate: no behavior drift, no public contract drift.
- [ ] Artifact evidence records moved symbols/zones, module files, public re-export changes, and no-change assertions.

Additional gates:

- Slice 0 and Slice 7: add `cargo check -p pwmd --bin pwmd`.
- Slice 2: include targeted snapshot compatibility tests.
- Slice 3: include targeted `v1_tx_*` tests.
- Slice 4: include targeted `transport_*`, `real_transport_*`, `policy_*` tests.
- Slice 5: include targeted `v1_*` HTTP/API tests.

## Artifact Policy

- `scoped_diff_stat` must focus on product/tooling code paths only: `crates/**`, `tools/**`.
- Exclude self-referential artifact noise from `scoped_diff_stat`: `docs/reviews/**`, `tasks/*.json`.
- For task manifests, use raw patch/update flow that preserves UTF-8/Cyrillic and avoids full JSON reserialization.

## Slice 0/8: Deep Review And Backlog Freeze

### Scope

- [x] Run `pwm-optimus` deep review over `crates/pwmd/src/lib.rs`, `crates/pwmd/src/main.rs`, and relevant tests.
- [x] Freeze responsibility-zone map:
  - identity/config,
  - snapshot/state persistence,
  - tx/policy guards,
  - transport/peer policy,
  - HTTP/API handlers,
  - app state/bootstrap/lifecycle,
  - tests/helpers.
- [x] Freeze exact module sequence for Slices 1-6.
- [x] Freeze external dependency inventory for root `pwmd::...` consumers.
- [x] Record out-of-scope list for Sprint 7.

### Candidate Files

- `docs/reviews/sprint-7-checklist.md`
- `crates/pwmd/src/lib.rs` (read-only inventory)
- `crates/pwmd/src/main.rs` (read-only consumer inventory)

### Done Criteria

- [x] Checklist contains fixed 8-slice structure.
- [x] Public-surface preservation list exists.
- [x] Per-slice risk and gate policy exists.
- [ ] Baseline commands pass:
  - [x] `cargo fmt --check`
  - [x] `cargo check -p pwmd`
  - [x] `cargo check -p pwmd --bin pwmd`
  - [x] `cargo test -p pwmd`

### Review Risk

- Low. Slice 0 should not move production code.

## Slice 1/8: Identity And Runtime Config Module

### Goal

Move identity/config leaf-zone out of root `lib.rs` while preserving root exports and CLI-facing behavior.

### Candidate Modules

- `crates/pwmd/src/identity.rs`
- `crates/pwmd/src/config.rs`

### Touched Symbols / Zones

- `ShardId`
- `RuntimeIdentity`
- `RuntimeIdentityMode`
- `RuntimeIdentityInput`
- `parse_cluster_domain_hi(...)`
- `resolve_runtime_identity(...)`
- `default_runtime_identity_for_shard(...)`
- `GenesisSource`
- `PwmdConfig`
- `TransportConfig`

### No-Change Assertions

- [x] CLI alias mapping (`A`/`B`) unchanged.
- [x] Partial explicit identity validation unchanged.
- [x] Default config values unchanged.
- [x] Existing root imports in `crates/pwmd/src/main.rs` still compile.

### Testing Gate

- [x] `cargo fmt --check`
- [x] `cargo check -p pwmd`
- [x] `cargo test -p pwmd`

### Review Risk

- Medium: root re-exports and `main.rs` imports must remain stable.

## Slice 2/8: Snapshot And State Persistence Module

### Goal

Move snapshot wire format and persistence code without changing snapshot JSON contract, migration behavior, or error strings.

### Candidate Module

- `crates/pwmd/src/snapshot.rs`

### Touched Symbols / Zones

- `GenesisFile`
- `SnapshotData*`
- `SNAPSHOT_VERSION`
- snapshot serde helpers
- `load_genesis_bundle(...)`
- `load_snapshot(...)`
- `save_snapshot(...)`
- `validate_snapshot(...)`

### No-Change Assertions

- [x] Canonical snapshot parsing unchanged.
- [x] Legacy snapshot migration unchanged.
- [x] Error-message substrings unchanged.
- [x] Atomic temp-write behavior unchanged.
- [x] Genesis validation unchanged.

### Testing Gate

- [x] `cargo fmt --check`
- [x] `cargo check -p pwmd`
- [x] targeted snapshot tests
- [x] `cargo test -p pwmd`

### Review Risk

- High: snapshot compatibility and error strings are persisted/user-facing behavior.

## Slice 3/8: Tx And Policy Guards Module

### Goal

Move local tx routing/policy guard code without changing validation semantics or HTTP error contract.

### Candidate Module

- `crates/pwmd/src/tx_policy.rs`

### Touched Symbols / Zones

- `shard_for_phase1_account(...)`
- `receiver_for_route(...)`
- `enforce_local_tx_guards(...)`

### No-Change Assertions

- [x] `400`/`409` status decisions unchanged.
- [x] User-facing tx error messages unchanged.
- [x] `reserve`/`witness`/`unknown` recipient prefilter unchanged.
- [x] Cross-domain / cross-shard semantics unchanged.
- [x] No changes to core tx model.

### Testing Gate

- [x] `cargo fmt --check`
- [x] `cargo check -p pwmd`
- [x] targeted `v1_tx_*` tests
- [x] `cargo test -p pwmd`

### Review Risk

- High: tx guard drift would be protocol-visible.

## Slice 4/8: Transport And Peer Policy Module

### Goal

Move transport, peer policy, churn/soak state and scheduler code into a module without changing runtime transport behavior.

### Candidate Module

- `crates/pwmd/src/transport.rs` or `crates/pwmd/src/transport/mod.rs`

### Touched Symbols / Zones

- `PeerClass`
- `PeerStatus`
- `PeerRecord`
- `PeerPolicy*`
- `Transport*`
- `ChurnSnapshot`
- `SoakConfidenceSnapshot`
- `HandshakeState`
- transport helpers
- `run_transport_tick*`
- `run_real_transport_tick(...)`
- `spawn_transport_loop(...)`
- `spawn_real_transport_loop(...)`

### No-Change Assertions

- [x] Class labels remain `native` / `foreign` / `unknown`.
- [x] Backoff math unchanged.
- [x] Seed rotation ordering unchanged.
- [x] Reconnect/runaway guard unchanged.
- [x] Churn counters unchanged.
- [x] Dev peer stats fields unchanged.
- [x] Peer status transitions unchanged.

### Testing Gate

- [x] `cargo fmt --check`
- [x] `cargo check -p pwmd`
- [x] targeted `transport_*`, `real_transport_*`, `policy_*` tests
- [x] `cargo test -p pwmd`

### Review Risk

- High: largest move, many private helper dependencies and test-only accesses.

## Slice 5/8: HTTP/API Handlers Module

### Goal

Move DTOs, handlers and router into an API module while preserving exact HTTP behavior.

### Candidate Module

- `crates/pwmd/src/api.rs`

### Touched Symbols / Zones

- `StatusOut`
- `HeadOut`
- `AcctOut`
- `AcctListOut`
- `PeerHelloOut`
- `PeerStatsOut`
- `v1_*` handlers
- `ensure_ready(...)`
- `router(...)`
- `V1_TX_BODY_LIMIT`
- `hex32(...)`
- `parse_id(...)`
- `current_time_ms(...)` if still local to API needs

### No-Change Assertions

- [x] Routes unchanged.
- [x] Methods unchanged.
- [x] Body limit unchanged.
- [x] JSON response fields unchanged.
- [x] Dev-profile `404` messages unchanged.
- [x] Readiness `503` message unchanged.

### Testing Gate

- [x] `cargo fmt --check`
- [x] `cargo check -p pwmd`
- [x] targeted `v1_*` tests
- [x] `cargo test -p pwmd`

### Review Risk

- High: HTTP response contract is user-facing.

## Slice 6/8: App State, Bootstrap And Lifecycle Modules

### Goal

Move app construction and runtime orchestration after dependent modules are stable.

### Candidate Modules

- `crates/pwmd/src/state.rs`
- `crates/pwmd/src/bootstrap.rs`
- optionally `crates/pwmd/src/lifecycle.rs`

### Touched Symbols / Zones

- `App`
- `Inner`
- `InitPhase`
- `InitState`
- `app_from_chain_boot(...)`
- `app_from_dev_net*`
- `app_from_genesis*`
- `spawn_seal_loop(...)`
- `spawn_snapshot_loader(...)`
- `run_with(...)`
- `run(...)`

### No-Change Assertions

- [x] Fast-start snapshot phases unchanged.
- [x] Startup stderr lines unchanged.
- [x] Seal loop behavior unchanged.
- [x] Pool/seal snapshot save behavior unchanged.
- [x] `pwmd::run_with(...)` root API unchanged.
- [x] `App.inner` external behavior preserved.

### Testing Gate

- [x] `cargo fmt --check`
- [x] `cargo check -p pwmd`
- [x] `cargo check -p pwmd --bin pwmd`
- [x] status/snapshot startup tests
- [x] `cargo test -p pwmd`

### Review Risk

- High: likely needs careful `pub(crate)` field/accessor adjustments.

## Slice 7/8: Wrap-Up And Facade Audit

### Goal

Finalize root facade, module declarations and test placement after all moves. No feature or behavior work.

### Candidate Files

- `crates/pwmd/src/lib.rs`
- new `crates/pwmd/src/*.rs` modules
- `docs/reviews/sprint-7-checklist.md`

### Touched Symbols / Zones

- root `mod` declarations
- root `pub use` list
- private module visibility
- test placement cleanup

### No-Change Assertions

- [x] `crates/pwmd/src/main.rs` imports still compile through crate root.
- [x] No new public API except preserved re-exports.
- [x] No route/error/tx/transport semantic drift.
- [x] Module names and responsibilities documented in checklist or review report.

### Testing Gate

- [x] `cargo fmt --check`
- [x] `cargo check -p pwmd`
- [x] `cargo check -p pwmd --bin pwmd`
- [x] `cargo test -p pwmd`
- [ ] optional workspace `cargo check`

### Review Risk

- Medium: facade cleanup can accidentally expose or hide symbols.

### Final module map (Slice 7 wrap-up)

- `config.rs`: `PwmdConfig`, `TransportConfig`, genesis source wiring.
- `identity.rs`: shard aliases and runtime identity resolution.
- `state.rs`: shared app state (`App`, `Inner`, init phase/state).
- `snapshot.rs`: genesis bundle IO and canonical snapshot load/save/validation.
- `tx_policy.rs`: shard routing and local tx guard contract.
- `transport.rs`: peer policy, scheduler/backoff, real transport tick/loops.
- `api.rs`: `/v1/*` handlers, DTOs and router assembly.
- `bootstrap.rs`: app constructors from devnet/genesis/snapshot.
- `lifecycle.rs`: node runtime orchestration (`run_with`, loops, server startup).
- `lib.rs`: crate facade only (root `mod` + `pub use` + integration tests).

## Out Of Scope

- [ ] New feature behavior.
- [ ] New HTTP endpoints.
- [ ] Changes to route names, response fields, status codes, or error strings.
- [ ] Changes to tx validation/routing semantics.
- [ ] Changes to scheduler/backoff/churn/status semantics.
- [ ] Deep performance refactors.
- [ ] Sprint 11 final optimization tasks.
- [ ] Public API expansion solely for tests.

## Open Decisions Before Slice 1

- [x] Inline tests move with their zone modules where practical; keep only genuinely cross-module integration-style tests in root `#[cfg(test)] mod tests`.
- [x] `App.inner` must remain `pub` exactly as now for external smoke/integration consumers.
- [x] Slice 7 does not require workspace-wide `cargo test` if dependency review confirms no external app-level consumers beyond `pwmd`; focused `pwmd` testing plus relevant `cargo check` is sufficient.
