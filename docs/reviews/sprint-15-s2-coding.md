# Sprint 15 Slice S15-S2 Coding

## Scope
- Implemented explicit runtime/API balance semantics for `pwmd` account views:
  - `local_state_balance`
  - `authoritative_home_balance`
  - `spendable_on_this_shard`
- Remediated legacy `balance_pwm` ambiguity for foreign accounts:
  - local account: `balance_pwm == local_state_balance`
  - foreign account: `balance_pwm == "0"` (safe legacy fallback; not spendable truth)
- Added status-level contract marker so operators/clients can detect split-balance semantics mode.
- Stayed within S15-S2 boundaries (no readiness/genesis/storage changes from S15-S1/S15-S3+).

## Files
- `crates/pwmd/src/api.rs`
  - Extended `AcctOut` with split semantics fields and `local_view_only`.
  - Added runtime mapping that treats foreign accounts as local-view-only and non-spendable on this shard.
  - Hardened legacy `balance_pwm` for foreign accounts to `0` to protect old clients from spendable misread.
  - Extended `StatusOut` with `balance_semantics` contract marker.
- `crates/pwmd/src/lib.rs`
  - Added focused API tests for split balance semantics, foreign spendability guard, and list-view split semantics.
- `crates/pwmd/Cargo.toml`
  - Bumped `pwmd` build/version marker: `0.1.18 -> 0.1.19` (public API response contract changed).

## Tests
- Added `v1_status_exposes_split_balance_semantics_contract`.
- Added `v1_account_marks_foreign_balance_as_non_spendable_local_view`.
- Added `v1_accounts_keeps_local_foreign_split_semantics_in_list_view`.
- Assertions prove that a foreign account can expose local state but is never presented as spendable truth on this shard.

## Compatibility / Migration Policy
- **Compatibility break (intentional, safety-first):** for foreign accounts `balance_pwm` is no longer a mirror of `local_state_balance`; it is now forced to `"0"`.
- Rationale: prevents legacy clients (that only read `balance_pwm`) from treating foreign local-view values as spendable balance.
- Migration path:
  - clients should switch to split fields (`local_state_balance`, `authoritative_home_balance`, `spendable_on_this_shard`) and `local_view_only`;
  - for spendability decisions, rely only on `spendable_on_this_shard` (and authoritative field when implemented).

## Limits
- `authoritative_home_balance` is currently `null` (authoritative proof path is not implemented in this slice).
- CLI/TUI output contract was not expanded in this slice; API contract now carries unambiguous semantics for downstream UX layers.
- No S15-S3/S15-S4 work included.
