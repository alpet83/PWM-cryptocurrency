# Sprint 14 — Validator Key Multiplicity Review

## Verdict
`approve with nits`

## Ключевой вывод
Текущее дублирование `validator_keys[i]` на каждую `gen_cfg.rows[i]` — это не случайный артефакт, а жёсткий контракт текущей архитектуры (loader + chain rotation + signature mapping).

## Что это значит
- Модель “1 валидатор по умолчанию + N premine-holders” сейчас не является «мелкой доработкой».
- Для такого поведения нужен отдельный архитектурный срез (разделение `validators[]` и `rows[]`).

## Оценка вариантов
- **A (текущий, row-aligned)**: проще и стабильнее сейчас; меньше риск регрессий.
- **B (single default validator + optional extras)**: лучше для будущего, но требует schema/runtime refactor, а не точечной правки.

## Рекомендация сейчас
Оставить текущую модель A в текущем спринте, но явно зафиксировать в docs, что это intentional coupling.

## Если переходить на B
Нужен отдельный slice/ADR:
1) новая схема (`validators[]` отдельно от `rows[]`),  
2) обновление loader/chain ротации,  
3) e2e тесты на `1 validator + N premine rows`.
