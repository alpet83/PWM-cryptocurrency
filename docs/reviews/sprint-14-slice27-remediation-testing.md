# Sprint 14 Slice27 Remediation Testing

## Verdict

`FAIL`

The merge-save remediation itself passed targeted checks, but the required full command
`cargo test -p pwm-cli wallet -- --nocapture` did not complete: it hung on
`wallet::tests::tx_send_recipient_book_tempfile_rejects_unknown_then_allows_after_append`
after Rust's test harness reported it had been running for over 60 seconds.

## Required Checks

1. Regression `wallet_account_add_drops_legacy_active_account_key`: PASS.
   - `cargo test -p pwm-cli wallet_account_add_drops_legacy_active_account_key -- --nocapture`
   - Result: 1 passed, 0 failed.
2. `cargo test -p pwm-cli wallet -- --nocapture`: FAIL.
   - Result before kill: 68 wallet-filtered tests had printed `ok`; the remaining
     `wallet::tests::tx_send_recipient_book_tempfile_rejects_unknown_then_allows_after_append`
     exceeded 60 seconds and the run was stopped.
3. v3 fresh/new and merge-save paths omit `active_account_id_hex`: PASS.
   - `wallet::tests::save_wallet_v3_new_creates_parent_directories` passed and asserts the saved fresh v3 YAML omits the key.
   - `wallet::tests::wallet_account_add_drops_legacy_active_account_key` passed and asserts merge-save after `wallet_account_add` removes the legacy key.
   - `tests::tx_signer_uses_deterministic_v3_default_without_active_marker` passed and asserts the post-add v3 wallet still omits the key while deterministic signer selection works.
4. Old wallet with `active_account_id_hex` still loads: PASS.
   - Covered by `wallet::tests::load_wallet_yaml_ignores_v3_legacy_active_account`.
   - Also covered in the regression before the rewrite step.
5. `tmp/genesis.yaml` passphrase `1234` load/account-list sanity: PASS.
   - `tmp/genesis.yaml` contains no `active_account_id_hex`.
   - `PWM_WALLET_PASSPHRASE=1234 cargo run -p pwm-cli -- wallet show --wallet tmp/genesis.yaml` passed.
   - `PWM_WALLET_PASSPHRASE=1234 cargo run -p pwm-cli -- wallet account list --wallet tmp/genesis.yaml` passed and marked the CY account as active.

## Commands Run

- `cargo test -p pwm-cli wallet_account_add_drops_legacy_active_account_key -- --nocapture` - PASS.
- `cargo test -p pwm-cli wallet -- --nocapture` - FAIL/HANG; stopped after the harness reported the long-running wallet test.
- `cargo test -p pwm-cli save_wallet_v3_new_creates_parent_directories -- --nocapture` - PASS.
- `cargo test -p pwm-cli load_wallet_yaml_ignores_v3_legacy_active_account -- --nocapture` - PASS.
- `cargo test -p pwm-cli tx_signer_uses_deterministic_v3_default_without_active_marker -- --nocapture` - PASS.
- `rg -n "active_account_id_hex" tmp/genesis.yaml` - PASS; no matches.
- `PWM_WALLET_PASSPHRASE=1234 cargo run -p pwm-cli -- wallet show --wallet tmp/genesis.yaml` - PASS.
- `PWM_WALLET_PASSPHRASE=1234 cargo run -p pwm-cli -- wallet account list --wallet tmp/genesis.yaml` - PASS.

## Notes

- No checklist rows were updated.
- Cleanup: yes; the hung cargo process was stopped, and no `pwmd`/`pwm-tui` processes were left running.
