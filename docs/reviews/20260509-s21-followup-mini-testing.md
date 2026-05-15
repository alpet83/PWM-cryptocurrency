# S2.1 follow-up mini-testing — lease gate mock + two-process harness

**Date:** 2026-05-09  
**Parent ticket:** `tasks/20260509-s21-external-lease-backend.json`  
**Related:** `docs/reviews/20260509-s21-external-lease-testing.md`

## Scope

 pwm-coding slice closes pwm-testing gaps called out as PARTIAL:

1. **Fail-closed when `LeaseBackend` returns `Err`**
   - Test-only `ErrLeaseBackend` in `crates/pwmd/src/lease_backend.rs` (`#[cfg(test)]`), returns errors on all CAS ops.
   - Unit: `lease::tests::step_lease_backend_err_closed` — asserts `allow_seal == false`, `LeaseState::FencedStandby`, `last_reason` prefix `lease_backend_error `.
   - Integration with seal gate path: `lifecycle::tests::lease_gate_backend_err_closed` — `run_lease_gate(&App)` with injected backend; asserts denial + `lease_last_err` captures injected message.

2. **Two OS processes, shared `--seal-lease-dir` semantics**
   - Helper binary **`pwmd_lease_probe`** (`crates/pwmd/src/bin/pwmd_lease_probe.rs`): one deterministic `step_lease` tick against `FileLeaseBackend`, prints JSON (`allow_seal`, `lease_state`, `last_reason`), exit `0` if sealing allowed else `5`. IO open failure → exit `2`.
   - Integration test **`two_proc_file_lease_takeover`** in `crates/pwmd/tests/lease_two_proc.rs`: two `Command::new` children sharing a temp lease dir; peer `node-a` acquires; peer `node-b` blocked until TTL + takeover window then obtains takeover (`allow_seal` true).
   - Hidden crate exports for the probe only: `step_lease`, `FileLeaseBackend` (`crates/pwmd/src/lib.rs`, `#[doc(hidden)]`).

## Commands (targeted)

- `cargo check -p pwmd`
- `cargo test -p pwmd backend_err_closed --lib`
- `cargo test -p pwmd --test lease_two_proc`

Full `cargo test -p pwmd` was observed with unrelated failures in `tests::transport_peer::{v1_hi_accepts_native_cls,v1_hi_mx_sig}` on one Windows run; treat as separate flake/environment investigation unless reproduced after isolation.

## Platform notes

- **Windows:** file-lock lease backend relies on host filesystem locking behavior; probe harness uses normal temp dirs under `%TEMP%`. If antivirus/indexers delay `.lease.lock`, timeouts could theoretically widen — CI should prefer SSD/local temp.

## Test names (quick index)

| Name | Kind |
|------|------|
| `lease::tests::step_lease_backend_err_closed` | lib unit |
| `lifecycle::tests::lease_gate_backend_err_closed` | lib async unit |
| `two_proc_file_lease_takeover` | integration (`tests/lease_two_proc.rs`) |
