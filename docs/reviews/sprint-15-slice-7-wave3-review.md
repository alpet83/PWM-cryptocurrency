# Sprint 15 Slice 7 — Wave 3 integrated review (bounded tail + ClickHouse incremental)

## 1) Scope recap

- **Ticket:** `tasks/20260503-s15-slice-7-incremental-storage-architecture.json`.
- **Фокус:** `Chain.blocks` как `VecDeque` с `TAIL_BLOCK_CAP = 1000` и eviction после `seal`; ClickHouse — INSERT по блоку, checkpoint каждые 100 блоков, `ch_load` с блоками + replay и fallback на legacy `snapshot_json`; DDL `tools/docker/sql/clickhouse_pwm_snapshots.sql`; `SnapChCfg` (таблицы, БД из сети, ключ строки).
- **Контекст:** `docs/reviews/sprint-15-slice-7-plan.md`, wave2-review (закрытые зазоры: память, CH monolithic).
- **pwm-testing:** fmt / test pwm-core+pwmd (`clickhouse-snapshot`) / check workspace — PASS.

## 2) Requirements fit

**Соответствует заявленному Wave 3:**

- In-memory хвост ограничен **≤ 1000** блоков; `canonical_h` / `tip_hash` от последнего блока, не от длины deque.
- CH seal: строка на блок + checkpoint на интервале 100; relay summary без полного blob цепи (checkpoint-строка).
- Загрузка: блоки из CH → сборка → `validate_snapshot`; пустой блоковый лог → legacy таблица монолита.

**Зазоры относительно полного acceptance Slice 7:**

- **Explorer-oriented поля** (`tx_count`, `shard_balance` в дизайне тикета): могут быть в payload строки блока; отдельная эксплуатационная дока/миграция — при необходимости следующий тикет.

## 3) Style and module shape

- Идентификаторы в зоне правок укладываются в лимит длины сегментов; у ключевых модулей сохранены краткие `//!`.

## 4) Safety

- Replay после загрузки блоков из CH отлавливает дыры и несогласованность состояния.
- **Нит (DDL):** `ReplacingMergeTree` для таблицы блоков с `ORDER BY (height)` без `row_key` — при нескольких логических цепочках в одной физической таблице возможна некорректная дедупликация при merge; оператору нужна одна цепь на таблицу или исправление порядка ключей в DDL (`row_key`, height).
- **Нит (эксплуатация):** при сбое CH после успешного seal узел может уйти в `ready_degraded` при сохранении непрерывности сервиса — мониторинг и процедуры восстановления зафиксировать в ops-доке.
- **Нит:** миграция со старого только-legacy хранилища — порядок заполнения новых таблиц до переключения читателей на стороне процесса.

## 5) Tests

- `chain`: `tail_cap_evicts_old_keeps_tip`.
- `pwmd`: mock/replay тесты snapshot backend; полный цикл INSERT→SELECT→replay без живого CH не автоматизирован (ожидаемо).

## 6) Verdict

**PASS with nits**

Приоритет:

1. **MEDIUM:** DDL / операторская заметка про `row_key` в ключе сортировки блоков при multi-tenant таблице.
2. **MEDIUM:** документировать семантику деградации при ошибках CH после seal.
3. **LOW:** общий хелпер для частичного replay vs полного `validate_snapshot` (следующий рефактор).

## 7) Participation

```json
{
  "agent": "pwm-review",
  "result": "PASS",
  "artifacts": {
    "review_md": "docs/reviews/sprint-15-slice-7-wave3-review.md"
  },
  "token_usage": {
    "source": "estimate",
    "input": null,
    "output": null,
    "total": 4200,
    "confidence": "low"
  }
}
```
