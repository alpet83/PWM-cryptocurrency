# Sprint 8 Checklist: Burn-Quota Path (`marks_quota`) + Zero-Fee Baseline

Дата старта: 2026-04-25  
Фокус: feature sprint (продуктовая логика), не decomposition.

## Scope Freeze (Slice 0 baseline)

### In Scope (точно входит в Sprint 8)

- `marks_quota` как burn-only ресурс (state + execution path для burn).
- `BURN_MARK` списывает quota, а не `balance_pwm`.
- Baseline `fee=0` для mark-based flow без ввода альтернативной fee-политики.
- Cross-domain burn context ограничен source-only proof boundary.

### Non-Goals (точно вне Sprint 8)

- Любая переработка Sprint 7 module decomposition и facade layering.
- Расширение/изменение API-роутов, DTO-полей и error-map вне burn scope.
- Изменения fee-модели за пределами zero-fee baseline для mark-based flow.
- Внедрение target-side burn поведения вне source-only контекста.
- Optimization backlog Sprint 11 и несвязанные refactor-работы.

## Pre-Task (обязательный старт)

- [x] Подтверждён scope и non-goals для Sprint 8 (Slice 0 freeze).
- [x] Сверены спецификация и RFC ссылки:
  - `docs/WHITE_SPEC_v0.md`
  - `docs/rfc/7-tx-and-state-model.md`
  - `docs/rfc/3-cross-domain-roaming.md`
  - `docs/rfc/6-policy-engine.md`
- [x] Зафиксирован baseline acceptance pack (1 happy + 2 negative).

## Sprint 8 No-Change/Guardrails (contract baseline)

- [x] Не ломать закрытые контракты Sprint 7 (`pwmd` facade и API stability).
- [x] Не менять route/field/error contracts вне burn scope.
- [x] Не выходить за agreed baseline: `marks_quota` + `BURN_MARK` + `fee=0` + source-only burn context.
- [x] Не смешивать Sprint 8 feature-работу с Sprint 11 optimization backlog.

## Acceptance Pack Baseline (Slice 0 contract)

- **Happy path (1):** валидный `BURN_MARK` при достаточном `marks_quota` уменьшает quota, не трогает `balance_pwm`, проходит по `fee=0`.
- **Negative path (1):** недостаточный `marks_quota` даёт deterministic reject без побочных списаний и без изменения route/DTO/error формата.
- **Negative path (2):** burn context вне source-only boundary отклоняется ожидаемым guardrail-поведением без target-side side effects.

## Slice Plan (freeze-версия)

### Slice 0/6: Scope Freeze + Contract Baseline

- [x] Зафиксированы exact touch zones и acceptance criteria.
- [x] Подтверждён baseline по `marks` / `marks_quota` / burn flow (документарно).
- [x] Подготовлен regression checklist baseline для slices 1-5.

### Slice 1/6: State Model Wiring (`marks_quota`)

- [x] Touched zones: `crates/pwmd/src/state.rs`, `crates/pwmd/src/bootstrap.rs`, `crates/pwm-core/src/state.rs`.
- [x] Добавить/нормализовать state-представление `marks_quota` и default/init semantics.
- [x] Не менять unrelated account fields/contracts.

#### Slice 1 Mini DoD (must hold)

- [x] Изменения ограничены state wiring (`marks_quota`) без API/route/error drift.
- [x] Для валидного baseline burn-path `balance_pwm` не изменяется из-за quota wiring.
- [x] Для недостаточной quota reject-путь не делает побочных списаний.
- [x] Evidence фиксирует конкретные touched symbols и before/after state invariants.

### Slice 2/6: Tx Validation And Execution Path (`BURN_MARK`)

- [x] Touched zones: `crates/pwmd/src/tx_policy.rs`, `crates/pwmd/src/lifecycle.rs` (если задействована tx loop wiring), `crates/pwm-core/src/tx.rs`, `crates/pwm-core/src/state.rs`.
- [x] Провести `BURN_MARK` по quota-path вместо `balance_pwm`.
- [x] Сохранить explicit reject-поведение для недостаточной quota без изменения вне-scope tx semantics.

### Slice 3/6: Zero-Fee Baseline

- [x] Touched zones: `crates/pwmd/src/tx_policy.rs`, `crates/pwmd/src/api.rs` (только если нужна валидация входа без контракта drift), `crates/pwm-core/src/tx.rs`, `crates/pwm-core/src/state.rs`.
- [x] Зафиксировать и удержать `fee=0` policy для mark-based flow.
- [x] Проверить что fee-path не списывает лишние ресурсы и не меняет внешний error/API контракт вне scope.

### Slice 4/6: Cross-Domain Burn Context (Source-Only Proof Handling)

- [x] Touched zones: `crates/pwmd/src/tx_policy.rs`, `crates/pwm-core/src/tx.rs`, `crates/pwm-core/src/state.rs` (proof/context validation boundary).
- [x] Добавить/уточнить source-boundary handling для burn context.
- [x] Проверить replay/consistency guardrails без внедрения target-side behavior.

### Slice 5/6: Wrap-Up And Contract Audit

- [x] Touched zones: audit pass по `crates/pwmd/src/lib.rs` (facade re-exports), `crates/pwmd/src/api.rs`, `crates/pwmd/src/state.rs`, `crates/pwmd/src/tx_policy.rs`.
- [x] Финальный audit по API/error/state contracts и Sprint 7 compatibility.
- [x] Consolidated test/review evidence + handoff в Sprint 9.

## Gates Per Slice

- [x] Coding gate: `cargo fmt --check`, `cargo check -p pwmd` (и релевантные crates при изменении).
- [x] Testing gate: targeted tests + `cargo test -p pwmd` + **Slice 5:** полный `cargo test -p pwm-core`.
- [x] Review gate: semantic check (spec alignment + no unintended drift).
- [x] Artifact closeout: обновить review/checklist/status-note по slice.

## Evidence Notes (Sprint 8)

- `scoped_diff_stat` фокусировать на product/tooling code paths.
- Для каждого slice фиксировать:
  - touched symbols,
  - asserted unchanged contracts,
  - commands and results,
  - residual risks.
