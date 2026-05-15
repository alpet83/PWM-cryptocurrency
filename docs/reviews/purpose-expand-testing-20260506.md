# Smoke test: purpose placeholder expansion (`{utc_time}` / `{utc_timestamp}`)

**Date:** 2026-05-06  
**Agent:** pwm-testing  
**Feature commit:** `d2d51a2` — feat(burn): purpose placeholder expansion in CLI+TUI  

## Verdict

**PASS**

## Preflight

| Item | Result |
|------|--------|
| `P:\opt\docker\rust-target-shared\debug\incremental` | Present; size **≤ 2 GiB** — no delete |

## Command matrix

| Command | Result | Notes |
|---------|--------|--------|
| `cargo fmt --check` | PASS | |
| `cargo check --workspace` | PASS | |
| `cargo test -p pwm-cli` | PASS | 150 unit + 3 integration; `purpose_expand::*` tests green |
| `cargo test -p pwm-tui` | PASS | lib + integration (`send_form`, `wallet_roaming`) |
| `python scripts/check_rust_fn_name_segments.py` (listed files) | PASS | `violations: []` for all paths |

## Naming check paths

- `crates/pwm-cli/src/purpose_expand.rs`
- `crates/pwm-cli/src/cmd_tx.rs`
- `crates/pwm-tui/src/tx_submit.rs`

## Notes

- PowerShell may surface `NativeCommandError` on `cargo` stderr (`Finished test profile…`); test exit codes and summaries were **ok** / **0 failed**.
