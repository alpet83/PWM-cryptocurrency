# Sprint 14 Slice 22 Coding

## Decisions

- Balance UX: internal/API balances stay raw, but public TUI tables now render decimal `PWM` with fixed scale `1 PWM = 1_000_000 raw`. CLI tx amount/fee help now names raw units explicitly so operator input is not confused with displayed coin units.
- Target recipient initialization: keep current IMPORT auto-stub contract for missing/uninitialized target `--to`; make it explicit in CLI help, a runtime note before `tx-import` submit, and operator docs. No new blocker for transactions to uninitialized target addresses was added.

## Tests Run

- `cargo fmt` - blocked by pre-existing parse error in `crates/pwmd/src/snapshot.rs` (`SNAPSHOT_VERSION as u64` pattern).
- `cargo fmt -p pwm-cli -p pwm-tui` - pass.
- `cargo test -p pwm-cli -p pwm-tui` - pass (`134 + 73` tests; latest audited count, matching `sprint-14-slice22-testing.md`).
- `cargo check -p pwm-cli -p pwm-tui` - pass.

## Notes

- Raw precision remains preserved in structs, RPC JSON, tx payloads, and state transitions.
- Added focused tests for TUI balance formatting and CLI help/import note wording.
- CQDS background index rebuild requested for project `5`; MCP returned `enqueue=duplicate` for existing `code_index` job.
