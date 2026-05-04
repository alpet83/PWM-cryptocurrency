# Sprint 14 — Slice 7 remediation testing

Repo: `P:/opt/docker/PWM-cryptocurrency`  
Date: `2026-04-28`

## Scope

Validated remediation fixes:
1. `addr-derive` without `--wallet-out` stays stateless (no wallet file writes).
2. `addr-bruteforce` with `--overwrite-wallet` starts from fresh index (`0`), no resume from existing max.
3. Explicit non-tilde wallet paths do not depend on home-dir resolution.

## Commands run

```powershell
cargo test -p pwm-cli addr_derive_cli_keeps_stateless_mode_without_wallet_out
cargo test -p pwm-cli addr_bruteforce_resume_start_index_is_zero_with_overwrite_wallet
cargo test -p pwm-cli resolve_wallet_out_path_keeps_explicit_non_tilde_path_without_home
cargo test -p pwm-cli
```

## Results

- `tests::addr_derive_cli_keeps_stateless_mode_without_wallet_out` — **PASS**.
- `tests::addr_bruteforce_resume_start_index_is_zero_with_overwrite_wallet` — **PASS**.
- `tests::resolve_wallet_out_path_keeps_explicit_non_tilde_path_without_home` — **PASS**.
- `cargo test -p pwm-cli` — **PASS** (`123 passed; 0 failed`; suite time `55.31s`).

## Verdict

`approve` — all three Slice 7 remediation behaviors are covered by focused tests and pass; full `pwm-cli` test suite is green.
