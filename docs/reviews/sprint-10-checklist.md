# Sprint 10 Checklist: Hardening / Reliability / Conformance / MVP Cut

Дата старта: 2026-04-26  
Фокус: зафиксировать baseline Sprint 10 и открыть execution-поток hardening без расширения product scope.

## Scope Freeze (Slice 0 baseline)

### In Scope (Sprint 10)

- Hardening operator UX для существующих CLI/TUI/pwmd workflows без wire drift.
- Reliability: воспроизводимость demo/runtime сценариев, устойчивость к типовым operator mistakes.
- Conformance: синхронизация docs/checklists/gates с фактическим состоянием MVP.
- MVP cut: приоритизация только того, что уменьшает риск интеграции и релизного прогона.

### Non-Goals (Sprint 10)

- Новые продуктовые фичи вне hardening/conformance.
- Изменение wire/API контрактов `pwmd` без отдельного согласования.
- Добавление EXPORT/IMPORT tx-flow до появления соответствующей поддержки в `pwm-core`.
- Подмена testing-pass: coding-pass фиксирует baseline и compile-smoke, но не закрывает full test matrix.

### Handoff from Sprint 9 (фиксирован)

- EXPORT/IMPORT остаётся в defer до core: без user-facing stub-команд и без псевдо-реализации.
- `wallet import-seed` трактуется только как wallet workflow, не как cross-shard EXPORT/IMPORT.
- Возврат к EXPORT/IMPORT разрешён только после появления нужных `TxBody` и согласованного core slice.

## Pre-Task (обязательный старт)

- [x] Созданы baseline-артефакты Sprint 10:
  - `docs/reviews/sprint-10-checklist.md`
  - `docs/reviews/sprint-10-status-note.md`
  - `docs/reviews/sprint-10-review-report.md`
  - `docs/reviews/sprint-10-test-report.md`
- [x] Scope/non-goals/gates зафиксированы для Slice 0.
- [x] Handoff из Sprint 9 (EXPORT/IMPORT defer до core) перенесён в Sprint 10 baseline.

## Slices (0..6)

### Slice 0/6: Planning + Freeze (baseline)

- [x] Baseline артефакты созданы и синхронизированы.
- [x] Readiness к Slice 1 подтверждена по coding-pass gate.

### Slice 1/6: Reliability Pass for Operator Flows

- [x] Уточнить и стабилизировать operator paths для текущих CLI/TUI сценариев без API drift.
- [x] Зафиксировать регрессионно-опасные зоны и минимальные hardening fixes (coding-pass: RPC HTTP/nonce/submit).

### Slice 2/6: Conformance Pass (Docs vs Runtime)

- [x] Сверить operator guides/checklists с текущим поведением бинарей.
- [x] Устранить расхождения формулировок/ожиданий в артефактах Sprint 10.

### Slice 3/6: MVP Cut Validation

- [x] Подтвердить, что Sprint 10 не расширяет scope beyond MVP cut.
- [x] Оформить список явно deferred инициатив после Sprint 10.

### Slice 4/6: Stabilization Wrap

- [x] Закрыть только согласованные hardening правки.
- [x] Обновить status/review/test артефакты для Slice 4.

### Slice 5/6: Closeout Prep (no final verdict here)

- [x] Консолидировать evidence по slices 1..4.
- [x] Подготовить handoff в следующий спринт без release verdict в coding-pass.

### Slice 6/6: Orchestrator Closeout (operator-confirmed)

- [x] Расширенная регрессия после подтверждения оператором: `cargo fmt --check`, `cargo test -p pwm-cli`, `cargo test -p pwm-tui`, `cargo test -p pwmd` — PASS.
- [x] Sprint 10 закрыт в артефактах (handoff в Sprint 11 optimization backlog); **release verdict** по-прежнему вне scope этого спринта.

## Acceptance / Gates

- [x] Slice 0 Coding gate: `cargo fmt --check` -> PASS.
- [x] Slice 0 Smoke gate (docs-only исключение): `cargo check -p pwm-cli` -> PASS.
- [x] Artifact gate: baseline-артефакты Sprint 10 заведены и согласованы.
- [x] Product-code guardrail: в Slice 0 product code не менялся.
- [x] Slice 1 UX-crates gate: `cargo fmt --check`, `cargo test -p pwm-cli`, `cargo test -p pwm-tui` -> PASS.
- [x] Slice 2 conformance gate: docs синхронизированы с runtime (`PWM_CLI_RPC_TIMEOUT_MS`, nonce/submit error policy, CLI/TUI timeout env names) + `cargo fmt --check`, `cargo test -p pwm-cli`, `cargo test -p pwm-tui` -> PASS.
- [x] Slice 6 orchestrator closeout gate: `cargo fmt --check`, `cargo test -p pwm-cli`, `cargo test -p pwm-tui`, `cargo test -p pwmd` -> PASS.

Примечание по continuity: для execution slices Sprint 10 baseline возвращается к расширенному smoke/regression покрытию по затронутым UX-crates (`pwm-cli` + `pwm-tui`; `pwmd` при касании контрактов). Полноценные test verdict/матрица остаются зоной testing-pass.

### Slice 1 coding-pass evidence (2026-04-26)

- `cargo fmt --check` — PASS  
- `cargo test -p pwm-cli` — PASS  
- `cargo test -p pwm-tui` — PASS  
- Изменения: `pwm-cli` / `pwm-tui` только; без `pwmd` wire и без `pwm-core` контрактов.

### Slice 2 coding-pass evidence (2026-04-26)

- `cargo fmt --check` — PASS  
- `cargo test -p pwm-cli` — PASS (63 tests)  
- `cargo test -p pwm-tui` — PASS (53 tests)  
- Синхронизировано в docs: `PWM_CLI_RPC_TIMEOUT_MS` (default 10000 ms, max 120000 ms), явные nonce/submit ошибки вместо silent `nonce=0`, различие env timeout (`PWM_CLI_RPC_TIMEOUT_MS` vs `PWM_TUI_RPC_TIMEOUT_MS`).
- Product code: без изменений (docs-only conformance update).

### Slice 3 coding-pass evidence (2026-04-26)

- MVP-cut boundary validated по docs-артефактам: Sprint 10 ограничен hardening/reliability/conformance и не добавляет новые product-capabilities.
- Scope-guard подтверждён в baseline/non-goals (`sprint-10-checklist`, `sprint-10-status-note`, `sprint-10-review-report`, `sprint-10-test-report`) и согласован с `docs/MVP-checklist.md` (без расширения MVP-разделов).
- Явный deferred-list после Sprint 10 зафиксирован и синхронизирован во всех Sprint 10 review-артефактах.
- Product code: без изменений (docs-only MVP cut validation).

### Slice 4 coding-pass evidence (2026-04-26)

- `cargo fmt --check` — PASS  
- `cargo test -p pwm-cli` — PASS  
- `cargo test -p pwm-tui` — PASS  
- Product hardening scope: только `pwm-cli`/`pwm-tui` стабилизации fallback HTTP client + timeout/env diagnostics/messages; без `pwmd` wire/API drift, без `pwm-core` contract changes, без EXPORT/IMPORT enablement.

### Slice 5 coding-pass evidence (2026-04-26)

- Консолидировано evidence slices 1..4 в Sprint 10 артефактах: `sprint-10-checklist` / `sprint-10-status-note` / `sprint-10-review-report` / `sprint-10-test-report`.
- Зафиксирован handoff в post-Sprint 10 цикл: финальный release verdict не выставляется в coding-pass closeout prep.
- Residual risks и deferred list из Slice 3 подтверждены без изменений продуктового scope.
- Проверки closeout coding-pass:
  - `cargo fmt --check` — PASS
  - docs-only smoke: `cargo check -p pwm-cli` — PASS
- Product code: без изменений (docs-only closeout prep).

### Slice 6 orchestrator closeout evidence (2026-04-26)

- Подтверждение оператором получено; выполнена расширенная регрессия:
  - `cargo fmt --check` — PASS
  - `cargo test -p pwm-cli` — PASS (64 tests)
  - `cargo test -p pwm-tui` — PASS (54 tests)
  - `cargo test -p pwmd` — PASS (59 tests)
- Sprint 10 закрыт; следующий roadmap-этап — Sprint 11 (optimization backlog по плану).

## Deferred After Sprint 10 (explicit)

- EXPORT/IMPORT cross-shard tx-flow (core-dependent): defer до появления соответствующих `TxBody`/runtime-поддержки в `pwm-core` и отдельного core slice.
- Любые user-facing команды/стабы под EXPORT/IMPORT: defer до готовности core semantics, чтобы не создавать ложный UX/контракт.
- Новые wire/API контракты `pwmd` и расширение REST surface: вне Sprint 10; отдельно после hardening/conformance closeout.
- Новые продуктовые фичи вне hardening/reliability/conformance (новые tx-capabilities, протокольные расширения): defer post-Sprint 10 backlog.

## Residual Risks (post-Sprint 10 handoff)

- Runtime reliability риски смещены в testing-pass матрицу: coding-pass подтвердил compile/docs gates, но не заменяет full e2e/regression прогон.
- Операторские ошибки конфигурации RPC/timeout по-прежнему возможны в полевых условиях; mitigation покрыт diagnostics/hints, но требует дальнейшей валидации на расширенных сценариях.
- Граница MVP cut удержана, но любые попытки ускорить EXPORT/IMPORT до готовности `pwm-core` остаются источником регрессионного риска и должны блокироваться scope-gate.
