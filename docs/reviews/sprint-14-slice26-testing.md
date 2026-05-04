# Sprint 14 Slice26 Testing: TUI owner-selection signing fix

Date: 2026-04-29

## Verdict

PASS.

`pwm-tui` now has regression coverage for the Slice25 review finding: the F6 send form and signing path use the runtime selected Owner row, not the persisted active wallet account. If the selected owner cannot be signed, the TUI path blocks before RPC submit with a clear error.

## Required checks

1. `cargo test -p pwm-tui` passes: 79 passed, 0 failed, 0 ignored.
2. F6 form uses selected Owner row as sender when wallet active account differs: covered by `f6_wallet_mode_uses_selected_owner_row_as_sender`.
3. Signing derives/signs selected wallet v3 account, not persisted active account: covered by `signing_material_for_sender` selecting `owned_accounts` metadata and deriving from wallet seed/payload for non-active selected accounts.
4. If selected owner cannot be signed, TUI blocks before submit with clear message: covered by `signing_rejects_selected_owner_without_material`; F6 also calls `signing_material_for_sender` before opening the send form.
5. CY selected while DB active never signs DB: covered by `cy_selected_while_db_active_signs_cy_not_db`, which asserts signed sender is CY and `domain_hi != 0xDB`.
6. `tmp/genesis.yaml` passphrase `1234` was inspected. Public wallet metadata remains the same as Slice24: schema v3 encrypted wallet, active CY account `2cfb...ae5e`, second DO account `32ec...c1c5`. The old `domain_hi=0xDB` mismatch is explained as the prior mixed active-header/signing-payload path; current TUI unit coverage proves selected CY signing derives CY material or blocks before submit. I did not assert live on-screen TUI text because ratatui alternate-screen output is not a reliable machine-checkable channel.

## Commands

- `cargo fmt -p pwm-tui` - pass.
- `cargo test -p pwm-tui` - pass, 79 tests.
- `PWM_WALLET_PASSPHRASE=1234 target/debug/pwm.exe wallet show --wallet tmp/genesis.yaml` - pass.
- `PWM_WALLET_PASSPHRASE=1234 target/debug/pwm.exe wallet account list --wallet tmp/genesis.yaml` - pass.

## Cleanup

No `pwmd` or `pwm-tui` process was started for this slice. Final process check found none running.

