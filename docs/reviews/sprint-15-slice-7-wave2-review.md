# Sprint 15 Slice 7 — Wave 2 integrated review (JsonFile incremental + checkpoint summary)

## 1) Scope recap

- **Ticket:** `tasks/20260503-s15-slice-7-incremental-storage-architecture.json`.
- **Фокус:** JsonFile incremental — append блоков в epoch JSONL (`epochs/block_e*.json`), manifest (`epochs/pwm-epochs-manifest.json`), checkpoint/summary в `pwm-data.json` без полного `blocks[]`; seal path и relay summary.
- **Контекст:** `docs/reviews/sprint-15-slice-7-plan.md`, `docs/reviews/sprint-15-slice-7-checklist.md`.
- **Проверены:** `snapshot/incremental.rs`, `snapshot/io.rs`, `snapshot/store.rs`, `snapshot/types.rs`, `snapshot/epoch.rs`, `lifecycle.rs`, `relay.rs`, `issues-report.md`, `pwmd/Cargo.toml`.
- **pwm-testing:** fmt / test / check / bench `--no-run` — PASS.

## 2) Requirements fit

**Соответствует заявленному Wave 2:**

- Seal JsonFile идёт через append в epoch на каждый sealed block; на границе `height % SNAP_CHK_BLK_IV == 0` — checkpoint summary в `pwm-data.json` (`blocks_stored: epochs`, пустой `blocks`, `checkpoint_height = tip`).
- Загрузка: при epoch-режиме блоки собираются из manifest + epoch-файлов, затем полный replay через `validate_snapshot`.
- Legacy inline `blocks[]` и миграции v0/v1/v2 сохранены.
- Relay без локального seal: после import вызывается `save_tip_summary` (согласовано с `issues-report`).

**Зазор относительно acceptance тикета Slice 7:**

- Пункт **in-memory chain block cache ≤ 1000** в этом диффе **не закрыт** (`Chain.blocks` по-прежнему полный `Vec<Block>`). Это отдельная волна или корректировка acceptance.

## 3) Style and module shape

- Нарушений **≤ 5 сегментов** в затронутых символах не замечено.
- У ключевых модулей есть краткие `//!`.

## 4) Safety

- Epoch/manifest: **tmp → write → fsync → rename** — соответствует design-lock.
- Порядок epoch затем manifest; при расхождении загрузчик может fail-fast по `canonical_h` vs числу восстановленных блоков.
- **Нит:** опционально `fsync` каталога после rename (LOW, зависит от FS/деплоя).
- **Нит MEDIUM:** каждый seal может пересобирать большое тело epoch-файла в память — масштабный край для больших эпох.

## 5) Tests

- Есть round-trip legacy и прогон **105** блоков с checkpoint на границах интервала.
- Рекомендуется добавить: высота **> 1000** (вторая эпоха), негативы разрыва manifest/файлов.

## 6) Verdict

**PASS with nits**

Приоритет:

1. **HIGH (acceptance Slice 7):** bounded in-memory blocks ≤ 1000 — не в этом diff; следующая волна или пересмотр тикета.
2. **MEDIUM:** память/CPU при больших epoch bodies на каждый seal.
3. **LOW:** fsync dir, расширенные crash/recovery тесты.

## 7) Participation

```json
{
  "agent": "pwm-review",
  "result": "PASS",
  "artifacts": {
    "review_md": "docs/reviews/sprint-15-slice-7-wave2-review.md"
  },
  "token_usage": {
    "source": "estimate",
    "input": 95000,
    "output": 2900,
    "total": 97900,
    "confidence": "medium"
  }
}
```
