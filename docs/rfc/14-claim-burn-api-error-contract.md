# RFC 0014: Claim/Burn API error contract

**Status:** Active (V2-1 publication pack)  
**Version:** 1.1  
**Slice source:** D (`sprint-v2-1-slice-d-api-contract-freeze.md`)

## Abstract

RFC фиксирует wire-уровень reject-контракта для Claim/Burn и задаёт аддитивное расширение для V4 policy decisions: стабильный mapping `error.code -> response_class`, минимальную JSON-форму отказа и требования симметрии вердиктов между `mempool`, `preflight` и `apply`, включая explicit-claim, auto-claim и policy ветки.

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
- Burn: `E_BURN_SCHEMA_INVALID`, `E_BURN_UNITS_INVALID`, `E_BURN_OVER_BALANCE`, `E_BURN_POLICY_REJECT`.
- Import fee: `E_IMPORT_FEE_TOO_LOW`.
- Policy V4: `E_POLICY_SCHEMA_INVALID`, `E_POLICY_NOT_INSTALLED`, `E_POLICY_NOT_ACTIVE`, `E_POLICY_DENIED`, `E_POLICY_SENDER_FILTERED`, `E_POLICY_ROUTING_DENIED`, `E_POLICY_MISSING_COSIGN`, `E_POLICY_RESCUE_REQUIRED`, `E_POLICY_EMERGENCY_COSIGN_REQUIRED`, `E_POLICY_ACCOUNT_FINALIZED`, `E_POLICY_IRREVERSIBLE`.

Семантика кодов стабильна; расширение допустимо только аддитивно.

### Minimal reject JSON shape

Обязательные поля:

- `ok=false`
- `phase` (`mempool|preflight|apply`)
- `tx_kind` (`claim|burn|import|policy|transfer|stake|unstake|init|export`)
- `claim_mode` (`explicit|auto`) для `tx_kind=claim` (для `burn` может отсутствовать)
- `response_class`
- `error.code`
- `error.message`
- `error.trace_id`

Для `IMPORT` при нарушении минимальной комиссии:

- `tx_kind = "import"` (или эквивалентный межшардовый тип в API),
- `error.code = "E_IMPORT_FEE_TOO_LOW"`,
- `response_class = "POLICY_REJECT"`.

Для V4 policy rejects:

- `tx_kind` SHOULD reflect the submitted transaction kind (`policy` for `PolicyTx`, or the original kind for a transfer/stake/init rejected by policy).
- `policy_code` MAY identify the policy enum variant (`routing.same_domain_only`, `routing.emergency_redirect`, `sender_filter`, `default_behavior`, `cosign_required`).
- `policy_phase` MAY be `evaluate`, `activate`, `deactivate`, or `apply`.
- `response_class` MUST be `POLICY_REJECT` for deterministic policy denial and `VALIDATION_ERROR` for malformed policy payloads.

## Error Semantics

- При одинаковом входе и эквивалентном pre-state `preflight` должен вернуть тот же `error.code`, что и `apply`.
- `mempool` обязан совпадать с `apply` для consensus-critical отказов.
- При state drift между preflight и apply допускается другой результат, но только в рамках стабильного набора кодов и классов.
- Для временной недоступности canonical anchor view используется только `E_ANCHOR_STATE_UNAVAILABLE` + `TEMPORARY_UNAVAILABLE`.
- Для auto-claim reject в составе релевантной транзакции узел обязан возвращать claim-класс ошибки (`E_*`) с явным `claim_mode=auto`; источник можно указывать либо как `tx_kind=claim`, либо как исходный `tx_kind` с дополнительным claim-контекстом, но семантика `error.code` должна оставаться claim-совместимой.
- Для `IMPORT` policy должен enforce `import_fee >= MIN_IMPORT_FEE_UNITS` (`0.01 PWM` в целевых единицах), и при reject возвращать `E_IMPORT_FEE_TOO_LOW` без state-mutation.
- Для V4 policy decisions `preflight` и `apply` должны возвращать один и тот же `error.code` при одинаковом pre-state. `evaluate_policy` не мутирует state; если state drift меняет результат к моменту apply, ответ остаётся в стабильном наборе `E_POLICY_*`.
- `E_POLICY_ACCOUNT_FINALIZED` означает, что старый account key больше не авторизует запрошенное действие после irreversible emergency finalization.
- `E_POLICY_EMERGENCY_COSIGN_REQUIRED` означает, что emergency routing activation не содержит валидной подписи rescue address.

## Compatibility

- RFC является API-слоем поверх RFC 0013 и сохраняет трассируемость к его decision classes.
- Контракт рассчитан на аддитивное расширение без breaking-переопределения существующих `error.code`.
- V4 policy codes are additive and MUST NOT change the semantics of existing Claim/Burn/Import codes.

## V5 Addendum: retired ClaimTx wire surface

**Status:** Active for MVP V5.

V5 retires standalone `ClaimTx` and claim/free-day state. The Claim error codes listed above remain historical V2 compatibility labels and are not active validation outcomes for newly submitted V5 transactions.

Normative V5 changes:

- submitted legacy `ClaimTx` is rejected as an unsupported schema/transaction kind, for example `TX_SCHEMA_UNSUPPORTED` or `E_SCHEMA_INVALID` depending on the existing decode layer;
- lazy marks generation during account touch is not reported as `tx_kind=claim` and does not use `claim_mode=explicit|auto`;
- `BURN_MARK` over-balance checks use the effective lazy mark balance after RFC 0012 v2 touch semantics;
- V5 policy rejects for deferred activation use existing V4 policy codes:
  - `ActivatePolicy` before `activate_at_height` -> `E_POLICY_NOT_ACTIVE`;
  - redundant `ActivatePolicy` at or after `activate_at_height` -> `E_POLICY_DENIED` with an "already active" message.

Retired active-scope codes:

- `E_ANCHOR_RANGE_INVALID`
- `E_ANCHOR_CONTINUITY_BROKEN`
- `E_ANCHOR_STATE_UNAVAILABLE`
- `E_CLAIM_UNITS_INVALID`
- `E_CLAIM_OVER_MATURED`
- `E_FREE_CLAIM_DAILY_LIMIT`
- `E_REORG_STATE_MISMATCH` when used only for claim/free-day state

## Out-of-Scope

- Изменение consensus/policy правил за пределами RFC 0011-0013.
- Детали transport-level ретраев и backoff.
- Расширенные UX сообщения клиентов поверх `error.message`.

## References

- [Slice D API contract freeze](../reviews/sprint-v2-1-slice-d-api-contract-freeze.md)
- [Slice C policy matrix freeze](../reviews/sprint-v2-1-slice-c-policy-matrix-freeze.md)
- [Slice E implementation handoff](../reviews/sprint-v2-1-slice-e-implementation-handoff.md)
