# Sprint 10 Review Report (Slice 0–6, coding-pass + orchestrator closeout)

Дата: 2026-04-26  
Исполнитель: coding-pass

## Review Scope

### Slice 0 (baseline)

- Создание baseline-артефактов Sprint 10.
- Формализация scope/non-goals/pre-task/slices/gates.
- Фиксация handoff из Sprint 9 по EXPORT/IMPORT defer до core.

### Slice 1 (operator reliability)

- **pwm-cli:** убраны «тихие» сценарии nonce при сбоях RPC/HTTP; убраны паники `.expect("http")` на submit; добавлены таймауты и переменная `PWM_CLI_RPC_TIMEOUT_MS`; улучшены тексты ошибок (статус, фрагмент body, подсказки по RPC URL).
- **pwm-tui:** при ошибке HTTP или отсутствии/битом поле `nonce` больше не подставляется `0` для подписи; уточнены сообщения при ошибке соединения.

### Slice 2 (conformance docs vs runtime)

- Обновлены operator guides: добавлены runtime-границы `PWM_CLI_RPC_TIMEOUT_MS` (default `10000` ms, max `120000` ms) и зафиксировано различие timeout env names: CLI `PWM_CLI_RPC_TIMEOUT_MS` vs TUI `PWM_TUI_RPC_TIMEOUT_MS`.
- В smoke/сценариях закреплено поведение nonce/submit после hardening: при HTTP/JSON проблемах — явная ошибка, без silent `nonce=0`.
- Sprint 10 checklist/status/test/report синхронизированы с фактическим состоянием после Slice 2 и readiness к Slice 3.

### Slice 3 (MVP cut validation)

- Подтверждено по Sprint 10 артефактам, что sprint scope остаётся в рамках hardening/reliability/conformance и не расширяется beyond MVP cut.
- Сформирован и зафиксирован явный deferred-list post-Sprint 10 для инициатив вне текущего cut (особенно EXPORT/IMPORT core-dependent).
- Синхронизированы `sprint-10-checklist` / `sprint-10-status-note` / `sprint-10-review-report` / `sprint-10-test-report`.

### Slice 4 (stabilization wrap)

- **pwm-cli:** fallback ветка `reqwest::blocking::Client::builder().build()` теперь operator-visible: одноразовое предупреждение при fallback на `Client::new()` с явной пометкой, что timeout behavior может отличаться, и ссылкой на `PWM_CLI_RPC_TIMEOUT_MS`.
- **pwm-tui:** аналогичная fallback-диагностика для HTTP client (с `PWM_TUI_RPC_TIMEOUT_MS`), плюс timeout error messages в `fetch_nonce`/submit paths приведены к единому operator-facing виду с timeout/env hint.
- В оба crate добавлены минимальные unit-тесты timeout env parser (валидный, нулевой, out-of-range, нечисловой значения).
- Изменения ограничены hardening-слоем: без wire/API drift в `pwmd`, без контрактных изменений в `pwm-core`, без EXPORT/IMPORT enablement.

### Slice 5 (closeout prep, no release verdict)

- Консолидирован evidence-контур по slices 1..4 во всех sprint-10 артефактах (`checklist`/`status-note`/`review-report`/`test-report`) в согласованной формулировке.
- Подготовлен handoff в следующий спринт: release verdict не выставляется в coding-pass closeout; финальный verdict остаётся за совокупностью closeout + testing/review passes.
- Residual risks и deferred-list из Slice 3 закреплены как post-Sprint 10 ограничения без scope expansion.

### Slice 6 (orchestrator closeout, operator-confirmed)

- После подтверждения оператором выполнена расширенная регрессия: `cargo fmt --check`, `cargo test -p pwm-cli`, `cargo test -p pwm-tui`, `cargo test -p pwmd` — PASS.
- Sprint 10 закрыт в артефактах; handoff в Sprint 11 (optimization backlog) зафиксирован без изменения non-goals и без release verdict в coding-pass.

## Change Surface (Slice 5–6 closeout update)

- Docs: `docs/reviews/sprint-10-checklist.md`
- Docs: `docs/reviews/sprint-10-status-note.md`
- Docs: `docs/reviews/sprint-10-test-report.md`
- Docs: `docs/reviews/sprint-10-review-report.md`
- Product code в Slice 5–6: без изменений (docs-only closeout + orchestrator regression evidence).

## Non-Goals Compliance

- Нет изменений wire/API `pwmd`.
- Нет изменений контрактов `pwm-core`.
- Нет EXPORT/IMPORT.
- Нет scope expansion beyond MVP cut.

## Deferred After Sprint 10 (explicit)

- EXPORT/IMPORT cross-shard tx-flow и все user-facing сценарии вокруг него — defer до core-ready (`TxBody` + runtime semantics в `pwm-core`).
- Любые новые `pwmd` wire/API расширения вне hardening/conformance — defer в отдельный post-Sprint 10 трек.
- Capability-фичи, не относящиеся к reliability/hardening/conformance (новые протокольные или UX-потоки), — defer после closeout Sprint 10.

## Quality Gates Evidence (Slice 4)

- `cargo fmt --check` → PASS  
- `cargo test -p pwm-cli` → PASS  
- `cargo test -p pwm-tui` → PASS  

## Quality Gates Evidence (Slice 5 closeout prep)

- `cargo fmt --check` → PASS  
- docs-only smoke: `cargo check -p pwm-cli` → PASS  

## Quality Gates Evidence (Slice 6 orchestrator closeout)

- `cargo fmt --check` → PASS  
- `cargo test -p pwm-cli` → PASS (64 tests)  
- `cargo test -p pwm-tui` → PASS (54 tests)  
- `cargo test -p pwmd` → PASS (59 tests)  

## Findings / Verdict

- **Slice 0:** baseline APPROVE (docs-only).
- **Slice 1:** hardening **APPROVE** с оговоркой: изменилось поведение при ранее «тихих» сбоях nonce (теперь явная ошибка вместо подписи с nonce 0) — это намеренное улучшение надёжности; операторские гайды сверить в Slice 2 (conformance).
- **Slice 2:** conformance **APPROVE**: docs и Sprint 10 артефакты приведены к текущему runtime CLI/TUI; readiness к Slice 3 подтверждена.
- **Slice 3:** MVP cut validation **APPROVE**: scope freeze подтверждён, defer-инициативы оформлены явно, Sprint 10 review-артефакты синхронизированы.
- **Slice 4:** stabilization wrap **APPROVE**: low-risk hardening nits закрыты, operator diagnostics усилены, guardrails/non-goals соблюдены.
- **Slice 5:** closeout prep **APPROVE**: evidence консолидирован, handoff и residual/deferred ограничения оформлены; release verdict намеренно не выставляется в coding-pass.
- **Slice 6:** orchestrator closeout **APPROVE**: расширенная регрессия PASS; Sprint 10 закрыт в артефактах; release verdict по-прежнему вне coding-pass.

## Residual Risks (post-Sprint 10 handoff)

- coding-pass не покрывает полный e2e/regression объём; финальный confidence требует testing-pass матрицы.
- Несмотря на усиленные diagnostics, в production-like условиях остаётся риск неверной RPC/timeout конфигурации оператором.
- Scope pressure на ранний EXPORT/IMPORT остаётся высоким; до core-ready (`TxBody` + runtime semantics) это источник потенциальной регрессии.

Финальный sprint/release verdict не выставляется в coding-pass; Sprint 10 закрыт по orchestrator regression gate (Slice 6) без изменения этой политики.
