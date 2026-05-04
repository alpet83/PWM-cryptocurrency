# Sprint 15 — Slice 5 ревью (ClickHouse snapshot prototype)

**Коммиты:** первичная реализация см. историю по тегам задачи `20260504-s15-slice-5-clickhouse-snapshot-prototype`; после конвейера — remediation DDL/smoke/issue-sync в том же PR/ветке.

**Вердикт:** **PASS (approve with nits)** → закрыто после remediation ниже.

## Кратко

- Опциональный Cargo feature **`clickhouse-snapshot`**, HTTP-бэкенд **`ch_http`**, ключ строки **`pwmd_snap_row_key`** (override или канонический составной ключ), канонический JSON совместим с **`validate_snapshot`** / файловым путём.
- Docker: **`tools/docker/pwmd-clickhouse-compose.yaml`**, DDL **`tools/docker/sql/clickhouse_pwm_snapshots.ddl`**, smoke **`docs/reviews/sprint-15-slice-5-smoke.md`**.
- Дефолтная сборка без feature и **`cargo test --workspace`** остаются зелёными.

## Ниты (учтённые в remediation)

1. **DDL и версия образа:** в ClickHouse 24.8 некорректен **`ORDER BY (row_key, inserted_at DESC)`** в DDL MergeTree → заменено на **`ORDER BY (row_key, inserted_at)`**; выбор последней версии строки по-прежнему через **`ORDER BY inserted_at DESC LIMIT 1`** в SELECT.
2. **Док ↔ код по символам ключа:** smoke и текст ошибки **`pwmd_snap_row_key`** выровнены с **`is_safe_snap_key_seg`** (символ **`/`** не допускается).
3. **Отложено:** редиректы reqwest, лимит размера ответа, не логировать URL с credentials, расширить операторскую доку по **`--clickhouse-url`**.

HTTP-клиент снимков использует **`no_proxy()`**, чтобы локальный ClickHouse не уходил в системный HTTP-прокси; **`ch_ping_env`** бьёт в **`/ping`**.

## Тестирование

- **pwm-testing:** `cargo fmt --check`, `cargo test --workspace`, `cargo test -p pwmd --features clickhouse-snapshot --lib` — PASS.
- **Compose:** после исправления DDL контейнер стартует; опционально **`PWM_CLICKHOUSE_TEST_URL=http://127.0.0.1:8123`** и **`ch_ping_env`**.
