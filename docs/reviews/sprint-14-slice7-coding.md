# Sprint 14 Slice 7 — coding report

## Scope implemented

- Added centralized wallet output path resolver with default `~/.pwm-crypto/default-wallet.yaml`.
- Added `~` expansion for Windows/Unix home resolution and unified usage in wallet-producing commands.
- Added auto-create parent directories before wallet writes.
- Soft-deprecated `addr-derive` (warning + replacement hint) without breaking stdout fields.
- Implemented cluster-aware resume for `addr-bruteforce` with fallback to global max when no domain-compatible accounts exist.
- Stopped persisting `country_code_label` by default for new save flows (read compatibility preserved).
- Updated CLI docs for defaults, deprecation, and resume semantics.

## Files changed

- `crates/pwm-cli/src/main.rs`
- `crates/pwm-cli/src/wallet.rs`
- `docs/pwm-cli.md`
- `docs/reviews/sprint-14-slice7-coding.md`

## Commands run

- `cargo fmt`
- `cargo test -p pwm-cli`

## Test evidence summary

- `cargo test -p pwm-cli` passed (121 passed, 0 failed).
- Added/updated tests for:
  - wallet default-path helper + tilde expansion helpers,
  - parent directory auto-create on wallet save,
  - cluster-aware resume on mixed-domain wallet accounts.

## Notes and tradeoffs

- `addr-derive` now always resolves a wallet output path (explicit or default) and keeps existing stdout fields unchanged.
- `country_code_label` remains in schema for backward read compatibility, but new save paths now pass `None` and serializer omits the field when absent.
