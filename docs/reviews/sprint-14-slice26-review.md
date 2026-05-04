# Sprint 14 Slice26 Review

## Verdict
`request changes`

## Finding
TUI F6 now uses the selected Owner row, but selected active wallet account can still use the unlocked active `signing_key` shortcut without verifying that the key and derivation metadata actually produce the selected account id.

This can preserve the original bad shape if wallet header says active CY while decrypted payload key belongs to DO.

## Required Fix
- Active and non-active wallet v3 signing must share the same derive/verify invariant, or the active shortcut must verify `signing_key + derivation_index -> selected account`.
- On mismatch, TUI must block before submit with a clear selected-owner signing error.
- Add a regression for active CY selected with mismatched DO payload key.
