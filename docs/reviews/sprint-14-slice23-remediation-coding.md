# Sprint 14 Slice23 Remediation Coding

## Changes
- Updated the stale `tx-import` CLI note test to assert the strict initialized target-recipient contract.
- Replaced tester-guide wording that described missing/uninitialized target recipients as stub-creditable.
- Shortened touched private CLI helpers:
  - `parse_initialized_from_account_json` -> `parse_init_flag`
  - `try_fetch_nonce_and_initialized_from_account_json` -> `parse_nonce_init`
  - `ensure_sender_initialized_for_import` -> `ensure_import_sender`
  - `post_tx_import_with_retry` -> `post_import_retry`

## Results
- `cargo fmt` — passed.
- `cargo test -p pwm-cli` — passed, 138 tests.
- `cargo test -p pwm-core recipient` — passed, 3 tests.
- `cargo test -p pwmd recipient` — passed, 11 tests in `lib.rs` and 0 tests in `main.rs`.
- `cargo check` — passed.

## Notes
- `tx-import` policy is now documented as: target `--to` must already be initialized on the target shard; missing or `initialized=false` rejects before credit.
- No commit was created.
