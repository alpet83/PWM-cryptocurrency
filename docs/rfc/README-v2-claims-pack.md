# V2 Claims RFC Pack (Sprint V2-1)

## Purpose

Этот пакет публикует формализованные RFC для claim/burn перед кодовыми слайсами E-1/E-2/E-3. Источником истины остаются freeze-артефакты в `docs/reviews/`; RFC ниже нормализуют решения в стабильную структуру без дублирования полной детализации.

## Navigation

1. [RFC 0011: Burn purpose and Claim transaction schema](./11-burn-purpose-and-claim-tx.md)
2. [RFC 0012: Claim maturity and state model](./12-claim-maturity-and-state-model.md)
3. [RFC 0013: Claim policy matrix](./13-claim-policy-matrix.md)
4. [RFC 0014: Claim/Burn API error contract](./14-claim-burn-api-error-contract.md)

## Dependency flow

`RFC 0011 (tx)` -> `RFC 0012 (state)` -> `RFC 0013 (policy)` -> `RFC 0014 (api)`

## Source freeze artifacts (normative inputs)

- [RFC inputs 2026-05-05](../reviews/sprint-v2-1-rfc-inputs-20260505.md)
- [Slice A tx schema freeze](../reviews/sprint-v2-1-slice-a-tx-schema-freeze.md)
- [Slice B state freeze](../reviews/sprint-v2-1-slice-b-state-freeze.md)
- [Slice C policy matrix freeze](../reviews/sprint-v2-1-slice-c-policy-matrix-freeze.md)
- [Slice D API contract freeze](../reviews/sprint-v2-1-slice-d-api-contract-freeze.md)
- [Slice E implementation handoff](../reviews/sprint-v2-1-slice-e-implementation-handoff.md)

## Publication notes

- Пакет `docs-only`: без изменений `crates/*`.
- Error semantics в RFC 0014 сохраняют semantic classes из RFC 0013.
- Legacy совместимость `BurnMarkTx v1` документирована в RFC 0011 как временный adapter path.
