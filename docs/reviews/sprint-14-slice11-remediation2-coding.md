# Sprint 14 — Slice 11 remediation 2 (coding)

Дата: 2026-04-28

## Что исправлено

- В `pwm-cli genesis-build` устранён generation-side дефект: выходной v4 bundle теперь всегда удовлетворяет инварианту `validators.set[*].acct_hex in funding.rows[*].acct_hex`.
- Добавлен deterministic guard в сборке:
  - если validator account отсутствует в funding rows, автоматически добавляется funding row с тем же `acct_hex/pubkey_hex/der_idx` и `bal=0`.
- Поведение минимально-инвазивное: существующие funding rows не модифицируются, новые rows добавляются только при отсутствии совпадения по `acct_hex`.

## Тесты

- Обновлён `genesis_build_generates_decoupled_v4_bundle`: теперь проверяет наличие validator account в funding rows и `bal=0` для добавленной строки.
- Добавлен регрессионный тест `genesis_build_adds_zero_balance_row_for_missing_validator_account` для точного сценария missing validator funding account.

## Изменённые файлы

- `crates/pwm-cli/src/main.rs`
- `docs/pwm-cli.md`
- `issues-report.md`
- `docs/reviews/sprint-14-slice11-remediation2-coding.md`
