# Sprint 14 Slice27 Review

## Verdict
`request changes`

## Finding
Load/runtime behavior no longer requires `active_account_id_hex`, but merge-save can still preserve a legacy `active_account_id_hex` key from old v3 YAML files.

`save_wallet_yaml_v3_merge()` starts from the existing YAML map and overlays serialized v3 data. Because `active_account_id_hex: None` is skipped during serialization, old files can keep the key after account add/remove style rewrites.

## Required Fix
- Remove `active_account_id_hex` during v3 merge-save/write paths.
- Add a regression: legacy v3 with `active_account_id_hex` followed by a write operation saves YAML without that key.
