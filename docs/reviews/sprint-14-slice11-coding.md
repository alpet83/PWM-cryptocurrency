# Sprint 14 — Slice 11 coding report

## Scope delivered

- Implemented genesis `schema_version=4` with decoupled sections:
  - `gen_cfg.funding.rows`
  - `gen_cfg.validators.set`
  - `gen_cfg.reward_policy.mode` (default `to_producer_account`)
- Removed runtime coupling `validators.len == funding.rows.len`.
- Switched producer rotation and block signature verification to validator set.
- Kept reward default deterministic via producer account routing.

## Runtime/core changes

- `pwm-core`:
  - `GenCfg` now carries separate `funding` and `vals` sections plus `rew`.
  - `Chain::boot` validates keys against `vals.set` and requires non-empty validator set.
  - `Chain::seal` producer index/signature path now uses `vals.set`.
- Added focused tests:
  - `seal_allows_one_val_many_funding`
  - `prod_rotation_uses_vals_len`
  - `reward_default_is_deterministic`

## Loader/bootstrap changes (`pwmd`)

- `load_genesis_bundle` now supports only `schema_version=4` (one-way break).
- Parser/validator now checks:
  - non-empty `gen_cfg.validators.set`
  - `validator_keys.len == validators.set.len`
  - derived key/account matches `validators.set[i]`
- Error messages updated to explicit v4 contract.

## CLI changes (`pwm-cli`)

- `genesis-build` now emits v4 decoupled schema only.
- Added `--val-id` (optional); if omitted, active wallet account is used as validator source.
- Supports practical default flow: many funding rows from wallet + single validator set entry.

## Docs updated

- `docs/GENESIS_BLOCK.md`
- `docs/pwmd.md`
- `docs/pwm-cli.md`

## Validation executed

- `cargo fmt`
- `cargo check -p pwm-core -p pwmd -p pwm-cli`
- Focused tests:
  - `cargo test -p pwm-core seal_allows_one_val_many_funding`
  - `cargo test -p pwm-core prod_rotation_uses_vals_len`
  - `cargo test -p pwm-core reward_default_is_deterministic`
  - `cargo test -p pwmd genesis_json_v4_roundtrip_encrypted_validator_key`
  - `cargo test -p pwmd genesis_json_v4_rejects_wrong_passphrase`
  - `cargo test -p pwmd genesis_json_v4_rejects_path_mismatch`
  - `cargo test -p pwm-cli genesis_build_generates_decoupled_v4_bundle`

All listed commands passed.
