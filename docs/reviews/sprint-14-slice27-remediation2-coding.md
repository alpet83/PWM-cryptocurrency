# Sprint 14 Slice27 Remediation2 Coding

## Changes
- Renamed private wallet v3 cleanup helpers to satisfy the <=4 snake_case segment rule:
  - `save_wallet_yaml_v3_merge` -> `save_v3_merge`
  - `serialize_wallet_yaml_v3_clean` -> `ser_v3_clean`
  - `wallet_yaml_v3_clean_value` -> `v3_clean_value`
- Updated the local call sites in `crates/pwm-cli/src/wallet.rs`.
- Verified the old helper names no longer appear in `wallet.rs` or `main.rs`.

## Test Results
- `cargo fmt` — passed.
- `cargo check -p pwm-cli` — passed.
- `cargo test -p pwm-cli wallet_account_add_drops_legacy_active_account_key -- --nocapture` — passed, 1 test, 0.01s.
- `cargo test -p pwm-cli load_wallet_yaml_parses_schema_v3_plaintext_minimal -- --nocapture` — passed, 1 test, 0.00s.
- `cargo test -p pwm-cli wallet_yaml_roundtrip -- --nocapture` — passed, 1 test, 0.00s.

## Hang Note Classification
- Checked `cargo test -p pwm-cli tx_send_recipient_book_tempfile_rejects_unknown_then_allows_after_append -- --nocapture` with a 300s timeout.
- Result: passed in 56.10s, no timeout.
- Classification: existing long-running/flaky wallet/address-book test, not introduced by Slice27 remediation2. The slow path is the test fixture's bounded address brute force; this helper rename does not affect that behavior.

## CQDS
- Background index rebuild was enqueued for project 5 after the code/doc updates.
