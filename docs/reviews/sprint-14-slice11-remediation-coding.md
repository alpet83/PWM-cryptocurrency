# Sprint 14 — Slice 11 remediation (coding)

Дата: 2026-04-28

## Что исправлено

- Устранена high-risk регрессия reward semantics: при `reward_policy=to_producer_account` больше нет silent reward-loss, если producer account отсутствует в funding state.
- Принята политика **fail-fast invariant** (минимальный риск): аккаунт каждого валидатора из `validators.set[*].acct` обязан существовать в `funding.rows[*].acct`.
- В `Chain::boot` добавлены явные инварианты:
  - `rows` обязан зеркалить `funding.rows`;
  - каждый validator account обязан присутствовать в funding rows.
- В `Chain::seal` добавлен runtime guard перед начислением reward: при нарушении инварианта block sealing завершается ошибкой с явным текстом.
- Снижен риск divergence `rows` vs `funding.rows` в затронутых путях:
  - добавлен строгий check в `Chain::boot`;
  - `snapshot_genesis_rows` переведён на источник `cfg.funding.rows` (без неоднозначного `cfg.rows`).

## Тесты

Добавлен новый тест:
- `chain::tests::boot_rejects_missing_validator_funding_account` (гарантирует fail-fast при отсутствии validator account в funding rows).

Проверены и остаются зелёными decoupled сценарии:
- `chain::tests::seal_allows_one_val_many_funding`
- `chain::tests::prod_rotation_uses_vals_len`
- `chain::tests::reward_default_is_deterministic`

## Документация

- `docs/MVP-checklist.md`: синхронизирован контракт на `schema v4` (вместо устаревшего `v3` упоминания в пункте `--genesis-file`).
- `docs/GENESIS_BLOCK.md`: добавлен явный reward-инвариант и обновлён pre-launch checklist под `v4` + проверка присутствия validator account в funding rows.

## Изменённые файлы

- `crates/pwm-core/src/chain.rs`
- `crates/pwmd/src/snapshot.rs`
- `docs/MVP-checklist.md`
- `docs/GENESIS_BLOCK.md`
- `issues-report.md`
- `docs/reviews/sprint-14-slice11-remediation-coding.md`

## Команды и результаты

- `cargo fmt` -> OK
- `cargo test -p pwm-core chain::tests::reward_default_is_deterministic` -> OK
- `cargo test -p pwm-core chain::tests::boot_rejects_missing_validator_funding_account` -> OK
- `cargo test -p pwm-core chain::tests::seal_allows_one_val_many_funding` -> OK
- `cargo test -p pwm-core chain::tests::prod_rotation_uses_vals_len` -> OK
- `cargo test -p pwmd genesis_json_v4_roundtrip_encrypted_validator_key` -> OK
- `cargo test -p pwm-cli genesis_build_generates_decoupled_v4_bundle` -> OK
