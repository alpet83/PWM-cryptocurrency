# Sprint 8 Review Report (Slice 0 Scope Proof)

Дата: 2026-04-25  
Исполнитель: `pwm-coding`

## Review Scope

Slice 0/6: planning/freeze only (scope lock + contract baseline), без feature implementation и без изменений продуктового Rust-кода.

## Scope Proof Verdict

**PASS (pre-review, freeze baseline)**

- Scope Sprint 8 формализован в freeze-артефактах.
- Pre-task стартовые условия закрыты.
- Acceptance pack зафиксирован в формате 1 happy + 2 negative.
- Touched zones для slices 1-5 определены и ограничены.

## Acceptance Pack (Baseline)

- **Happy:** valid `BURN_MARK` with sufficient `marks_quota` reduces quota, keeps `balance_pwm` intact, and follows `fee=0`.
- **Negative 1:** insufficient `marks_quota` yields deterministic reject with no side-effect deductions and no contract drift.
- **Negative 2:** burn context outside source-only boundary is rejected without target-side behavior activation.

## No-Change Guardrails (explicit)

- Не ломать Sprint 7 facade/API stability (`pwmd` root facade compatibility must hold).
- Не менять route/field/error contracts вне burn scope.
- Не выходить за функциональный контур `marks_quota` + `BURN_MARK` + zero-fee baseline + source-only burn context.

## Change Surface (Slice 0)

- Updated: `docs/reviews/sprint-8-checklist.md`
- Added: `docs/reviews/sprint-8-status-note.md`
- Added: `docs/reviews/sprint-8-review-report.md`
- Product code changes: **none** (docs-only slice).

## Risks And Follow-up

- Основной риск: scope drift в slices 1-5 при точечных API/validation правках.
- Контрмера: на каждом slice проверять guardrails и acceptance pack до merge.

---

## Slice 1 Scope Proof (state wiring for `marks_quota`)

### Touched zones

- `crates/pwm-core/src/state.rs`
- `crates/pwmd/src/state.rs`
- `crates/pwmd/src/bootstrap.rs`
- `crates/pwmd/src/snapshot.rs`
- `docs/reviews/sprint-8-checklist.md`

### Explicit no-change assertions

- API routes/methods/response fields: no changes
- user-facing error map outside burn scope: no changes
- Sprint 7 facade/re-export stability: preserved
- tx/transport logic outside Slice 1 scope: no changes

### Invariant checks

- `BURN_MARK` debits `marks_quota` and does not debit `balance_pwm`.
- Insufficient quota reject does not produce side effects on account/quota state.
- Snapshot state contract rejects orphan `marks_quota` IDs (prevents hidden compatibility drift before state-root validation).

## Slice 1 Review Gate

### Verdict

**PASS (semantic)**

### Findings by severity

#### High

- None.

#### Medium

- Изначальный риск по orphan `marks_quota` keys в snapshot устранён в рамках Slice 1 explicit contract-check + regression test.

#### Low

- Остаточный риск ограничен покрытием текущего test suite.

### Recommendation

- Перейти к Slice 2 с узким фокусом на tx execution/validation path (`BURN_MARK` на quota model), не расширяя внешний контракт.

---

## Slice 2 Scope Proof (tx validation/execution path)

### Touched zones

- `crates/pwmd/src/tx_policy.rs`
- `crates/pwm-core/src/state.rs`
- `docs/reviews/sprint-8-checklist.md`

### Explicit no-change assertions

- API routes/methods/response fields: no changes
- user-facing error map outside burn scope: no changes
- Sprint 7 facade/re-export stability: preserved
- transport/scheduler semantics: no changes

### Invariant checks

- Insufficient quota path yields explicit `InsufficientMarks` reject.
- Reject path has no side effects (`account`, `fee_pool`, `marks_quota` unchanged).
- `BURN_MARK` guard-path coverage in `tx_policy` confirms policy-invalid beneficiary reject and same-shard allow path.

## Slice 2 Review Gate

### Verdict

**PASS (semantic)**

### Findings by severity

#### High

- None.

#### Medium

- None.

#### Low

- Checklist touched zones mention broader candidate files, while фактический diff у Slice 2 уже и точнее; это process-nit, не semantic issue.

### Recommendation

- Перейти к Slice 3 с фокусом на `fee=0` baseline enforcement и regression checks на отсутствие hidden fee side effects.

---

## Slice 3 Scope Proof (zero-fee baseline for mark-based flow)

### Touched zones

- `crates/pwm-core/src/tx.rs`
- `crates/pwm-core/src/state.rs`
- `docs/reviews/sprint-8-checklist.md`

### Explicit no-change assertions

- API routes/methods/response fields: no changes
- user-facing error map outside burn scope: no changes
- Sprint 7 facade/re-export stability: preserved
- transport/scheduler/lifecycle semantics: no changes

### Invariant checks

- `BurnMark` fee baseline is canonically `0` via `TxBody::fee_amount()`.
- Fee path side effects are explicit and scoped (no hidden `fee_pool`/balance drifts in burn scenarios).
- Existing burn reject invariants (`InsufficientMarks` + no side effects) remain preserved.

## Slice 3 Review Gate

### Verdict

**PASS (semantic)**

### Findings by severity

#### High

- None.

#### Medium

- None.

#### Low

- Process-nit: в checklist Slice 3 были указаны candidate touched zones в `pwmd`, фактический diff для Slice 3 ограничился `pwm-core` + docs (semantic issue отсутствует).

### Recommendation

- Перейти к Slice 4 с явным source-only proof boundary и targeted replay/consistency tests для cross-domain burn context.

---

## Slice 4 Scope Proof (cross-domain burn context source-only boundary)

### Touched zones

- `crates/pwmd/src/tx_policy.rs`
- `crates/pwm-core/src/tx.rs`
- `crates/pwm-core/src/state.rs`
- `docs/reviews/sprint-8-checklist.md`

### Explicit no-change assertions

- API routes/methods/response fields: no changes
- user-facing error map outside burn scope: no changes
- Sprint 7 facade/re-export stability: preserved
- transport/scheduler/lifecycle semantics: no changes
- target-side burn behavior: not introduced

### Invariant checks

- Source-only boundary для burn context enforced на local ingress (`pwmd`) и закреплён hard invariant в `pwm-core::State::apply_tx`.
- Cross-domain burn context reject remains deterministic.
- Reject path no-side-effects preserved for account/quota/fee_pool.
- Replay/consistency guardrails are aligned with current slice scope (signature/domain/nonce path unchanged, boundary checks strengthened).

## Slice 4 Review Gate

### Verdict

**PASS (semantic)**

### Findings by severity

#### High

- None.

#### Medium

- Изначальный medium-risk (boundary enforcement mostly at ingress) закрыт: source-only invariant дополнительно зафиксирован на core state boundary.

#### Low

- Full `pwm-core` regression suite не запускалась в рамках Slice 4; residual risk снят в Slice 5 (полный `cargo test -p pwm-core`).

### Recommendation

- Перейти к Slice 5: финальный contract audit + consolidated evidence + handoff в Sprint 9.

---

## Slice 5 Scope Proof (wrap-up, contract audit, sprint closeout)

### Touched zones (audit-only, без нового product diff)

- `crates/pwmd/src/lib.rs` — публичные re-exports согласованы с Sprint 7 decomposition; расширения поверхности вне burn scope не выявлены.
- `crates/pwmd/src/api.rs`, `crates/pwmd/src/state.rs`, `crates/pwmd/src/tx_policy.rs` — сверка с freeze: burn-path изменения изолированы в ожидаемых символах; маршруты и error map вне burn scope не менялись в slices 1–4.
- Артефакты: `docs/reviews/sprint-8-checklist.md`, `sprint-8-status-note.md`, `sprint-8-test-report.md`, настоящий файл.

### Explicit no-change assertions

- Sprint 7 facade stability: **HOLD**
- Route/field/error contracts вне burn scope: **NO DRIFT** (подтверждено сравнением с областью изменений slices 1–4 и audit pass).
- Non-goals Sprint 8: fee model expansion, target-side burn, cross-shard burn — **не внедрялись**.

### Invariant checks

- `marks_quota` + `BURN_MARK` + `fee=0` + source-only burn context — замкнуты и покрыты тестами (см. test report Slice 5).

## Slice 5 Review Gate

### Verdict

**PASS (semantic, sprint closeout)**

### Findings by severity

#### High / Medium

- None.

#### Low

- Чеклист Slice 4 уточнён по фактическим touched files (без `api.rs`/`chain.rs` в diff Slice 4).

### Recommendation

- Переход к **Sprint 9** (CLI/TUI для two-shard операций) с текущим frozen baseline ядра/pwmd.
