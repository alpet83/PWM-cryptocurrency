# RFC 0013: Claim policy matrix

**Status:** Active (V2-1 publication pack)  
**Version:** 1.0  
**Slice source:** C (`sprint-v2-1-slice-c-policy-matrix-freeze.md`)

## Abstract

RFC фиксирует policy-матрицу Claim/Burn для фаз `mempool`, `preflight`, `apply`: единый порядок проверок, anchor incompatibility predicates, единое правило округления maturity (`floor`), explicit-claim vs auto-claim policy границы и replay-поведение для reorg/rollback.

## Motivation

- Нужна детерминированная и проверяемая policy-логика перед реализацией.
- Требуется устранить расхождения между phase-specific проверками.
- Нужно закрепить классы отказов до API wire-формализации.

## Specification

### Unified phase order

Порядок проверок одинаковый по смыслу для всех фаз:

1. Schema/field gates.
2. Mode/fee policy.
3. Anchor/state compatibility.
4. Maturity arithmetic.
5. Free-day and reorg-sensitive checks.
6. State transition guards.

`apply` является канонической фазой; при одинаковом snapshot `mempool` и `preflight` обязаны давать эквивалентный вердикт по consensus-critical причинам.

### Explicit claim vs auto-claim policy boundary

- `ClaimTx` проходит полный phase-path (`mempool -> preflight -> apply`) как самостоятельная транзакция.
- Auto-claim выполняется только в `apply` как часть state-transition релевантной транзакции и не имеет отдельного mempool/preflight verdict как отдельный tx.
- Для одинакового canonical порядка транзакций блока итог `marks`/claim-state должен быть эквивалентен модели с явными materialization шагами.
- Ограничение `E_FREE_CLAIM_DAILY_LIMIT` применяется к explicit `ClaimTx` в режиме `free`; auto-claim не считается отдельной free-claim транзакцией.
- Для auto-claim используется та же формула materialization (`hours`, `matured_raw`, `floor`) и тот же zero-delta rule (`matured_units_available == 0` -> no-op).

### Maturity rounding

- Материализуемая величина определяется только как `floor(matured_units_raw)`.
- Перенос дробного остатка как отдельного переносимого state-credit не допускается.

### Anchor incompatibility predicates

Claim отклоняется при любом из условий:

- future anchor: `anchor_ref > inclusion_height`;
- non-monotonic anchor: `anchor_ref < last_claim_anchor_ref`;
- continuity broken: в интервале `(anchor_ref, inclusion_height]` есть ненулевое изменение релевантного баланса;
- canonical anchor view unavailable: нода не может построить канонический snapshot для проверки.

### Reorg policy baseline

- `last_free_claim_utc_day` и `last_claim_anchor_ref` определяются только canonical replay после расхождения.
- orphaned эффекты claim/free полностью аннулируются.

## Error Semantics

Нормализованные policy-классы ошибок:

- `E_SCHEMA_INVALID`
- `E_MODE_FEE_CONFLICT`
- `E_FEE_POLICY_REJECT`
- `E_ANCHOR_RANGE_INVALID`
- `E_ANCHOR_CONTINUITY_BROKEN`
- `E_ANCHOR_STATE_UNAVAILABLE`
- `E_CLAIM_UNITS_INVALID`
- `E_CLAIM_OVER_MATURED`
- `E_FREE_CLAIM_DAILY_LIMIT`
- `E_REORG_STATE_MISMATCH`

Эти коды фиксируют semantic class для следующего API-слоя; wire-форма задается RFC 0014.

## Compatibility

- RFC совместим с RFC 0011 (tx schema) и RFC 0012 (state model).
- Burn policy-кейсы следуют тем же фазовым принципам, даже если их матрица будет расширяться аддитивно.
- Для межшардового `IMPORT` применяется policy baseline variant B: комиссия импорта зачисляется в `fee_pool` target-шарда после успешного apply.
- Нормативный минимум `min_import_fee = 0.01 PWM` (в минимальных единицах сети параметризуется как integer-значение `MIN_IMPORT_FEE_UNITS`).

## Out-of-Scope

- JSON reject shape и обязательные API поля.
- Версионирование endpoint-контрактов.
- UX-интерпретация ошибок в CLI/TUI.

## References

- [Slice C policy matrix freeze](../reviews/sprint-v2-1-slice-c-policy-matrix-freeze.md)
- [Slice B state freeze](../reviews/sprint-v2-1-slice-b-state-freeze.md)
- [Slice A tx freeze](../reviews/sprint-v2-1-slice-a-tx-schema-freeze.md)
- [Slice E implementation handoff](../reviews/sprint-v2-1-slice-e-implementation-handoff.md)
