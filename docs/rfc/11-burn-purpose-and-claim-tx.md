# RFC 0011: Burn purpose and Claim transaction schema

**Status:** Active (V2-1 publication pack)  
**Version:** 1.0  
**Slice source:** A (`sprint-v2-1-slice-a-tx-schema-freeze.md`)

## Abstract

RFC фиксирует tx-контракт для V2-1: обязательное поле `purpose` в `BurnMarkTx v2`, baseline-схему `ClaimTx` (включая `free|paid` режим), детерминированную нормализацию строк, а также авто-клейминг как неявный state-эффект релевантных транзакций по балансу.

## Motivation

- Нужна единая tx-семантика до кодинга слайсов E-1/E-2/E-3.
- Требуется убрать двусмысленность валидации `purpose` между preflight/mempool/apply.
- Нужно зафиксировать форму ClaimTx как явный путь materialization накопленных marks и согласовать ее с auto-claim semantics.

## Specification

### BurnMarkTx v2

- `tx_type = "burn_mark"`, `schema_version = 2`.
- `purpose` обязательно.
- Нормализация `purpose_norm`: вход UTF-8, только `trim` по краям, без NFC/NFKC преобразований.
- Ограничение: `1..80` UTF-8 байт после нормализации.
- Запрет control-кодов `U+0000..U+001F` и `U+007F..U+009F`.
- Консенсус трактует `purpose` как непрозрачный текстовый тег; бизнес-интерпретация вне протокола.

### ClaimTx baseline

- `tx_type = "claim_mark"`, `schema_version = 1`.
- Обязательные поля: `account_id`, `nonce`, `mode`, `claim_units`, `anchor_ref`, `fee`, `sig`.
- `mode` из enum: `free|paid`.
- `claim_units` имеет тип `u32`; специальное значение `CLAIM_ALL = u32::MAX` зарезервировано как sentinel «материализовать весь matured».
- Инвариант режима:
  - `mode=free` -> `fee=0`,
  - `mode=paid` -> `fee>0` и прохождение fee policy.

### Explicit claim and auto-claim

- Протокол поддерживает два пути материализации марок:
  - **explicit claim** через `ClaimTx`,
  - **auto-claim** как неявный эффект релевантной транзакции, меняющей баланс монет или марок аккаунта.
- Auto-claim не является отдельной транзакцией и не добавляется в mempool.
- Расчет materialized-дельты для auto-claim обязан быть эквивалентен explicit-claim расчету при том же canonical pre-state.
- Ограничение `free|paid` относится к explicit `ClaimTx`; auto-claim не потребляет отдельный free-slot.

### Deterministic tx processing

- Канонический порядок сериализации начинается с `tx_type`, `schema_version`.
- Для `BurnMarkTx` в подпись входит `purpose_norm`.
- Вердикт валидации обязан быть семантически одинаковым в mempool, preflight и apply при одинаковом state snapshot.

## Validation and Error Semantics

Минимальный стабильный набор кодов для tx/preflight:

- `INVALID_PURPOSE_LENGTH`
- `INVALID_PURPOSE_CHARS`
- `INVALID_PURPOSE_ENCODING`
- `CLAIM_REQUIRED_FIELD_MISSING`
- `CLAIM_MODE_INVALID`
- `CLAIM_FEE_MODE_CONFLICT`
- `CLAIM_NONCE_INVALID`
- `CLAIM_DELTA_INVALID`
- `FREE_CLAIM_DAILY_LIMIT`
- `TX_SCHEMA_UNSUPPORTED`

Коды считаются stable-by-meaning: допускается только аддитивное расширение без переименования существующих кодов в рамках V2-1.

## Compatibility

- Временный переходный режим допускает legacy `BurnMarkTx v1` без `purpose` через adapter path.
- Новые клиенты по умолчанию формируют `BurnMarkTx v2`.
- `ClaimTx` вводится как новый тип и не меняет валидность старых не-claim транзакций.

## Out-of-Scope

- Формулы maturity/state и инварианты reorg (RFC 0012).
- Полный policy matrix по фазам (RFC 0013).
- Финальный wire-формат API reject-ответов (RFC 0014).
- План deprecation legacy v1 после стабилизации сети.

## References

- [Sprint V2-1 RFC inputs](../reviews/sprint-v2-1-rfc-inputs-20260505.md)
- [Slice A tx schema freeze](../reviews/sprint-v2-1-slice-a-tx-schema-freeze.md)
- [Slice E implementation handoff](../reviews/sprint-v2-1-slice-e-implementation-handoff.md)
