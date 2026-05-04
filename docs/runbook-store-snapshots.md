# Runbook: хранение снимков (JsonFile + ClickHouse)

## ClickHouse: подготовка схемы

1. Поднять ClickHouse (например `tools/docker/pwmd-clickhouse-compose.yaml` в репозитории, если используется в проекте).
2. Подставить плейсхолдер `{database}` в `tools/docker/sql/clickhouse_pwm_snapshots.sql` (или применить шаблон, принятый в вашем compose) и выполнить SQL в кластере.
3. Убедиться, что для физического кластера и домена создаются таблицы `blocks__0xHH`, `checkpoints__0xHH`, `validators_accept__0xHH` с `row_key` в `ORDER BY` (см. `docs/reviews/sprint-15-slice-7-plan.md` §3).

## Row key и идентичность цепочки

- Логический ключ строки: `row_key` = `pwmd_snap_row_key` (по умолчанию `network_id|0x{domain_hi}|{hex(genesis state0 digest)}`, либо override `--snapshot-store-key` / `PWM_SNAPSHOT_STORE_KEY`).
- Несколько нод в одной БД допустимы, если `row_key` различается; коллизия по одному ключу перезатрёт «чужой» снимок при той же физической таблице.
- Семантика загрузки различается: JsonFile по умолчанию доверяет summary+manifest и грузит tail, а ClickHouse сейчас остаётся full-replay backend; подробнее см. `guide-node-storage-and-snapshot.md`.

## Снижение доступности ClickHouse

- `pwmd` с `--snapshot-backend clickhouse` требует рабочий HTTP endpoint; при ошибке INSERT снимок в оперативной памяти остаётся, но персистентное состояние в CH отстаёт — смотрите логи `clickhouse snapshot INSERT http …`.
- Не отключайте ноду длительно при критичной персистенции: для MVP приоритет — JsonFile, пока нет отдельного outbox/очереди к CH.

## Импорт файла в ClickHouse (миграция / бенч)

```text
cargo run -p pwmd --features clickhouse-snapshot --bin pwmd-ch-snap-import -- \
  --genesis-file <path> --genesis-passphrase <pass> \
  --snapshot-file <path> --clickhouse-url http://127.0.0.1:8123 \
  --network-id <id> --domain-hi 0x2C --cluster-id <c> --node-id <n>
```

Имена таблиц `blocks` / `checkpoints` / `validators_accept` выводятся из `--domain-hi` и `--clickhouse-table` так же, как у ноды-источника; на выходе печатается `validators_accept=…`.

## См. также

- `docs/reviews/sprint-15-slice-6-bench.md` — бенчмарки загрузки.
- `docs/reviews/sprint-15-slice-7-plan.md` — контракт checkpoint / `shard_balance` / `validators_accept`.
