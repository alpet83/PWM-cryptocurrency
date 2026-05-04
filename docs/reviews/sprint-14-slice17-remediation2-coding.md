# Sprint 14 — Slice 17 remediation2 coding note

## Scope
- Urgent deadlock-risk remediation for `pwmd` lock ordering between `app.inner` and `app.init`.
- Style nit fix in `pwm-tui` (`inter_shard_cli_route_message` rename).

## Implemented fixes
- `crates/pwmd/src/api.rs`:
  - Added `snapshot_save_under_inner_lock(...)` so snapshot write happens while `inner` is held, but `init` update is deferred.
  - Refactored `persist_snapshot_or_http_err(...)` to accept precomputed save result and update `init` only after `inner` lock is dropped by caller.
  - Updated `/v1/tx`, `/v1/roaming-intents`, `/v1/roaming-intents/:id/finalize`, `/v1/roaming-intents/:id` paths to persist snapshot via deferred `init` update (no nested `inner+init` lock).
  - Refactored `/v1/status` to avoid overlapping `init` and `inner` read locks.
- `crates/pwmd/src/lifecycle.rs`:
  - In seal loop, snapshot-save result is captured under `inner`; `init` phase update moved into `apply_snapshot_init_state(...)` after dropping `inner`.
- `crates/pwm-tui/src/main.rs`:
  - Renamed helper `inter_shard_cli_route_message` -> `shard_cli_hint` (<=4 words).

## Regression coverage
- Added concurrency smoke test in `crates/pwmd/src/lib.rs`:
  - `v1_status_and_tx_do_not_deadlock_with_snapshot_persist`.
  - Runs concurrent `/v1/status` + `/v1/tx` calls with snapshot persistence enabled and enforces timeout per iteration to catch hangs.

## Safety notes
- Change set is behavior-preserving for business logic; remediation targets lock orchestration only.
- Snapshot error semantics (`ready`/`ready_degraded`) are preserved.
