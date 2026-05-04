# Sprint 15 — hotfix: DO цепочка «обнуляется» после межшардового перевода

**Статус (2026-05-02):** root cause найден и исправлен в **`c341ea1`** (`validate_snapshot`: перед `Import` подставляется provenance из `snapshot.state.exported_registry`, если она попала только через **handoff**, без локального `Export` в блоках). Дополнительно: изоляция дефолтного пути Neutral по **`--listen`** (`state/neutral/<listen-tag>/pwm-data.json`). Этот документ добавлен в **`8e3d230`**.

**Приоритет:** был выше абстракции snapshot-storage (Slice 4); после merge можно продолжать Slice 4.

## Симптомы (от оператора)

- После успешной межшардовой транзакции **зона DO в TUI** перестаёт показывать загрузку блокчейна (или показывает неконсистентно).
- После **перезапуска** ноды DO высота/состояние **сбрасываются к genesis** («ноль»).

## Подтверждённая причина

`validate_snapshot` воспроизводил только транзакции из `blocks`. Provenance для `Import` на целевом шарде могла попасть в `exported_registry` через **`/v1/export-provenance` (handoff)** без транзакции `Export` в локальных блоках → replay падал → `load_snapshot` Err → узел оставался на genesis.

## Исторические гипотезы (до разбора)

1. **`load_snapshot` → `validate_snapshot` Err** — подтверждено как основная линия для handoff-only provenance.
2. **Neutral + общий `pwm-data.json`** — исправлено разнесением пути по listen-тегу (см. README «Storage Layout»).
3. Порча JSON / UI-only регресс — при необходимости отдельные тикеты.

## Связь с чек-поинтами (Slice 6b)

Регрессия **`snap_rt_handoff_import_ok`** (`snapshot_roaming.rs`) — базовый эталон для будущего отката на checkpoint; формализацию ordering handoff vs блоков — по мере Slice 6b.

## Приёмка hotfix-слайса

- [x] Автотест handoff + Import replay (`snap_rt_handoff_import_ok`).
- [x] `cargo test --workspace` (pwm-testing на **`8e3d230`**).
- [x] Запись в `issues-report.md` (операторские пути Neutral).
- [x] pwm-review: **PASS with nits** — `docs/reviews/sprint-15-hotfix-do-after-xshard-review.md`.
