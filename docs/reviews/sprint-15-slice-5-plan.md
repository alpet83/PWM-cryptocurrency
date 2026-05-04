# Sprint 15 — Slice 5: ClickHouse prototype для snapshot

Цель MVP: **прототип persistence снимка в ClickHouse** (Docker) + **smoke** без регресса JSON-пути по умолчанию.

## Область

1. Реализовать вариант **`SnapshotBackend`** для ClickHouse (HTTP или официальный клиент под **опциональный Cargo feature**, чтобы `cargo test --workspace` без Docker оставался зелёным).
2. Хранить **канонический JSON** того же вида, что и файл (`SnapshotData` / `data_to_v2`), чтобы **`validate_snapshot`** и replay не расходились с Slice 4.
3. Ключ строки (logical key): стабильный идентификатор узла — например связка **`network_id` + node_id + genesis digest** или параметр конфигурации `snapshot_row_key`.
4. Операторский вход: CLI (`--snapshot-backend clickhouse`, URL, опционально БД/таблица) и/или env **`PWM_CLICKHOUSE_HTTP`** / аналог.
5. **`docker-compose`** (или **`tools/docker/`**) для локального ClickHouse + одноразовый DDL (MergeTree или простая таблица `JSON`/строка).
6. Документ: короткий smoke (`docs/reviews/sprint-15-slice-5-smoke.md` или раздел в этом файле).

## Вне scope

- Полная explorer-схема / история всех высот.
- Slice 6 (кросс-бэкенд consistency tests) — только заготовка при желании.

## Приёмка

- Дефолтная сборка: **`cargo fmt --all`**, **`cargo test --workspace`** — PASS без запущенного CH.
- С включённым feature / при наличии CH: описанный smoke PASS или **`#[ignore]`** интеграционный тест с явной инструкцией запуска CH в комментарии.
- **`issues-report.md`** — строка про операторский режим CH при изменении UX.

После кодирования: **pwm-testing** → **pwm-review**.

## Закрытие (конвейер)

- Реализация + Docker DDL/smoke — принято pwm-review (**PASS with nits**).
- Исправления после ревью: DDL **`ORDER BY`** совместим с образом compose (без **`DESC`** в кортеже ключей таблицы); smoke и **`issues-report`** синхронизированы с набором символов **`snapshot-store-key`**.
- Артефакт ревью: `docs/reviews/sprint-15-slice-5-review.md`; статус задачи: **`done_conveyor`**.
