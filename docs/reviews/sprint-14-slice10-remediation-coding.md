# Sprint 14 — Slice 10 remediation (coding)

- Убраны/помечены legacy-ссылки в `docs/MVP-checklist.md`: активный genesis flow теперь явно зафиксирован как `pwm genesis-build` schema v3 (`validator_keys[*].enc_seed`), а `validator_seeds_hex`/`m/0'/0'` отмечены как устаревшие.
- В `docs/genesis_bundle_from_seed.ps1` добавлен явный `OBSOLETE`-баннер и предупреждения о том, что скрипт только для исторического fallback, не для production genesis.
- В `crates/pwmd/src/snapshot.rs` добавлен safety cap для `validator_keys[*].enc_seed.kdf.iters` (fast-fail на экстремальных значениях до decrypt path).
- В `crates/pwmd/src/lib.rs` добавлен тест `genesis_json_v3_rejects_extreme_kdf_iters`, подтверждающий отказ при завышенном `kdf.iters`.
- Production-константа в `crates/pwm-cli/src/main.rs` переименована из `GENESIS_VALIDATOR_DER_PATH_IDX` в `GENESIS_DER_PATH_IDX` (лимит <= 4 words).
