# Sprint 14 Slice26 Coding: TUI runtime owner selection/signing

## Changes

- TUI `F6` now uses the selected `Owner` row as the fixed `from` value instead of the flattened wallet active header.
- Wallet signing now resolves material for the selected owner account. For wallet v3, TUI loads per-account derivation metadata and derives the selected account key from the available master seed or unlocked decrypted payload.
- Sends are blocked before submit when the selected owner cannot be signed, including locked encrypted wallets, missing unlocked master seed, and owner accounts not present in the wallet.
- `active_account_id_hex` remains wallet/CLI default metadata; docs now state that TUI runtime `Owner` selection is authoritative for sender/signing.

## Regression Coverage

- Added a two-owner wallet regression where active != selected and `F6` opens with the selected owner as sender.
- Added a signing rejection regression for selected owner without signing material.
- Added a CY-vs-DB regression proving selected CY owner derives/signs CY material even when DB is active.

## Tests

- `cargo fmt` - pass
- `cargo test -p pwm-tui` - pass, 79 tests
- `cargo check -p pwm-tui` - pass

No CLI/wallet crate code was changed in this slice, so no separate `pwm-cli` wallet tests were run.
