# Sprint V2-1 / Slice 0 — Practical Checklist (audit)

Цель слайса: зафиксировать расхождения между `docs/plans/mvp_v2.md` и текущей реализацией без продуктовых код-изменений.

## Scope checklist

- [x] Подтверждена целевая модель: один баланс `marks`, `marks_quota` — legacy-заглушка.
- [x] Составлена карта текущего поведения `BURN_MARK` (что списывается, какие проверки, какие побочные эффекты).
- [x] Зафиксированы точки начисления PWM/marks в рантайме (`seal`, `state`, `genesis`).
- [x] Зафиксирован текущий API-контракт аккаунта (`marks` в `pwmd`) и влияние скрытой legacy-логики.
- [x] Сформулирован RFC-first набор минимальных изменений для порогов эмиссии (PWM ~100k stake, marks ~1 stake).

## Evidence checklist

- [x] Аудит покрывает минимум: `crates/pwm-core/src/state.rs`, `chain.rs`, `genesis.rs`.
- [x] Аудит покрывает минимум: `crates/pwmd/src/api/types.rs`, `common.rs`.
- [x] Для каждого finding указан severity (High/Medium/Low) и impacted files.

## Output checklist

- [x] Подготовлен отчёт `docs/reviews/sprint-v2-1-slice-0-spec-impl-audit.md`.
- [x] В отчёте есть: Findings, Impacted files, Proposed next slices, Open questions, Acceptance checklist.
- [x] Обновлён тикет `tasks/20260505-v2-s1-s0-spec-impl-audit.json` (добавлены делегации `pwm-coding`/`pwm-testing`/`pwm-review`, обновлены `notes`).

