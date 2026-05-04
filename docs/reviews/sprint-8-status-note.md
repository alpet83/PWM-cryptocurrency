# Sprint 8 Status Note

Дата: 2026-04-25  
Этап: Sprint 8 completed (Slice 5/6 wrap-up)  
Статус: **HANDOFF → Sprint 9** (CLI/TUI integration per roadmap)

## Slice 0 Start/Ready State

- Sprint 8 scope зафиксирован: `marks_quota` + `BURN_MARK` + zero-fee baseline + source-only burn context.
- Non-goals зафиксированы: без Sprint 7 facade drift, без route/field/error drift вне burn scope, без fee/model expansion.
- Acceptance baseline зафиксирован: 1 happy + 2 negative сценария.
- Touched zones для slices 1-5 перечислены и ограничены.
- Slice 0 выполнен как planning/freeze, без изменений продуктового Rust-кода.

## Current Gates (Sprint 8 closeout)

- Coding gate (`cargo fmt --check`, `cargo check -p pwmd`, `cargo check -p pwm-core`): **PASS**
- Testing gate (`cargo test -p pwmd` + full `cargo test -p pwm-core` на Slice 5): **PASS**
- Review gate (spec alignment + contract drift check): **PASS**
- Artifact closeout (status/checklist/review/test sync): **PASS**

## Contract Guardrails Snapshot

- Sprint 7 facade/API stability: MUST HOLD.
- Route/field/error contracts вне burn scope: NO CHANGES.
- Burn feature boundary: ONLY `marks_quota` + `BURN_MARK` + `fee=0` baseline + source-only burn context.

## Next Step

- Sprint 9: CLI/TUI integration for two-shard operations (см. multi-sprint plan).

## Slice 1 Update (state wiring)

- Изменены зоны: `crates/pwm-core/src/state.rs`, `crates/pwmd/src/state.rs`, `crates/pwmd/src/bootstrap.rs`, `crates/pwmd/src/snapshot.rs`.
- Добавлено состояние `marks_quota` в core state с `serde(default)` и нормализацией.
- Burn-path state invariant зафиксирован: `BURN_MARK` работает через quota-path без списания `balance_pwm`.
- Добавлены тесты инвариантов:
  - `burn_mark_debits_quota_without_touching_balance`
  - `burn_mark_rejects_insufficient_quota_without_side_effects`
  - `snapshot_rejects_orphan_marks_quota_ids` (совместимость/контракт snapshot)

## Slice 1 Gates

- Coding gate: **PASS** (`cargo fmt --check`, `cargo check -p pwmd`, `cargo check -p pwm-core`).
- Testing gate: **PASS** (`cargo test -p pwmd`: 56 passed, 0 failed).
- Review gate: **APPROVE with nits addressed** (orphan `marks_quota` snapshot edge-case закрыт explicit contract-check + test).

## Slice 2 Update (tx validation/execution path)

- Изменены зоны: `crates/pwmd/src/tx_policy.rs`, `crates/pwm-core/src/state.rs`.
- Добавлено targeted покрытие guard-path для `BURN_MARK` в `tx_policy`:
  - same-shard beneficiary allow,
  - policy-invalid beneficiary reject (`400`) без contract drift.
- Усилен `pwm-core` no-side-effects reject test для недостаточной quota:
  - инварианты на неизменность `account`, `fee_pool`, `marks_quota`.

## Slice 2 Gates

- Coding gate: **PASS** (`cargo fmt --check`, `cargo check -p pwmd`, `cargo check -p pwm-core`).
- Testing gate: **PASS** (`cargo test -p pwmd`: 58 passed, 0 failed; `cargo test -p pwm-core burn_mark`: 2 passed).
- Review gate: **APPROVE** (scope/guardrails соблюдены, drift вне burn scope не выявлен).

## Slice 3 Update (zero-fee baseline)

- Изменены зоны: `crates/pwm-core/src/tx.rs`, `crates/pwm-core/src/state.rs`.
- Введён канонический `TxBody::fee_amount()` с фиксированным `0` для `BurnMark` (и других non-transfer форм).
- Transfer fee-path теперь использует единый canonical fee-view (`tx.body.fee_amount()`), что снижает риск неявного drift.
- Добавлены targeted tests:
  - `fee_amount_is_zero_for_burn_mark`
  - усилены burn tests на invariants `fee_pool`/`nonce`/`balance`/`marks_quota`.

## Slice 3 Gates

- Coding gate: **PASS** (`cargo fmt --check`, `cargo check -p pwmd`, `cargo check -p pwm-core`).
- Testing gate: **PASS** (`cargo test -p pwmd`: 58 passed, 0 failed; `cargo test -p pwm-core burn_mark`: 4 passed).
- Review gate: **APPROVE with low process nit** (semantic drift не выявлен; фиксировать actual touched files в evidence явно).

## Slice 4 Update (source-only burn context boundary)

- Изменены зоны: `crates/pwmd/src/tx_policy.rs`, `crates/pwm-core/src/tx.rs`, `crates/pwm-core/src/state.rs`.
- В `pwmd` добавлен explicit source-boundary reject для cross-domain `BurnMark` context на local tx path.
- В `pwm-core` добавлен канонический helper `burn_context_is_source_domain(...)` и hard invariant на уровне state transition (`apply_tx`) для `BurnMark`.
- Добавлены/уточнены targeted tests для boundary/consistency:
  - `burn_mark_guard_rejects_cross_domain_context` (`pwmd`)
  - `burn_context_*` tests (`pwm-core/tx`)
  - `burn_mark_rejects_cross_domain_beneficiary_without_side_effects` (`pwm-core/state`)

## Slice 4 Gates

- Coding gate: **PASS** (`cargo fmt --check`, `cargo check -p pwmd`, `cargo check -p pwm-core`).
- Testing gate: **PASS** (`cargo test -p pwmd`: 59 passed, 0 failed; `cargo test -p pwm-core burn_mark`: 5 passed).
- Review gate: **APPROVE with nits addressed** (medium risk про boundary-level enforcement закрыт hard invariant в `pwm-core`).

## Slice 5 Update (wrap-up + contract audit)

- Изменения кода в Slice 5 не вносились: финальный audit поверх slices 1–4.
- Проверены зоны: `crates/pwmd/src/lib.rs` (re-exports без расширения публичной поверхности вне Sprint 7), `api.rs` / `state.rs` / `tx_policy.rs` — согласованность с freeze guardrails и отсутствием drift вне burn scope.
- Consolidated evidence: `sprint-8-review-report.md` (Slice 5), `sprint-8-test-report.md` (полный `pwm-core` suite).

## Slice 5 Gates

- Coding gate: **PASS** (`cargo fmt --check`, `cargo check -p pwmd`, `cargo check -p pwm-core`).
- Testing gate: **PASS** (`cargo test -p pwmd`: 59 passed; `cargo test -p pwm-core`: 56 passed).
- Review gate: **APPROVE** (Sprint 7 facade/API stability сохранена; burn scope замкнут на quota + zero-fee + source-only context).
