# RFC 0012: Claim maturity and state model

**Status:** Active (V2-1 publication pack)  
**Version:** 1.0  
**Slice source:** B (`sprint-v2-1-slice-b-state-freeze.md`)

## Abstract

RFC фиксирует state-модель для claim-механики: релевантный баланс для maturity, семантику `anchor_ref` и `claim_units`, правило сброса непрерывности при любом изменении stake, авто-клейминг при релевантных транзакциях по балансу монет/марок, один free-claim маркер на UTC-day и replay-correct поведение при reorg/rollback.

## Motivation

- Нужна единая state-семантика до внедрения policy/API.
- Требуется исключить недетерминизм free-claim и anchor-проверок.
- Нужно зафиксировать минимальный набор replayable полей состояния для совместной реализации в consensus и API.
- Нужно уменьшить количество «пустых» claim-транзакций за счет авто-клейминга на релевантных балансных событиях.

## Specification

### Maturity base

- Релевантный баланс для maturity: только `staked_pwm_units`.
- Любое ненулевое изменение `staked_pwm_units` прерывает текущий непрерывный интервал maturity.
- `staked_pwm_units` не является переводимым балансом: перемещение value из stake выполняется только через `UNSTAKE` (или эквивалентные stake-governance операции), а не через `TRANSFER`.
- Получение эмиссии монет и materialization марок требует участия в stake-контуре (`STAKE`/`UNSTAKE` lifecycle и валидная stake-состояние).

### Claim anchor and units

- `anchor_ref` трактуется как опорная высота для детерминированного replay.
- Валидный диапазон: `anchor_ref <= inclusion_height`.
- Монотонность: `anchor_ref >= last_claim_anchor_ref(account)`.
- `claim_units` — целая материализуемая дельта (`u32`) и должна быть в диапазоне `0 < claim_units <= matured_units_available`.
- Sentinel `CLAIM_ALL = u32::MAX` означает «материализовать весь доступный matured на момент apply».

Формула materialization для v2 baseline (нормализация к целым PWM):

- `hours = floor(delta_seconds / 3600)` (целое).
- `whole_pwm_staked = floor(staked_raw / 1_000_000)`.
- `matured_units_available = whole_pwm_staked * hours`.
- Эквивалент: `1 whole PWM staked for 1 hour = 1 mark`.
- Для `staked_raw < 1_000_000` maturity за час равен `0` (integer truncation).

### Auto-claim trigger semantics

- Материализация марок выполняется:
  - явной `ClaimTx`, или
  - неявно (auto-claim) в составе **релевантной stake-management транзакции**, которая меняет баланс монет или марок аккаунта в рамках stake/claim контура.
- Релевантная транзакция обязана использовать тот же детерминированный расчет matured-дельты, что и явный claim-путь.
- Для auto-claim не создается отдельная транзакция и отдельная запись в mempool: материализация выполняется в том же state-transition шаге, что и базовая релевантная транзакция.
- Если в одном блоке для аккаунта есть несколько релевантных транзакций, расчет и применение materialized-дельты должны быть эквивалентны последовательному replay по порядку транзакций блока.
- В случае нулевой дельты (`matured_units_available == 0`) auto-claim не выполняется, а базовая транзакция продолжает обрабатываться без ошибки claim-пути.

### Free-claim day marker

- Состояние хранит один маркер `last_free_claim_utc_day`.
- День вычисляется только из chain time: `utc_day = floor(block_unix_time_utc / 86400)`.
- Повторная free-claim в том же `utc_day` отклоняется; paid fallback сохраняется.
- Ограничение «одна free-claim в сутки» применяется к **явному** claim-пути. Auto-claim в составе релевантной транзакции не требует отдельного free-slot, так как не является самостоятельной claim-транзакцией.

### Reorg and rollback baseline

- Claim/free state полностью rollback/replay-able.
- Эффекты orphaned-ветки не сохраняются.
- Один и тот же canonical префикс блоков обязан давать одинаковое claim-state.

## Validation Semantics

Нормативные инварианты:

- `last_claim_anchor_ref` не убывает.
- Over-claim запрещен.
- Любое изменение релевантного баланса сбрасывает непрерывность.
- Не более одной успешной free-claim на аккаунт в одном `utc_day`.
- Источник времени для free-day только chain time.
- После reorg не остается побочных claim/free эффектов orphaned-ветки.
- Auto-claim и explicit-claim на одинаковом canonical порядке блоков дают одинаковый итоговый `marks` и claim-state.

## Compatibility

- Внутреннее представление state может отличаться, если внешняя семантика RFC сохранена.
- RFC совместим с tx-контрактом RFC 0011 и задает state-основу для policy/API RFC 0013/0014.

## Out-of-Scope

- Финальный фазовый policy-order и mapping классов ошибок (RFC 0013).
- Wire-формат reject-ответов (`response_class`, `trace_id`) (RFC 0014).
- Экономические расширения вне Sprint V2-1.

## References

- [Sprint V2-1 RFC inputs](../reviews/sprint-v2-1-rfc-inputs-20260505.md)
- [Slice B state freeze](../reviews/sprint-v2-1-slice-b-state-freeze.md)
- [Slice A tx freeze](../reviews/sprint-v2-1-slice-a-tx-schema-freeze.md)
- [Slice E implementation handoff](../reviews/sprint-v2-1-slice-e-implementation-handoff.md)
