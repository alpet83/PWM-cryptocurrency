# Sprint 15 — Slice 7 Wave 4 integrated review

## 1) Scope recap

- **Ticket:** `tasks/20260503-s15-slice-7-incremental-storage-architecture.json`.
- **Фокус:** `shard_balance` JSON на checkpoint (§6.1 плана), `SnapChCfg.table_validators_accept` + DDL `validators_accept__*`, отложенный INSERT с одноразовым `tracing::warn`, runbook хранилища, шаблон bench-отчёта, правки DDL (`ORDER BY` с `row_key`), колонки checkpoint с `state_root` / `shard_balance`.
- **Исправление:** `blocks_cover_full_history` — обход `VecDeque` через `iter().next()` вместо несовместимого с `Vec` API.

## 2) Requirements fit

- **shard_balance:** формула и момент расчёта зафиксированы в плане §6.1; сериализация через `BTreeMap` даёт стабильный порядок ключей `"0xHH"`.
- **validators_accept:** таблица в SQL + имя в конфиге; запись в Wave 4 намеренно не выполняется — предупреждение один раз на процесс; ориентир `checkpoint_digest` — `hex(pwm_core::digest(state))`.
- **Bench doc:** таблица результатов для локального заполнения; явно отмечено отсутствие отдельного bench «checkpoint + хвост» до соответствующего загрузчика.

## 3) Verdict

**PASS with nits**

1. **LOW:** Заполнить числа в `sprint-15-slice-7-wave4-bench.md` при наличии фикстур / живого CH.
2. **LOW:** При появлении консенсусных подписей — заменить warn на реальный INSERT в `validators_accept__*`.

## 4) Participation

```json
{
  "agent": "pwm-review",
  "result": "PASS",
  "artifacts": ["docs/reviews/sprint-15-slice-7-wave4-review.md"]
}
```
