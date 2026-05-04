# Sprint 14 — Slice 11 design review (decoupled genesis schema)

## Verdict
`approve with nits`

## Обоснование применением
Текущий дизайн жёстко сцепляет `premine rows` и `validator set` (1:1 индексно), из-за чего неудобны реальные сценарии:
- 1 validator + много premine holders,
- независимая ротация валидаторов,
- кастомная маршрутизация reward.

## Целевая модель (v4)
- Разделить роли:
  - `funding.rows` — только initial balances,
  - `validators.set` — producer keys / rotation,
  - `reward_policy` — куда начислять награду.
- Убрать инвариант `validators.len == funding.rows.len`.

## Что менять в runtime
- Producer selection и block signature verification — только по `validators.set`.
- Reward accounting — через `reward_policy` (default: `to_producer_account`).

## Миграция
- One-way v3 -> v4 (pre-public допустимо).
- Отдельная команда миграции/верификации genesis.

## Checklist (кратко)
- `pwm-core/pwmd`: новая схема и инварианты.
- `pwm-cli genesis-build`: role-separated UX.
- Тесты: `1 validator + N funding rows`, `M validators + custom reward recipients`, e2e replay/snapshot.
