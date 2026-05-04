# Sprint 14 - Slice 11 remediation testing

Дата: 2026-04-28

## Scope

Проверка ремедиации по трём критериям:
1. missing validator funding account now fails explicitly (no silent reward loss);
2. existing decoupled positive cases remain green;
3. docs/contract references are consistent with v4 in touched files.

## Executed targeted tests

- `cargo test -p pwm-core chain::tests::boot_rejects_missing_validator_funding_account` -> PASS
- `cargo test -p pwm-core chain::tests::seal_allows_one_val_many_funding` -> PASS
- `cargo test -p pwm-core chain::tests::prod_rotation_uses_vals_len` -> PASS
- `cargo test -p pwm-core chain::tests::reward_default_is_deterministic` -> PASS
- `cargo test -p pwmd genesis_json_v4_roundtrip_encrypted_validator_key` -> PASS
- `cargo test -p pwm-cli genesis_build_generates_decoupled_v4_bundle` -> PASS

Запуски выполнены через `cq_process_ctl` (host mode), все процессы завершились `returncode=0`.

## Validation notes

### 1) Explicit failure on missing validator funding account

Подтверждено:
- в `pwm-core` есть fail-fast test `boot_rejects_missing_validator_funding_account`;
- тест проходит и фиксирует явный отказ при отсутствии `validators.set[*].acct` в `funding.rows`.

Вердикт: **PASS**.

### 2) Existing decoupled positive paths stay green

Подтверждено зелёными таргетными тестами:
- `seal_allows_one_val_many_funding` (1 validator + N funding rows);
- `prod_rotation_uses_vals_len`;
- `reward_default_is_deterministic`;
- `genesis_json_v4_roundtrip_encrypted_validator_key`;
- `genesis_build_generates_decoupled_v4_bundle`.

Вердикт: **PASS**.

### 3) v4 consistency in touched docs/contracts

Проверены ключевые затронутые файлы:
- `docs/GENESIS_BLOCK.md` — v4-only формулировки подтверждены;
- `docs/MVP-checklist.md` — пункт по `--genesis-file` синхронизирован с v4.

Найдено одно остаточное упоминание legacy v3 в `docs/MVP-checklist.md` (историческая строка про genesis flow), что формально нарушает строгую "v4-only consistency" в пределах файла.

Вердикт: **PARTIAL / NIT** (функционально не блокирует ремедиацию, но требует doc-cleanup).

## Overall verdict

- Runtime remediation behavior: **OK**
- Targeted tests (`pwm-core` / `pwmd` / `pwm-cli`): **GREEN**
- Documentation contract consistency with strict v4 wording: **minor follow-up needed** (single legacy v3 reference in checklist).
