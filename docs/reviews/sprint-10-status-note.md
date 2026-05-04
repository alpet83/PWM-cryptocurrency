# Sprint 10 Status Note

Дата: 2026-04-26  
Этап: Sprint 10 completed (slices 0..6; orchestrator-confirmed closeout)  
Статус: **SPRINT 10 CLOSED — HANDOFF TO SPRINT 11 (OPTIMIZATION BACKLOG)**

## Slice 0 Baseline State

- Scope Sprint 10 зафиксирован: hardening / reliability / conformance / MVP cut.
- Non-goals зафиксированы: без feature expansion и без EXPORT/IMPORT до core.
- Handoff из Sprint 9 перенесён без искажений: EXPORT/IMPORT остаётся deferred до `pwm-core`.
- Созданы и синхронизированы артефакты: checklist/status/review/test.

## Slice 1 Operator Reliability (coding-pass)

- **pwm-cli:** HTTP client с connect/request timeout (`PWM_CLI_RPC_TIMEOUT_MS`, default 10s, max 120s); `fetch_nonce` и submit tx больше не дают «тихий» nonce `0` и не паникуют на сетевых ошибках — сообщения с HTTP status/body snippet и подсказками по `--rpc`/`PWM_RPC`; парсинг `nonce` как число или десятичная строка (как в TUI).
- **pwm-tui:** `fetch_nonce` / разбор ответа аккаунта: при не-2xx или невалидном JSON/`nonce` возвращается ошибка вместо подстановки `0`; уточнены сообщения при connect failure.

## Slice 2 Conformance (docs vs runtime, coding-pass)

- Обновлены operator guides: добавлены фактические timeout-параметры runtime (`PWM_CLI_RPC_TIMEOUT_MS`: default 10000 ms, max 120000 ms) и зафиксировано различие env timeout между CLI и TUI (`PWM_CLI_RPC_TIMEOUT_MS` vs `PWM_TUI_RPC_TIMEOUT_MS`).
- В сценариях тестера закреплено текущее поведение nonce/submit: при HTTP/JSON ошибке нет silent `nonce=0`, команды завершаются явной ошибкой.
- Обновлены Sprint 10 артефакты (checklist/status/test/review) и evidence-гейты для Slice 2.

## Current Gates (Slice 2)

- Coding gate (`cargo fmt --check`): **PASS**
- Docs-only smoke gate: `cargo check -p pwm-cli`: **PASS**
- MVP cut validation gate (scope/no expansion): **PASS**
- Wire/API `pwmd`, `pwm-core` контракты: **не менялись** (docs-only в Slice 2)

Примечание continuity: Slice 4 — stabilization wrap; полная test matrix остаётся зоной testing-pass.

## Slice 4 Stabilization Wrap (coding-pass)

- **pwm-cli:** fallback при ошибке сборки HTTP client больше не «тихий»: выводится одноразовое operator-facing предупреждение, что выполнен переход на reqwest defaults и timeout-поведение может отличаться; явная ссылка на `PWM_CLI_RPC_TIMEOUT_MS`.
- **pwm-tui:** аналогичный одноразовый fallback warning для HTTP client (`PWM_TUI_RPC_TIMEOUT_MS`) + timeout-сообщения в send/nonce paths синхронизированы с явной env-подсказкой и текущим timeout.
- Добавлены минимальные unit-тесты парсинга timeout env в `pwm-cli` и `pwm-tui` (valid/invalid/overflow/default cases).
- Guardrails соблюдены: без `pwmd` wire/API drift, без `pwm-core` contract changes, без EXPORT/IMPORT.

## Current Gates (Slice 4)

- `cargo fmt --check`: **PASS**
- `cargo test -p pwm-cli`: **PASS**
- `cargo test -p pwm-tui`: **PASS**

## Slice 5 Closeout Prep (coding-pass)

- Evidence по slices 1..4 консолидирован во всех sprint-10 артефактах без изменения product scope.
- Подготовлен handoff в следующий спринт: release verdict не выставляется в coding-pass; финальная оценка остаётся за closeout + testing/review passes.
- Residual risks и deferred list (из Slice 3) зафиксированы как обязательные ограничения на post-Sprint 10 backlog.

## Current Gates (Slice 5)

- `cargo fmt --check`: **PASS**
- docs-only smoke: `cargo check -p pwm-cli`: **PASS**

## Slice 6 Orchestrator Closeout (operator-confirmed)

- После подтверждения оператором выполнена расширенная регрессия:
  - `cargo fmt --check` — **PASS**
  - `cargo test -p pwm-cli` — **PASS** (64 tests)
  - `cargo test -p pwm-tui` — **PASS** (54 tests)
  - `cargo test -p pwmd` — **PASS** (59 tests)
- Sprint 10 закрыт; **release verdict** по-прежнему вне scope спринта (см. roadmap Sprint 11 для optimization cut).

## Slice 3 MVP Cut Validation Result

- Подтверждено по Sprint 10 docs-артефактам: текущий спринт не расширяет MVP scope beyond hardening/reliability/conformance.
- Явный список deferred инициатив после Sprint 10 оформлен и синхронизирован во всех review-артефактах.
- Product code не менялся в рамках данного slice.

## Deferred After Sprint 10 (explicit)

- EXPORT/IMPORT cross-shard flow и зависимые user-facing сценарии: defer до появления core-поддержки (`TxBody` + runtime semantics в `pwm-core`).
- Любые wire/API расширения `pwmd`, выходящие за hardening/conformance: defer в отдельные post-Sprint 10 инициативы.
- Новые продуктовые capability-задачи вне reliability/hardening/conformance: defer в следующий цикл после closeout Sprint 10.

## Residual Risks (handoff)

- Полный runtime confidence остаётся неполным до testing-pass (широкая e2e/regression матрица вне coding-pass).
- Риски операторской конфигурации RPC/timeout снижены diagnostic-улучшениями, но требуют подтверждения на расширенных средах/нагрузке.
- Попытки раннего возврата к EXPORT/IMPORT без core-ready semantics должны считаться нарушением scope-gate post-Sprint 10.
