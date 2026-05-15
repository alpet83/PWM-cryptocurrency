# S2.1 external lease backend — testing report (pwm-testing)

**Date:** 2026-05-09  
**Workspace:** validated at pre-commit `7266d96785c9bd3e9644ab75903485d086701589` (tests run on that tree; report/ticket committed afterward).  
**Commit validation note:** prompt referenced full objects `0977e8c975cd2e20f8fc72f8fac2ef417b75ebad` and `7266d96d27adccf1474d03680bd1579919f61833`; **`git cat-file` fails** for both in this clone. Local implementation matches **`0977e8c03a487695ab2b6a7334f9680fc2928276`** (prefix `0977e8c`). Ticket `coding_commit` / `commits[]` were corrected to that object; `HEAD` at test time **`7266d96785c9bd3e9644ab75903485d086701589`** (differs from prompt `7266d96d27…`).

**Verdict: PARTIAL** — unit coverage for file backend + in-process two-`LeaseRuntime` simulation passes; explicit **backend `Err` → fail-closed** and **two separate `pwmd` processes** are not covered by the targeted test run (see gaps).

## Preflight

- `powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/dev/preflight_target_debug.ps1` → OK (`target/debug` under threshold).

## Commands

| Command | Result |
|--------|--------|
| `cargo test -p pwmd lease -- --nocapture` | **PASS** — 9 tests |
| `cargo check -p pwmd` | **PASS** |

## Checks vs acceptance

### 1) Default backend mode and config parsing

- **Config default:** `PwmdConfig::default().seal_lease_backend == LeaseBackendMode::File` — covered by `config::tests::lease_backend_default_file`.
- **CLI:** `CliLeaseBackend::File` default (`main.rs`, `default_hash_t = CliLeaseBackend::File`); when `--seal-lease-dir` omitted, `seal_lease_dir = state_root.join("leases")`.
- **Evidence:** `cargo test … lease_backend_default_file` ok.

### 2) CAS semantics (acquire / renew / takeover / release)

- **File backend:** `file_acq_then_renew_ok`, `file_takeover_cas_gate` (wrong `exp_expiry` → `CasMiss`; matching triple → `Taken` with incremented term/fence), `file_release_cas_gate`.
- **Runtime (`step_lease`):** renew CAS miss and takeover CAS paths set `last_reason` with `cas_miss`; stale owner scenarios covered with `ProcessLocalLeaseBackend` and with `file_two_node_takeover_sim`.

### 3) `single_sealer` fail-closed gate on backend errors

- **Code review:** `step_lease` sets `allow_seal = false`, `last_reason = lease_backend_error {e}` on `acquire`/`renew`/`takeover` errors; `run_lease_gate` returns `step.allow_seal` (so seal loop skips when false). `lease_last_err` is updated when `last_reason` starts with `lease_backend_error `.
- **Automated gap:** no test injects a failing `LeaseBackend` (e.g. permission/IO) to assert `allow_seal == false` through the gate end-to-end.

### 4) Two-node same-key simulation (one active, takeover after timeout)

- **Automated:** `lease::tests::file_two_node_takeover_sim` — single process, shared `FileLeaseBackend`, two `LeaseRuntime`s; standby blocked → `SuspectActiveLost` → takeover; old owner blocked after successor.
- **Gap vs ticket acceptance plan:** not two **OS processes** / two **`pwmd`** instances with distinct ports and data dirs (manual / follow-up integration).

### 5) Status observability fields

- **`StatusOut` / `v1_status`:** `lease_backend_mode`, `lease_backend_path`, `lease_last_backend_error`, `lease_state`, `seal_gate_allowed`, `lease_owner_id`, `lease_term`, `lease_expires_at_ms`, `lease_last_tip`, `lease_fence`, `lease_last_reason`, counters `lease_acquire_ok` … `lease_takeover_ok`.
- **Note:** `handlers_status` unit test `status_exposes_identity_signals` still uses bootstrap defaults (`lease_backend_mode == "process_local"`) — consistent with `app_from_dev_net()`, not an indicator of CLI defaults.

## Tests executed (name list)

- `config::tests::lease_backend_default_file`
- `lease::tests::{lease_renew_ok_same_owner, lease_takeover_after_timeout, old_active_blocked_without_lease, lease_release_cas_ok, file_two_node_takeover_sim}`
- `lease_backend::tests::{file_acq_then_renew_ok, file_takeover_cas_gate, file_release_cas_gate}`

## Follow-ups (recommended)

1. Add a **mock `LeaseBackend`** returning `Err` for acquire/renew to assert `LeaseStep.allow_seal == false` and (optional) a thin `run_lease_gate` test if exposed to tests.  
2. Optional **two-process `pwmd`** harness under `tests/` or runbook-driven manual step — matches ticket `artifacts.acceptance_plan` item 2–3.

## Mini follow-up (2026-05-09)

Implemented in **`docs/reviews/20260509-s21-followup-mini-testing.md`**: `ErrLeaseBackend` + `run_lease_gate` unit coverage, and two-process harness via `pwmd_lease_probe` + `tests/lease_two_proc.rs`.
