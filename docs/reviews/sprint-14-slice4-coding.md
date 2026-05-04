# Sprint 14 — Slice 4 coding evidence

## Done

- Added targeted negative test in `crates/pwm-tui/src/main.rs`:
  - `load_wallet_identity_reports_v3_active_account_mismatch_without_panic`
- Scenario: schema v3 wallet has invalid `active_account_id_hex` that does not match any `accounts[*].id_hex`.
- Assertion: `load_wallet_identity(...)` returns `Err` containing `active_account_id_hex` and `not found in accounts` (clear message), with no panic path.

## Scope notes

- No production logic changes were needed; behavior already routed errors through `failed to load wallet ...`.
- Optional UX consistency tweak was intentionally skipped (not required for this closeout slice).
