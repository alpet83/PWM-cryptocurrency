# S2.1 follow-up mini-testing — pwm-testing report

**Date:** 2026-05-09  
**Ticket:** `tasks/20260509-s21-followup-mini-tests.json`  
**Coding commit (anchored):** `6a33cb9b484934fc386129243229942cfe655f51`  
**Platform:** Windows (repo `P:\opt\docker\PWM-cryptocurrency`)

## Verdict: **PARTIAL** (targeted **PASS**; full `pwmd` suite **FAIL** on known `transport_peer`)

| Command | Result |
|---------|--------|
| `cargo test -p pwmd backend_err_closed --lib` | **PASS** — 2 tests: `lease::tests::step_lease_backend_err_closed`, `lifecycle::tests::lease_gate_backend_err_closed` |
| `cargo test -p pwmd lease_gate_backend_err_closed --lib` | **PASS** — 1 test (subset of above) |
| `cargo test -p pwmd --test lease_two_proc` | **PASS** — `two_proc_file_lease_takeover` |
| `cargo check -p pwmd` | **PASS** |
| `cargo test -p pwmd` (full) | **FAIL** — 301 passed, 2 failed |

## Full-suite failures (isolation)

Failures are **not** in lease/mock-backend/two-proc paths:

- `tests::transport_peer::v1_hi_accepts_native_cls` — panic `crates/pwmd/src/tests/transport_peer.rs:25` (`left: false`, `right: true`)
- `tests::transport_peer::v1_hi_mx_sig` — panic `crates/pwmd/src/tests/transport_peer.rs:83`

Same pair called out in `docs/reviews/20260509-s21-followup-mini-testing.md` as unrelated Windows/environment noise. **No evidence** the S2.1 follow-up commit introduced these; they are pre-existing relative to this slice’s scope.

## Delegation

- **Agent:** `pwm-testing`
- **Targeted acceptance:** satisfied for fail-closed backend gate + `lease_two_proc` + `cargo check -p pwmd`.
