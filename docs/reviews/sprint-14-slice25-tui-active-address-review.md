# Sprint 14 Slice25 Review: TUI Active Address

## Verdict
`request changes`

## Finding
`active_account_id_hex` is valid as a wallet/CLI default, but it should not be authoritative for TUI runtime transaction source.

Current TUI wallet mode can show one Owner row as selected while F6/signing still uses the persisted active wallet account flattened into the wallet header. This can explain CY selected in the panel while the signed transaction sender has `domain_hi=0xDB`.

## Recommendation
- Keep `active_account_id_hex` as CLI default and optional startup highlight.
- TUI F6 must use the selected Owner row as sender.
- If the selected row cannot be signed with current wallet material, block with a clear message.
- Add regression tests for multi-account wallet selection where persisted active differs from selected Owner.

## Remediation Plan
1. F6 form creation uses `owner_rows[owner_sel]` for `from`.
2. Wallet signing derives/loads signing material for the selected account, not only active account.
3. Docs state that TUI source is runtime Owner selection.
