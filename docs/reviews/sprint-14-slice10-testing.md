# Sprint 14 — Slice 10 testing

Source: focused `pwm-testing` validation run for `pwmd` and `pwm-cli`.

## Verdict

`approve`

Slice 10 hardening checks passed in focused test/runtime scenarios: schema gating to v3 works, genesis output contains encrypted validator material, passphrase flows behave as expected, and negative paths are rejected.

## Scope checked

1. `pwmd` accepts only `schema_version=3` for `--genesis-file` and rejects old format.
2. `pwm-cli genesis-build` emits encrypted validator keys (no plaintext seed fields).
3. Passphrase flow works for `pwm-cli` and `pwmd` (flag/env), and missing passphrase in non-tty fails.
4. Negative cases: wrong passphrase, malformed encrypted payload, derivation path mismatch.

## Commands and results

### 1) pwmd schema_version=3 only + negative genesis cases

Command:

`cargo test -p pwmd genesis_json_ -- --nocapture`

Result: **PASS** (`5 passed`)
- `genesis_json_rejects_unsupported_schema_version`
- `genesis_json_v3_roundtrip_encrypted_validator_key`
- `genesis_json_v3_rejects_wrong_passphrase`
- `genesis_json_v3_rejects_malformed_payload`
- `genesis_json_v3_rejects_path_mismatch`

### 2) pwm-cli genesis-build encrypted validator keys

Commands:

- `cargo run -q -p pwm-cli -- wallet init --country CY --wallet-out tmp/slice10-test/wallet.yaml --wallet-passphrase wallet-pass`
- `cargo run -q -p pwm-cli -- genesis-build --wallet tmp/slice10-test/wallet.yaml --wallet-passphrase wallet-pass --genesis-passphrase gen-pass --out tmp/slice10-test/genesis.json`

Artifact check:

- `schema_version` in output JSON is `3`.
- `validator_keys[*].enc_seed` contains `kdf` + `aead.ciphertext_b64`.
- Plaintext seed-like fields (`seed_hex`, `master_seed`, `plaintext_seed`, `validator_seed`) are absent.

Result: **PASS**

### 3) passphrase flow (flag/env + non-tty missing pass)

`pwm-cli`:

- Missing passphrase (non-tty):
  - `cargo run -q -p pwm-cli -- genesis-build --wallet tmp/slice10-test/wallet.yaml --wallet-passphrase wallet-pass --out tmp/slice10-test/genesis-missing-pass.json`
  - Result: **FAILS as expected**, exit code `2`, message: `missing genesis passphrase...`
- Env passphrase:
  - `PWM_GENESIS_PASSPHRASE=gen-pass-env` + same `genesis-build` command
  - Result: **PASS**, exit code `0`, genesis file emitted.

`pwmd`:

- Missing passphrase (non-tty):
  - `cargo run -q -p pwmd -- --genesis-file tmp/slice10-test/genesis.json`
  - Result: **FAILS as expected**, exit code `2`, message: `missing genesis passphrase in non-tty mode...`
- Flag/env positive path (startup smoke with timeout):
  - with `--genesis-passphrase gen-pass`
  - with `PWM_GENESIS_PASSPHRASE=gen-pass`
  - Result: process passed passphrase gate and started (timed out intentionally), no passphrase validation error.

Overall result: **PASS**

### 4) negative cases requested

Covered by focused tests:

- Wrong passphrase:
  - `pwmd::tests::genesis_json_v3_rejects_wrong_passphrase`
  - `pwm-cli::wallet::tests::backup_wallet_file_rejects_wrong_passphrase_for_encrypted`
- Malformed encrypted payload:
  - `pwmd::tests::genesis_json_v3_rejects_malformed_payload`
  - `pwm-cli::wallet::tests::backup_wallet_file_rejects_corrupted_encrypted_payload`
- Path mismatch:
  - `pwmd::tests::genesis_json_v3_rejects_path_mismatch`

Result: **PASS**

## Focused pwm-cli tests run

Commands:

- `cargo test -p pwm-cli genesis_build_generates_rows_from_wallet_accounts -- --nocapture`
- `cargo test -p pwm-cli wallet_account_add_encrypted_v3_requires_passphrase -- --nocapture`
- `cargo test -p pwm-cli backup_wallet_file_rejects_wrong_passphrase_for_encrypted -- --nocapture`
- `cargo test -p pwm-cli backup_wallet_file_rejects_corrupted_encrypted_payload -- --nocapture`

Result: **PASS** (`4/4` focused tests passed)
