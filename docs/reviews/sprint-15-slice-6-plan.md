# Sprint 15 — Slice 6: replay/consistency между backend снимка + бенчмарки загрузки

Цель MVP: **формально проверить**, что канонический снимок даёт одинаковый результат при загрузке через **`JsonFile`** и через **`ClickHouse`** (feature **`clickhouse-snapshot`**), и **измерить сравнимую стоимость загрузки** (latency), без регресса дефолтной сборки.

## Область

1. **Consistency / replay:** тесты на базе фикстур (`pwm-data`-совместимый JSON или минимальный валидный `SnapshotData`), общий **`GenCfg`/genesis digest** там, где требует **`validate_snapshot`** / **`load_snapshot`**. Утверждения: одинаковые декодированные структуры, одинаковые результаты ключевых проверок (или одинаковый хеш сериализованного канона после normalize — выбрать один стабильный критерий в коде).
2. **ClickHouse ветка:** без обязательного Docker в CI — для реального HTTP использовать **`PWM_CLICKHOUSE_TEST_URL`** / отдельный **`PWM_CLICKHOUSE_BENCH_URL`** или **`#[ignore]`** с комментарием; для юнит-уровня допускается mock HTTP (локальный сервер в тесте), если это быстрее стабильнее.
3. **Миграционный мостик файл → БД:** утилита **`pwmd-ch-snap-import`** (binary, `--features clickhouse-snapshot`): валидированный **`pwm-data.json`** → **`INSERT`** в таблицу ClickHouse в том же каноническом JSON, что и runtime **`ch_save`** (`encode_snap_data_txt`). Identity/genesis должны совпадать с нодой-источником (`./node-1.ps1` и т.п.).
4. **Бенчмарки:** по умолчанию брать **`./tmp/state-testnet/pwm-data.json`** + **`./tmp/genesis-custom.json`**, если файлы есть после прогона PS1-нод (переменные **`PWM_SNAPSHOT_BENCH_*`** переопределяют пути); иначе — синтетическая фикстура `dev_net`.
5. Документировать в **`issues-report.md`**, если меняется операторский контракт (env, флаги bench).

## Вне scope

- Slice **6b** (checkpoint / lazy blocks) — отдельный тикет.
- Изменение формата JSON снимка или семантики **`validate_snapshot`** — только если тест выявит баг (тогда минимальный фикс + тест).

## Приёмка

- **`cargo fmt --all`**, **`cargo test --workspace`** — PASS без запущенного ClickHouse.
- С **`--features clickhouse-snapshot`**: новые тесты consistency PASS; при наличии URL — опционально не игнорируемые интеграционные проверки по smoke compose из Slice 5.
- **`cargo bench -p pwmd`** (или целевой `--bench …`): группа сравнивает методы загрузки; документ с командой и интерпретацией.
- Тикет **`tasks/20260506-s15-slice-6-snapshot-backend-replay-benches.json`**: статус **`done_pwm_coding`** после merge.

После кодирования: **pwm-testing** → **pwm-review**.

## Закрытие конвейера

- Статус тикета: **`done_conveyor`**; артефакты: план, bench/testing/review docs.
- Префлайт pwm-testing: **`git_bash_exec`** + **`du -sm target/debug`**, порог **4 GiB** — см. **`docs/AGENT_PROMPT_testing.md`** §Preflight.
