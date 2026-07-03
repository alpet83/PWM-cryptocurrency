# Sprint 14 — Slice 6 testing (independent)

Date: 2026-04-28  
Repo: `P:/opt/docker/pwm-protocol`

## Scope

Independent testing for Slice 6 behaviors:

1. v3-by-default write path for `wallet init` / `wallet import-seed` / `addr-bruteforce` output.
2. User-visible naming change to `id_pretty` where applicable.
3. `addr-bruteforce` resume from existing wallet metadata.

## Executed commands

1. `cargo test -p pwm-cli`  
   Result: **PASS**  
   Duration: ~53.27s  
   Totals from run: **107 passed / 0 failed / 0 ignored**

## Coverage verification (new/changed tests)

### 1) v3-by-default write path

- Observed in regression run:
  - `wallet::tests::load_wallet_yaml_parses_schema_v3_plaintext_minimal`
  - `wallet::tests::wallet_yaml_roundtrip`
  - `wallet::tests::wallet_account_add_derives_and_persists_new_account`
- Why this is sufficient:
  - tests validate persisted v3 structure (`schema_version: 3`, `accounts[]`, `active_account_id_hex`) on create/write flows used by Slice 6 paths.
  - the runtime write path in `main.rs` for `wallet init`, `wallet import-seed`, `addr-bruteforce` uses `save_new_wallet_yaml_v3(...)`; this is covered by the same serializer/persistence contract validated above.

### 2) user-visible `id_pretty`

- Observed in regression run:
  - `tests::wallet_show_lines_redact_secrets_by_default`
  - `wallet::tests::load_wallet_yaml_parses_schema_v3_plaintext_minimal`
  - `wallet::tests::load_wallet_yaml_uses_truth_source_when_cached_ids_mismatch`
- Assertions covered:
  - user-facing output contains `id_pretty`
  - legacy naming `account_id_human` is not used in user-facing output paths where Slice 6 changed terminology.

### 3) `addr-bruteforce` resume from wallet metadata

- Observed in regression run:
  - `tests::addr_bruteforce_resume_start_index_uses_existing_wallet_metadata`
  - `wallet::tests::load_wallet_resume_start_index_for_v3_uses_max_account_index`
- Assertions covered:
  - resume start index is derived from existing wallet metadata (`max_derivation_index + 1`)
  - no restart from zero when wallet already has derived accounts.

## Gap analysis

- Obvious testing gap for Slice 6 in `pwm-cli` was not found after full crate regression + targeted test-name verification.
- Additional tests were **not added** in this pass.

## Bugs / regressions found

- **None observed** in this independent Slice 6 testing pass.

## Pass/fail summary

- Executed tests in this pass: **107 passed / 0 failed**.
- Target Slice 6 status: **PASS**.
