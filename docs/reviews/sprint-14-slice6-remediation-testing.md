# Sprint 14 — Slice 6 remediation testing

Date: 2026-04-28  
Repo: `P:/opt/docker/PWM-cryptocurrency`

## Scope

Validate remediation after coding:

1. Create-path is strict overwrite (`wallet init`, `wallet import-seed`, `addr-bruteforce`) and does not inherit stale fields.
2. Upgrade path persistence cleans legacy fields.
3. Account update paths preserve intended merge behavior.

## Commands and results

1. `cargo test -p pwm-cli save_new_wallet_yaml_v3_overwrites_existing_file_without_legacy_baggage`  
   **PASS** (`1 passed / 0 failed`)
2. `cargo test -p pwm-cli upgrade_wallet_persistence_drops_legacy_and_unknown_top_level_fields`  
   **PASS** (`1 passed / 0 failed`)
3. `cargo test -p pwm-cli wallet_v3_account_rewrite_preserves_unknown_and_created_metadata`  
   **PASS** (`1 passed / 0 failed`)
4. `cargo test -p pwm-cli`  
   **PASS** (`109 passed / 0 failed / 0 ignored`)

## Evidence

- **Create-path strict overwrite**  
  CLI create flows for `addr-bruteforce`, `wallet init`, and `wallet import-seed` call `save_new_wallet_yaml_v3(...)` in `crates/pwm-cli/src/main.rs`.  
  Targeted test `wallet::tests::save_new_wallet_yaml_v3_overwrites_existing_file_without_legacy_baggage` confirms overwrite removes legacy/unknown top-level fields (including injected `future_raw_key`) instead of inheriting stale payload.

- **Upgrade persistence cleanup**  
  `wallet::tests::upgrade_wallet_persistence_drops_legacy_and_unknown_top_level_fields` verifies `load_wallet_yaml_with_upgrade(..., true)` persists cleaned v3 content and removes legacy keys (`derivation_index`, `account_id_human`) and unknown baggage (`future_raw_key`).

- **Update-path merge preservation**  
  `wallet::tests::wallet_v3_account_rewrite_preserves_unknown_and_created_metadata` verifies account update operations (`wallet_account_add`, `wallet_account_use`) keep intended merge semantics for v3 files by preserving existing metadata keys (`wallet_created_at_unix_sec`, `future_raw_key`).

## Verdict

Slice 6 remediation validation is **PASS**.

- Create-path behavior is strict overwrite with no stale-field inheritance.
- Upgrade persistence cleans legacy/unknown baggage on write.
- Account update paths keep intended merge-preserve behavior.
