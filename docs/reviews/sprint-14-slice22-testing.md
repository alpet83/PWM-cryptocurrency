# Sprint 14 Slice 22 Testing

Verdict: PASS.

## Scope

- Reviewed Slice22 balance-unit UX and `tx-import` recipient/sender initialization contract against `docs/reviews/sprint-14-slice22-coding.md`.
- Checked focused CLI/TUI code paths and docs only; no broad repository search.

## Evidence

- TUI balance display uses decimal coin units through `format_pwm(raw)`, with unit tests for `0`, `1 raw`, `1 PWM`, and trimmed fractional display (`1.23 PWM`). Owner/receiver tables render balances through this formatter. The F6 send modal and operator docs state `1 PWM = 1_000_000 raw`, while debug JSON/internal fields remain raw.
- CLI help for tx amount/fee flags names raw units and the scale (`1 PWM = 1_000_000 raw`). Covered by `tests::cli_help_names_raw_pwm_units`.
- `tx-import` target `--to` help and runtime stderr note document the target stub contract: missing/uninitialized target accounts may be credited as stubs until recipient `tx-init`.
- Sender-side import auto-init is covered for missing and uninitialized sender accounts, and `tests::tx_import_auto_init_does_not_mask_unknown_export_id` confirms invalid import provenance still fails after auto-init.
- Focused smoke-impact scan found no assert-based CLI smoke tests that pin stderr for `tx-import`; the new note is covered as a unit-level wording contract and did not break current test suites.

## Commands

- `cargo test -p pwm-cli -p pwm-tui` - PASS (`134 + 73` tests, no hang watchdog).
- `cargo check -p pwm-cli -p pwm-tui` - PASS.
- `cargo fmt -p pwm-cli -p pwm-tui --check` - PASS.

Cleanup: no live `pwmd` / `pwm-tui` processes were left by this slice run.

## Checklist

- No `docs/MVP-checklist.md` rows changed in this testing pass; relevant CLI/TUI MVP rows were already marked done.

## Residual Risk

- Actual ratatui framebuffer copy was not asserted from stdout/stderr; per testing policy, on-screen TUI appearance remains a manual operator check unless a machine-readable UI snapshot hook is added.
