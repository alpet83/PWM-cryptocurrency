# Sprint 15 — Slice 6: запуск бенчмарков загрузки снимка (`pwmd`)

## Источник данных

По умолчанию бенч **`snap_load_jsonfile`** и ветка ClickHouse используют **реальные файлы QA-нод**, если они уже есть на диске:

| Файл | Значение по умолчанию | Переменная окружения |
|------|------------------------|----------------------|
| Снимок | `./tmp/state-testnet/pwm-data.json` | `PWM_SNAPSHOT_BENCH_FILE` |
| Genesis bundle | `./tmp/genesis-custom.json` | `PWM_SNAPSHOT_BENCH_GENESIS` |
| Passphrase genesis | `12345` | `PWM_SNAPSHOT_BENCH_GENESIS_PASS` |

Пути совпадают с типичным **`./node-1.ps1`** / **`./node-2.ps1`** из корня репозитория. Если пара genesis+snapshot не загружается, используется **синтетическая** цепочка `dev_net` во временном файле (сообщение в stderr только для synthetic).

## Миграция файла → ClickHouse (`pwmd-ch-snap-import`)

Одноразовый **`INSERT`** строки с тем же каноническим JSON, что пишет runtime **`ch_save`**:

```bash
cargo run -p pwmd --features clickhouse-snapshot --bin pwmd-ch-snap-import -- \
  --genesis-file ./tmp/genesis-custom.json \
  --genesis-passphrase "12345" \
  --snapshot-file ./tmp/state-testnet/pwm-data.json \
  --clickhouse-url http://127.0.0.1:8123 \
  --network-id testnet-qa \
  --domain-hi 0x2C \
  --cluster-id test-cluster-CY \
  --node-id test-node-CY
```

Флаги identity должны совпадать с нодой-источником файла (или задайте **`--snapshot-store-key`** явно). URL можно передать через **`PWM_CLICKHOUSE_URL`**.

Бенч **`snap_load_clickhouse`** при **`PWM_CLICKHOUSE_BENCH_URL`** сам вызывает **`SnapChCfg::import_snapshot_file`** перед замером (строка с **`row_key=s15_slice6_bench`**, БД **`pwm_snapshots`**, таблица **`node_snapshot`** — как в Slice 5 DDL).

## Имена функций Criterion

| Функция | Что измеряет |
|---------|----------------|
| **`snap_load_jsonfile`** | Прод-путь JsonFile на момент запуска бенча; для актуальной архитектуры см. ниже про trust-default tail load. |
| **`snap_decode_trust_state`** | Только парсинг файла + декод без full replay + wire; исторически был небезопасным сравнительным path, теперь близок по смыслу к доверию summary, но не заменяет `validate_snapshot_trusted`. |
| **`snap_validate_full_replay`** | Изолированный **`validate_snapshot`** на уже декодированном снимке (= полный replay как при загрузке). |
| **`snap_load_clickhouse`** | HTTP **`ch_load`** + decode + validate (feature **`clickhouse-snapshot`**). |

Сумма по смыслу: **`snap_decode_trust_state`** + **`snap_validate_full_replay`** ≈ работа **`snap_load_jsonfile`** без повторного чтения файла между двумя бенчами (на практике **`snap_load_jsonfile`** включает I/O чтения файла).

## Несколько нод и один ClickHouse

Разделение — не отдельные таблицы под шард по умолчанию, а **`row_key`** в одной таблице (**`pwm_snapshots.node_snapshot`** или то, что задано в CLI): ключ включает **`network_id`**, домен, **`cluster_id`**, **`node_id`**, digest genesis (`pwmd_snap_row_key`). Разные ноды при корректных identity → разные строки; общая БД допустима. Конфликт возможен только если две ноды используют один и тот же ключ (одинаковая identity + один **`snapshot-store-key`** override).

## Чекпоинт и «хвост блоков» при старте

Историческая запись выше отражала состояние до trust-default startup. Сейчас JsonFile epoch load по умолчанию читает summary `pwm-data.json`, manifest и хвост блоков из `epochs/`, затем вызывает `validate_snapshot_trusted`; полный replay включается `--snapshot-verify-chain` / `PWM_SNAPSHOT_VERIFY_CHAIN` или форсируется при лаге summary относительно manifest. ClickHouse (`ch_load`) остаётся full-replay веткой. См. [`sprint-15-doc-audit-trust-default-arch-shift.md`](./sprint-15-doc-audit-trust-default-arch-shift.md) и [`../guide-node-storage-and-snapshot.md`](../guide-node-storage-and-snapshot.md).

## Команды Criterion

Только JsonFile (без ClickHouse feature):

```bash
cargo bench -p pwmd --bench snapshot_load
```

С веткой ClickHouse (mock HTTP в процессе, Docker не нужен):

```bash
cargo bench -p pwmd --bench snapshot_load --features clickhouse-snapshot
```

Живой ClickHouse:

```bash
set PWM_CLICKHOUSE_BENCH_URL=http://127.0.0.1:8123
cargo bench -p pwmd --bench snapshot_load --features clickhouse-snapshot
```

Убедитесь, что DDL из **`tools/docker/sql/clickhouse_pwm_snapshots.ddl`** применён и база **`pwm_snapshots`** / таблица **`node_snapshot`** существуют.

## Интерпретация вывода Criterion

- Для каждой функции (`snap_load_jsonfile`, `snap_decode_trust_state`, `snap_validate_full_replay`, при feature — `snap_load_clickhouse`) печатается среднее время и доверительный интервал по итерациям.
- Отчёты при включённом `html_reports` у `criterion` — под `target/criterion/` workspace.

## Согласованность с тестами

Юнит-тест **`snap_ch_wire_jsonfile_mock`** (только с `--features clickhouse-snapshot`) проверяет равенство v2 wire после загрузки JsonFile vs ClickHouse; критерий стабильности см. комментарий в модуле теста.
