# Sprint 14 Slice27 Remediation Coding

## Fix

- Added v3 wallet YAML cleanup for all v3 write paths that serialize known wallet state: `save_wallet_v3_new`, strict v3 save, and v3 merge-save now remove the legacy top-level `active_account_id_hex` key before writing.
- Kept v3 merge-save preservation for unknown/future metadata, but explicitly excludes the removed active-account marker.
- Added a regression where an old v3 wallet containing `active_account_id_hex` is loaded and rewritten by `wallet_account_add`; the saved YAML no longer contains the legacy key and still loads.

## Results

- `cargo fmt` passed.
- `cargo test -p pwm-cli wallet_account_add_drops_legacy_active_account_key -- --nocapture` passed: 1 test.
- `cargo test -p pwm-cli wallet -- --nocapture` passed: 69 tests.

## Notes

- Fresh v3 writes were already covered by `save_wallet_v3_new_creates_parent_directories`; the shared cleanup keeps that behavior explicit even if an in-memory legacy value is present.
- Existing old v3 files still load through the existing legacy-active load regression.
