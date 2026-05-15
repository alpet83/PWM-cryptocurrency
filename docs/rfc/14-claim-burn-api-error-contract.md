# RFC 0014: Claim/Burn API error contract

**Status:** Active (V2-1 publication pack)  
**Version:** 1.0  
**Slice source:** D (`sprint-v2-1-slice-d-api-contract-freeze.md`)

## Abstract

RFC фиксирует wire-уровень reject-контракта для Claim/Burn: стабильный mapping `error.code -> response_class`, минимальную JSON-форму отказа и требования симметрии вердиктов между `mempool`, `preflight` и `apply`, включая explicit-claim и auto-claim ветки.

## Motivation

- Интеграторам нужен стабильный и предсказуемый error-контракт.
- Необходимо исключить деградацию в generic/internal ошибки для зафиксированных policy/state кейсов.
- Требуется трассируемость API-ответов к нормализованным decision classes RFC 0013.

## Specification

### Response classes

- `VALIDATION_ERROR`
- `POLICY_REJECT`
- `STATE_CONFLICT`
- `TEMPORARY_UNAVAILABLE`

`response_class` обязателен во всех reject-ответах.

### Stable code mapping baseline

Минимально обязательные коды (adopted from freeze):

- Claim: `E_SCHEMA_INVALID`, `E_MODE_FEE_CONFLICT`, `E_FEE_POLICY_REJECT`, `E_ANCHOR_RANGE_INVALID`, `E_ANCHOR_CONTINUITY_BROKEN`, `E_ANCHOR_STATE_UNAVAILABLE`, `E_CLAIM_UNITS_INVALID`, `E_CLAIM_OVER_MATURED`, `E_FREE_CLAIM_DAILY_LIMIT`, `E_REORG_STATE_MISMATCH`.
- Claim: `E_SCHEMA_INVALID`, `E_MODE_FEE_CONFLICT`, `E_FEE_POLICY_REJECT`, `E_ANCHOR_RANGE_INVALID`, `E_ANCHOR_CONTINUITY_BROKEN`, `E_ANCHOR_STATE_UNAVAILABLE`, `E_CLAIM_UNITS_INVALID`, `E_CLAIM_OVER_MATURED`, `E_FREE_CLAIM_DAILY_LIMIT`, `E_REORG_STATE_MISMATCH`.
- Burn: `E_BURN_SCHEMA_INVALID`, `E_BURN_UNITS_INVALID`, `E_BURN_OVER_BALANCE`, `E_BURN_POLICY_REJECT`.
- Import fee: `E_IMPORT_FEE_TOO_LOW`.

Семантика кодов стабильна; расширение допустимо только аддитивно.

### Minimal reject JSON shape

Обязательные поля:

- `ok=false`
- `phase` (`mempool|preflight|apply`)
- `tx_kind` (`claim|burn`)
- `claim_mode` (`explicit|auto`) для `tx_kind=claim` (для `burn` может отсутствовать)
- `response_class`
- `error.code`
- `error.message`
- `error.trace_id`

Для `IMPORT` при нарушении минимальной комиссии:

- `tx_kind = "import"` (или эквивалентный межшардовый тип в API),
- `error.code = "E_IMPORT_FEE_TOO_LOW"`,
- `response_class = "POLICY_REJECT"`.

## Error Semantics

- При одинаковом входе и эквивалентном pre-state `preflight` должен вернуть тот же `error.code`, что и `apply`.
- `mempool` обязан совпадать с `apply` для consensus-critical отказов.
- При state drift между preflight и apply допускается другой результат, но только в рамках стабильного набора кодов и классов.
- Для временной недоступности canonical anchor view используется только `E_ANCHOR_STATE_UNAVAILABLE` + `TEMPORARY_UNAVAILABLE`.
- Для auto-claim reject в составе релевантной транзакции узел обязан возвращать claim-класс ошибки (`E_*`) с явным `claim_mode=auto`; источник можно указывать либо как `tx_kind=claim`, либо как исходный `tx_kind` с дополнительным claim-контекстом, но семантика `error.code` должна оставаться claim-совместимой.
- Для `IMPORT` policy должен enforce `import_fee >= MIN_IMPORT_FEE_UNITS` (`0.01 PWM` в целевых единицах), и при reject возвращать `E_IMPORT_FEE_TOO_LOW` без state-mutation.

## Compatibility

- RFC является API-слоем поверх RFC 0013 и сохраняет трассируемость к его decision classes.
- Контракт рассчитан на аддитивное расширение без breaking-переопределения существующих `error.code`.

## Out-of-Scope

- Изменение consensus/policy правил за пределами RFC 0011-0013.
- Детали transport-level ретраев и backoff.
- Расширенные UX сообщения клиентов поверх `error.message`.

## References

- [Slice D API contract freeze](../reviews/sprint-v2-1-slice-d-api-contract-freeze.md)
- [Slice C policy matrix freeze](../reviews/sprint-v2-1-slice-c-policy-matrix-freeze.md)
- [Slice E implementation handoff](../reviews/sprint-v2-1-slice-e-implementation-handoff.md)
