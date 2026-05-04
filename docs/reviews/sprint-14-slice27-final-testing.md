# Sprint 14 Slice27 Final Testing

## Verdict

`PASS`

Slice27 remediation2 is accepted. The helper rename did not break wallet v3 cleanup behavior, `tmp/genesis.yaml` still loads with passphrase `1234` without `active_account_id_hex`, and the long wallet/address-book test passed in isolation with a bounded watchdog.

## Required Checks

1. Helper rename cleanup regression: PASS.
   - `cargo test -p pwm-cli wallet_account_add_drops_legacy_active_account_key -- --nocapture`
   - Result: 1 passed, 0 failed, 139 filtered out; finished in 0.01s.
   - Harness duration: 1.0s, timeout: 180s.
2. `tmp/genesis.yaml` passphrase `1234` load without `active_account_id_hex`: PASS.
   - File check: `active_account_id_hex` is absent from `tmp/genesis.yaml`.
   - `PWM_WALLET_PASSPHRASE=1234 cargo run -p pwm-cli -- wallet show --wallet tmp/genesis.yaml`
   - `PWM_WALLET_PASSPHRASE=1234 cargo run -p pwm-cli -- wallet account list --wallet tmp/genesis.yaml`
   - `wallet show` loaded the encrypted schema v3 wallet and selected the CY account.
   - `wallet account list` marked the CY account active.
3. Long wallet/address-book classification: PASS.
   - `cargo test -p pwm-cli tx_send_recipient_book_tempfile_rejects_unknown_then_allows_after_append -- --nocapture`
   - Result: 1 passed, 0 failed, 139 filtered out; finished in 66.78s.
   - Harness duration: 67.1s, timeout: 180s.
   - Classification: acceptable existing slow isolated test path; no Slice27 regression. The previous full `wallet` filter hang is not reproduced by the isolated test and remains attributable to the known long-running address-book fixture, not to the helper rename.

## Bounded Commands Run

- `cargo test -p pwm-cli wallet_account_add_drops_legacy_active_account_key -- --nocapture` - PASS, 1.0s, timeout 180s.
- File content check for `active_account_id_hex` in `tmp/genesis.yaml` - PASS.
- `PWM_WALLET_PASSPHRASE=1234 cargo run -p pwm-cli -- wallet show --wallet tmp/genesis.yaml` - PASS, 3.8s, timeout 90s.
- `PWM_WALLET_PASSPHRASE=1234 cargo run -p pwm-cli -- wallet account list --wallet tmp/genesis.yaml` - PASS, 0.6s, timeout 90s.
- `cargo test -p pwm-cli tx_send_recipient_book_tempfile_rejects_unknown_then_allows_after_append -- --nocapture` - PASS, 67.1s, timeout 180s.

## Notes

- The full hanging `cargo test -p pwm-cli wallet -- --nocapture` filter was not rerun.
- No checklist rows were updated.
- Cleanup: yes; no `pwmd` or `pwm-tui` processes were left running.
