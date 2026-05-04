# Sprint 15 / S3.1 testing: TUI staged cross-shard flow diagnostics

Дата проверки: 2026-04-29
Область: `crates/pwm-tui` (F6 cross-shard staged diagnostics)

## Проверяемые требования

1. TUI делает preflight перед roaming submit.
2. staged report явно показывает шаг падения и recovery hint (включая `missing_preflight`-style contracts).
3. Виден хотя бы один success-like staged path.
4. Нет регрессии в существующих тестах `pwm-tui`.

## Что проверено

### 1) Preflight перед roaming submit — PASS

- В `submit_roaming_intent(...)` сначала вызывается `POST /v1/export-readiness`, и только при success выполняется `POST /v1/roaming-intents`.
- На неуспешном preflight функция сразу возвращает staged fail (`xflow_preflight_fail(...)`) и не идёт в submit.
- Подтверждающий код: `crates/pwm-tui/src/main.rs` (`submit_roaming_intent`, `xflow_preflight_fail`).

### 2) Явный шаг падения + hint (включая `missing_preflight`) — PASS

- `xflow_export_fail(...)` формирует структурированный отчёт:
  - `1) preflight ...: OK`
  - `2) export submit ...: FAIL - ...`
  - включает `code` и `hint`, если backend вернул JSON reject (`code/hint/message`).
- Тест `f6_send_roaming_lifecycle_shows_missing_preflight_stage_hint` проверяет:
  - наличие `1) preflight`,
  - наличие `2) export submit`,
  - наличие `missing_preflight`,
  - наличие хинта `Run /v1/export-readiness`.

### 3) Наличие success-like staged path — PASS

- Тест `f6_send_roaming_lifecycle_duplicate_to_imported` проверяет staged-вывод с 4 шагами и успешным завершением:
  - `1) preflight`
  - `2) export submit`
  - `3) handoff/provenance`
  - `4) import submit`
  - `status=imported`
- Это подтверждает видимость полноценного "успешного" staged flow в диагностике.

### 4) Регрессии по `pwm-tui` тестам — PASS

Запуск:

- `cargo test -p pwm-tui`

Результат:

- `85 passed; 0 failed; 0 ignored`
- В том числе прошли релевантные тесты:
  - `f6_send_roaming_lifecycle_duplicate_to_imported`
  - `f6_send_roaming_lifecycle_shows_missing_preflight_stage_hint`
  - `f6_send_roaming_lifecycle_rejects_invalid_request`
  - `f6_send_roaming_lifecycle_handles_expired_status`

## Итоговый вердикт

**PASS**

S15-S3.1 TUI staged cross-shard flow diagnostics подтверждён: preflight-before-submit активен, failure-stage + hint (включая `missing_preflight`) явно отображаются, success-like staged path присутствует, регрессий в `pwm-tui` тестах не обнаружено.
