# Sprint 14 — Address generation checkup (coding report)

## What was implemented

- `addr-bruteforce` now uses safe add semantics by default for existing `--wallet-out`:
  - existing wallet: append account (no destructive replace);
  - missing wallet: create new wallet;
  - explicit destructive path is opt-in via `--overwrite-wallet`.
- Added dedicated `wallet account remove` command:
  - rejects removing the last account;
  - when removing active account, switches active deterministically to the smallest `derivation_index` (tie-break: smallest `id_hex`).
- `addr-derive` now supports optional wallet write path:
  - `--wallet-out` provided: create-or-append wallet using same safe add semantics;
  - `--wallet-out` omitted: remains stateless stdout-only flow.
- Help/UX updated:
  - passphrase/write behavior clarified;
  - plaintext warning shown only when creating/overwriting wallet, not when appending to existing wallet.

## Implementation notes

- Reused v3 account operations via shared wallet add path from master seed (`wallet_account_add_with_seed`).
- Added shared CLI helper for wallet persistence mode selection (`created` / `appended` / `overwritten`) to keep write semantics consistent between `addr-derive` and `addr-bruteforce`.

## Tests added/updated

- CLI parsing:
  - `addr-derive` with/without `--wallet-out`;
  - `addr-bruteforce --overwrite-wallet`;
  - `wallet account remove`.
- Behavior:
  - `addr-bruteforce` existing wallet appends by default.
  - `addr-derive --wallet-out` creates wallet when missing.
  - `wallet account remove` guardrails (cannot remove last account, active fallback behavior).
