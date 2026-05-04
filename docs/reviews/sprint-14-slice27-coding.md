# Sprint 14 Slice 27 — Coding

## What changed

- Wallet schema v3 read paths now accept files without `active_account_id_hex`.
- New v3 wallet writes omit the wallet-level active marker.
- CLI/TUI select signing accounts at runtime from `accounts[]`; when CLI has no explicit selector, it uses the deterministic first account by `(derivation_index, id_hex)`.
- `wallet account use` is retained only as a deprecated validator for account ids and no longer writes an authoritative active marker.

## What floated

The floating requirement was the schema/read-path dependency on wallet-level `active_account_id_hex`. That field was a UX/default marker, not a cryptographic link.

The cryptographic link remains anchored in each `accounts[]` record: `derivation_index` / `derivation_path`, account metadata, and the wallet master seed or decrypted seed payload. Signing derives/verifies the selected account from that metadata. If the seed or derivation metadata cannot prove the selected account, signing is blocked with a clear error.

## Checks to run

- `cargo fmt`
- Targeted wallet / CLI / TUI tests for v3 load without `active_account_id_hex`, deterministic CLI default, and TUI selected Owner signing.
