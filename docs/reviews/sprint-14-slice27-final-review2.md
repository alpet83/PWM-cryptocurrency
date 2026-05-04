# Sprint 14 Slice27 Final Review2

## Verdict
`approve`

## Summary
Wallet-level `active_account_id_hex` is no longer required or authoritative.

- v3 wallets without the field load.
- Fresh and merge-save v3 writes remove/omit the legacy top-level key.
- Old files with the key still load for compatibility.
- Runtime selection comes from `accounts[]` and deterministic/default selection, while TUI uses the selected Owner row.

The prior helper-name style blocker was fixed (`save_v3_merge`, `ser_v3_clean`, `v3_clean_value`).

## Testing Note
The previously hanging wallet/address-book test passed in isolation under timeout and is classified as an existing long-running test, not a Slice27 regression.
