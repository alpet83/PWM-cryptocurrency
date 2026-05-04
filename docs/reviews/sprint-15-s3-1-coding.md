# Sprint 15 S3.1 Coding

## Scope
- Реализована пошаговая диагностика cross-shard отправки в `pwm-tui` (F6 send).
- Протокольный контракт `pwmd` не менялся; использованы существующие endpoint'ы `/v1/export-readiness` и `/v1/roaming-intents`.

## Files
- `crates/pwm-tui/src/main.rs`
  - Добавлен обязательный stage preflight через `POST /v1/export-readiness` перед submit roaming intent.
  - Добавлен staged-report из 4 шагов:
    1) `preflight (export-readiness)`
    2) `export submit (roaming/export intent)`
    3) `handoff/provenance register` (с явной операторской подсказкой)
    4) `import submit`
  - На ошибке возвращается точный этап с actionable hint.
  - Для reject с JSON (`code/hint/message`) отображается код (в т.ч. `missing_preflight`) и подсказка.

## Behavior
- Успешный path теперь возвращает многострочный понятный отчёт по этапам с финальным `imported`.
- `missing_preflight` и смежные readiness-reject теперь видны как fail на этапе `export submit`, с прямой подсказкой что делать.
- Если handoff не подтверждён relay-статусом, UI явно говорит про ручной fallback (`tx-handoff-register` -> `tx-import`).

## Tests
- Обновлены существующие roaming-тесты под обязательный readiness preflight.
- Добавлен отдельный тест на диагностику `missing_preflight`:
  - `f6_send_roaming_lifecycle_shows_missing_preflight_stage_hint`
- Проверен success path staged-вывода:
  - `f6_send_roaming_lifecycle_duplicate_to_imported`

## Commands
- `cargo fmt`
- `cargo test -p pwm-tui`

Оба завершились успешно.
