# Sprint 14 Slice 28: TUI signing mismatch investigation

## Summary

User report is reproducible from the current wallet fixture shape in `tmp/genesis.yaml`.

`tmp/genesis.yaml` is schema v3 encrypted and contains two accounts:

- DO: `m/0/583600`, `32ecaa3884011f2c21bf09b05e835ec1df5545bebb2c6c478dcacfb70e7fc1c5`
- CY: `m/0/105053`, `2cfb1e1d7001d108b39e05b194f2d1b126931bbfef38506e34297a5474ddae5e`

The CY row matches the user-provided address:

`pwm1-CY/FB-f1E1D7001-td108b39e05b194f2d1b126931bbfef38506e34297a5474ddae5e`

## Evidence

CLI with `PWM_WALLET_PASSPHRASE=1234` agrees that the wallet default metadata points at CY `m/0/105053`:

```text
schema_version 3
wallet_mode encrypted
derivation_index 105053
derivation_path m/0/105053
domain_u16 11515
account_id_hex 2cfb1e1d7001d108b39e05b194f2d1b126931bbfef38506e34297a5474ddae5e
id_pretty pwm1-CY/FB-f1E1D7001-td108b39e05b194f2d1b126931bbfef38506e34297a5474ddae5e
```

But the decrypted encrypted payload still contains the DO signing key:

```text
payload signing_key_hex ecdbcc75d504baf2a306c910a804eaaa0e9bfc169ce321bb015ab6eead4ff7f8
payload verifying_key_hex 5663f2bd38abbad8f63d3ae2036a793aec5e5b3703879f9dcb1862c56d112e2f
```

Deriving from the same master seed confirms:

```text
m/0/583600 -> DO/EC, signing_key_hex ecdbcc75d504baf2a306c910a804eaaa0e9bfc169ce321bb015ab6eead4ff7f8
m/0/105053 -> CY/FB, signing_key_hex 0c8ba03ea8b5262c2000ebe2fe3f7818c258f52b5b838b1c64a9c69584e6e46a
```

So the master seed is correct and can derive CY; the single flattened `signing_key_hex` inside the encrypted payload is stale and belongs to DO.

## Root Cause

There are two related issues.

First, TUI still has an authoritative active marker path for schema v3:

- `pwm-core/src/wallet_read.rs` maps schema v3 to `WalletReadHeader` by choosing `default_v3_account()`.
- `pwm-tui/src/main.rs` marks `OwnedWalletAccount.is_active` by comparing each account to that `WalletReadHeader.account_id`.
- `owner_and_receivers()` returns `active_owner_idx`, and render code prefixes that row with `*`.

This is why the Owner panel still shows one `*` even though wallet-level active account should be removed/non-authoritative after Slice27.

Second, TUI signing has a stale flattened-key shortcut:

- `try_decrypt_wallet_secret_payload()` extracts one `signing_key_hex` from encrypted payload into `w.signing_key`.
- `signing_material_for_sender()` uses `verify_wallet_key(w.signing_key, ...)` directly when the selected sender equals `w.account_id`.
- With current fixture, `w.account_id` is CY `m/0/105053`, but `w.signing_key` is DO `m/0/583600`, producing:

```text
selected owner cannot be signed: signing key for m/0/105053 does not match selected account
```

This is not caused by `m/0/105053` vs hardened-path parsing. The implemented parser accepts only `m/0/<index>`, and CLI derivation for `m/0/105053` matches the reported CY address. It is also not a CY/FB low-byte/domain rendering bug: `domain_u16=11515` is `0x2CFB`, matching the pretty label `CY/FB`.

## CLI Comparison

CLI does not hit the same mismatch in its normal tx signer path. `load_sender_from_wallet()` unlocks wallet secrets, reads `master_seed_hex`, derives `m/0/<wallet.derivation_index>`, and verifies the derived account id. For this fixture that path derives CY successfully.

The account-list CLI still prints `*` for CY:

```text
  id_hex=32ecaa3884011f2c21bf09b05e835ec1df5545bebb2c6c478dcacfb70e7fc1c5 ... derivation_index=583600
* id_hex=2cfb1e1d7001d108b39e05b194f2d1b126931bbfef38506e34297a5474ddae5e ... derivation_index=105053
```

That marker is the same legacy/default-account concept, not proof of a real user-selected active account.

## Suggested Fix

1. Remove schema v3 active/default authority from UI semantics.
   - Do not render `*` for schema v3 wallet accounts.
   - Prefer an optional neutral label like `wallet`/`owned`, while runtime `owner_sel` remains the authoritative sender for F6.

2. In TUI wallet signing, derive from `master_seed_hex` for all schema v3 owned accounts, including the default/header account.
   - Treat decrypted `signing_key_hex` as legacy/compatibility material only.
   - If `master_seed_hex` exists, use `derive_wallet_key(seed, selected.derivation_index, selected.id)`.
   - Only fall back to flattened `signing_key_hex` for legacy wallets without a master seed.

3. Align CLI `wallet account list` marker with Slice27 semantics.
   - Either remove `*` for schema v3, or rename it in output/docs as deterministic display default rather than active account.

4. Add regression coverage:
   - Encrypted schema v3 wallet with two accounts, master seed for both, and stale flattened `signing_key_hex` for another account must still sign selected CY via master seed.
   - Owner panel state builder must not expose an active marker for schema v3 accounts after active-account removal.

## Commands Run

- `cargo run -q -p pwm-cli -- wallet show --wallet tmp/genesis.yaml --unsafe-show-secrets` with `PWM_WALLET_PASSPHRASE=1234`: passed.
- `cargo run -q -p pwm-cli -- wallet account list --wallet tmp/genesis.yaml`: passed.
- `cargo run -q -p pwm-cli -- wallet import-seed --master <fixture-seed> --derivation-index 105053 --wallet-out <temp> --plaintext-dev`: passed; temp file removed.
- `cargo run -q -p pwm-cli -- wallet import-seed --master <fixture-seed> --derivation-index 583600/105053 --wallet-out <temp> --plaintext-dev`: passed; temp files removed.

`cq_process_ctl` could not spawn because its host process registry was already full of completed entries (`Too many host processes (max 48)`), so CLI checks were run through local shell fallback.

Cleanup: no `pwmd` or `pwm-tui` process was started by this investigation.
