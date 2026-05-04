# Sprint 15 S2 Testing (Remediation Retest)

## Verdict
`PASS`

## Retest Scope
- Validate foreign `balance_pwm` safe behavior in `/v1/account` and `/v1/accounts`.
- Validate split semantics are preserved.
- Validate status marker contract is unchanged.

## Focused Retest Runs
1. `cargo test -p pwmd v1_account_marks_foreign_balance_as_non_spendable_local_view -- --nocapture`  
   Result: **PASS** (`1 passed, 0 failed`).
2. `cargo test -p pwmd v1_accounts_keeps_local_foreign_split_semantics_in_list_view -- --nocapture`  
   Result: **PASS** (`1 passed, 0 failed`).
3. `cargo test -p pwmd v1_status_exposes_split_balance_semantics_contract -- --nocapture`  
   Result: **PASS** (`1 passed, 0 failed`).

## Validation Result
- `/v1/account`: foreign account remains local-view only; spendable truth is not exposed as local spendability.
- `/v1/accounts`: list view preserves local/foreign split semantics and does not collapse foreign view into spendable local truth.
- Legacy compatibility remains intact: `balance_pwm` stays safe and aligned with split model expectations.
- `/v1/status`: `balance_semantics` contract marker is unchanged (`split:v1(local_state_balance,authoritative_home_balance,spendable_on_this_shard)`).

## Notes
- Live daemon re-check was attempted, but host disk-space exhaustion prevented a fresh `cargo run` build (`os error 112`). The remediation verdict is based on passing endpoint-level `pwmd` tests above.
