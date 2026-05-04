# Sprint 14 — Final closeout snapshot (Slices 0-4)

Дата: 2026-04-28
Статус: functionally complete

## Per-slice outcome

- Slice 0 (spec/audit): завершен аудит схемы кошелька и зафиксирован RFC для v3 формата.
- Slice 1 (core/cli schema path): реализованы чтение/миграционный путь v3 plaintext_dev и валидации derivation path/index; baseline совместимость сохранена.
- Slice 2 (CLI ops): закрыты операторские команды `wallet account list|add|use`, подтверждена совместимость UX и v2 flow.
- Slice 3 (TUI accounts panel): левая панель TUI показывает все аккаунты и активный аккаунт, без утечки секретов в логах в рамках review scope.
- Slice 4 (closeout stabilization): добавлен targeted negative path для `active_account_id_hex` mismatch/invalid path в загрузке v3 wallet; подтвержден clean error без panic.

## Key RFC decisions (Sprint 14)

- v3 schema: принят формат wallet v3 как целевой для multi-address wallet сценария.
- `id_pretty`: используется как человекочитаемый идентификатор аккаунта/записи для UX-слоя и операторских операций.
- payload A: зафиксирован как опорный payload-вариант для спринтового контура и демо-потока.
- active account behavior: активный аккаунт должен быть явно и безопасно резолвим; при mismatch/invalid path ожидается контролируемая ошибка, а не panic.

## Acceptance checklist summary

- Конвейер `pwm-coding -> pwm-testing -> pwm-review` для Slice 0..4 отмечен как завершенный в `sprint-14-checklist.md`.
- Демо-ready критерий (v3 wallet, 2 accounts, переключение и отправка через `use`, отображение в TUI) зафиксирован.
- Финальный review verdict по Slice 4: `approve with minor`.

## Residual risks

- Процессный риск дрейфа checklist/evidence между этапами остается возможным при ручной синхронизации.
- Негативные проверки на `active_account_id_hex` покрывают closeout-case; при необходимости можно расширить набор кейсов валидным hex с логическим mismatch.

## Evidence pointers (Slice 1..4)

- Slice 1: `docs/reviews/sprint-14-checklist.md` (секция Slice 1; testing/review outcomes отражены там же).
- Slice 2: `docs/reviews/sprint-14-slice2-review.md`.
- Slice 3: `docs/reviews/sprint-14-slice3-review.md`.
- Slice 4 testing: `docs/reviews/sprint-14-slice4-testing.md`.
- Slice 4 review: `docs/reviews/sprint-14-slice4-review.md`.
