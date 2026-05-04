# Sprint 15 — Slice 4: абстракция snapshot storage

Цель MVP-плана: **`JsonFile` baseline + интерфейс под будущий `Db`** без реализации конкретной БД в этом слайсе (ClickHouse → Slice 5).

## Область

1. Ввести контракт загрузки/сохранения снимка (trait или узкий enum), сохранив текущую семантику **`SnapshotData`** / **`validate_snapshot`** / atomic JSON write (`*.tmp` + rename).
2. Реализация **`JsonFile`**: делегирование существующей логики из `snapshot/io.rs` (или перенос тел в модуль `snapshot/store/` без изменения поведения).
3. Точки вызова **`lifecycle`**, **`relay`**, **`api/common`**, **`bootstrap`** перевести на абстракцию; путь из **`PwmdConfig`** остаётся источником для JSON-режима по умолчанию.
4. Заглушка или документированный **`unimplemented!`**/`todo` для **`Db`** допустима только если все match-ветви покрыты дефолтом JSON и компиляция/тесты зелёные — предпочтительно **`SnapshotBackend::JsonFile`** как единственный вариант в enum до Slice 5.

## Вне scope Slice 4

- Реальный драйвер ClickHouse / PostgreSQL.
- Изменение формата JSON на диске или ослабление `validate_snapshot`.

## Приёмка

- `cargo fmt --all`, `cargo test --workspace`.
- Существующие тесты snapshot (`snapshot_roaming.rs` и др.) без регресса.
- Короткая запись в `tasks/*.json` и при необходимости строка в `issues-report.md`.

После merge: делегация **pwm-testing** → **pwm-review**.

## Прогресс

- [x] Код + коммит **`20ee8fa`** (`feat(pwmd): SnapshotBackend abstraction for Slice 4 (JsonFile + Db stub)`).
- [x] pwm-testing — PASS (`cargo fmt --check`, `cargo test --workspace` на `20ee8fa`).
- [x] pwm-review — **PASS with nits**, см. `docs/reviews/sprint-15-slice-4-review.md`.

