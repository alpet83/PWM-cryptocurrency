# Sprint 14 — Slice 6 remediation review

## Scope recap
- Проверен remediation-объём из `tasks/20260428-s14-slice6-remediation-clean-overwrite.json`:
  - create-path пишет чистый `v3` без merge-наследования stale/unknown/sensitive полей;
  - upgrade persistence очищает legacy/unknown поля;
  - merge-поведение остаётся только для intended update-path.

## Requirement fit
- **Create-path isolation: PASS** — `wallet init`, `wallet import-seed`, `addr-bruteforce` сохраняют через strict `save_new_wallet_yaml_v3(...)`.
- **Upgrade cleanup: PASS** — `load_wallet_yaml_with_upgrade(..., upgrade_wallet=true)` персистит через strict `save_wallet_yaml_v3_strict(...)`.
- **Merge retained only for update-path: PASS** — merge-сохранение осталось только в `wallet_account_add` и `wallet_account_use` через `save_wallet_yaml_v3_merge(...)`.

## Tests and evidence
- `save_new_wallet_yaml_v3_overwrites_existing_file_without_legacy_baggage` — PASS
- `upgrade_wallet_persistence_drops_legacy_and_unknown_top_level_fields` — PASS
- `wallet_v3_account_rewrite_preserves_unknown_and_created_metadata` — PASS
- `cargo test -p pwm-cli` — PASS (`109 passed`, `0 failed`)

## Residual risk
- Низкий: create-path strict save использует `fs::write` (не `write_atomic`), что не возвращает merge-риск, но оставляет меньшую crash-safety именно на этапе create-write.

## Verdict
**pass** — blocker Slice 6 remediation закрыт.
