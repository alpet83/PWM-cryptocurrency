# Sprint 14 Slice 7 — remediation coding note

## Scope remediated

- Restored `addr-derive` stateless-by-default behavior:
  - when `--wallet-out` is omitted, wallet file is not written;
  - deprecation warning remains unchanged;
  - stdout compatibility is preserved (`wallet_path` still printed, `wallet_write_mode` is now `stateless` in this mode).
- Fixed `addr-bruteforce --overwrite-wallet` semantics:
  - resume now starts from `0` (fresh start) when overwrite flag is set;
  - append-by-default and resume-from-wallet behavior stays unchanged when overwrite flag is absent.
- Hardened wallet path resolver:
  - explicit non-tilde path no longer requires home directory resolution;
  - home resolution is used only for default path and `~` expansion cases.

## Files touched for remediation

- `crates/pwm-cli/src/main.rs`
- `docs/pwm-cli.md`
- `issues-report.md`

## Tests updated

- Added regression test for overwrite fresh-start:
  - `addr_bruteforce_resume_start_index_is_zero_with_overwrite_wallet`
- Added resolver robustness test:
  - `resolve_wallet_out_path_keeps_explicit_non_tilde_path_without_home`

## Commands run

- `cargo fmt`
- `cargo test -p pwm-cli`

## Result

- `cargo test -p pwm-cli` passed: `123 passed; 0 failed`.
