# Smoke test: CLAIM_ALL sentinel + TUI marks modal

**Date:** 2026-05-06  
**Agent:** pwm-testing  
**Feature commit (anchor):** `35f02e8` — feat(claim): CLAIM_ALL sentinel + TUI F5 marks modal + 1-hour guard  
**Workspace HEAD at test time:** `aa87e93` (includes `35f02e8` as ancestor)

## PASS/FAIL matrix

| Check | Result | Notes |
|-------|--------|-------|
| Preflight `P:/opt/docker/rust-target-shared/debug/incremental` | **PASS** | Sum ≈ 1.70 GiB (< 2 GiB); directory not removed |
| `cargo fmt --check` | **PASS** | exit 0 |
| `cargo check --workspace` | **PASS** | exit 0 |
| `cargo test -p pwm-core` | **PASS** | 105 tests, incl. `state::tests::claim_all_sentinel_all_matured` |
| `cargo test -p pwm-core claim` | **PASS** | 4 tests (filtered), incl. `claim_all_sentinel_all_matured` |
| `cargo test -p pwm-cli` | **PASS** | 150 + 3 integration; incl. `tx_claim_cli_parse` / paid / mode err |
| `cargo test -p pwm-tui` | **PASS** | 1 + 42 + 45 tests |
| `scripts/check_rust_fn_name_segments.py` (listed paths) | **PASS** | all files `violations: []` |

## Key behavior (smoke, not TUI manual)

- **Core:** `claim_all_sentinel_all_matured` exercises CLAIM_ALL / matured marks path.
- **CLI:** `Cmd::TxClaim` exposes `--all`; `cli_dispatch` maps `all || claim_units == 0` → `pwm_core::tx::CLAIM_ALL` before `run_tx_claim`. Full `pwm-cli` suite green; there is no dedicated `#[test]` that parses `--all` alone (optional coverage gap).
- **TUI:** F5 marks modal and 1-hour guard are **not** asserted via terminal capture per `docs/AGENT_PROMPT_testing.md` (operator check if needed); automated tests for `pwm-tui` pass.

## Preflight / cleanup handoff

- **preflight_target_debug:** shared incremental assessed; **removed: no** (under 2 GiB).
- **Processes:** none spawned (`pwmd` / `pwm-tui`).
- **snapshot_benches:** not requested for this ticket — **n/a**.

---

`agent`: pwm-testing  
`result`: PASS  
`artifacts`: this file; `tasks/20260506-claim-all-sentinel.json` (delegation)  
`commands`: see table; no hang watchdog  
`cleanup`: n/a  
`token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 90000, "confidence": "low" }`
