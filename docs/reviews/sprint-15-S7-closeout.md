# Sprint 15 — S7: закрытие спринта и decision gate

**Дата:** 2026-05-04  
**Чеклист-источник:** [sprint-15-checklist.md](sprint-15-checklist.md) §«S15-S7 — Sprint closeout и decision gate».

---

## 1. Краткое резюме

Sprint 15 по плану [mvp_v1_testnet_multi-sprint.md](../plans/mvp_v1_testnet_multi-sprint.md) (Scope A–C) доведён до **документированного closeout**: межшардовый контур, genesis-guardrails, абстракция снимков, прототип ClickHouse, согласованность replay между backend и инкрементальное хранение (Slice 7, волны 1–4) зафиксированы кодом, тестами и ревью. Крупные production-риски ClickHouse (объём записи, блокирующий HTTP в runtime) **не закрыты в этом спринте** и перенесены в backlog совместно с explorer-readiness и Slice 6b.

---

## 2. Что закрыто по Sprint 15

### 2.1 Трек межшардовой согласованности (S3.x, в т.ч. S3.12 / S3.16 / S3.17)

- **S3.12 closeout** — зафиксирован в [sprint-15-s3-12-9-closeout.md](sprint-15-s3-12-9-closeout.md): peer sessions, foreign balance/init, стабильность wire; явно отмечено, что режим **«одного окна»** — временная модель MVP.
- **S3.16 / S3.17** — итоги и операторский контур: [ROAMING_COMPLETION.md](../ROAMING_COMPLETION.md), [sprint-15-s3-17-closeout.md](sprint-15-s3-17-closeout.md), связанные правки `pwm-tui` / `pwmd` и гайды по тестированию (см. тикеты в `tasks/20260501-s15-slice3-17-*.json` в оркестраторском контуре).
- **Слайс O (кодовая база)** — не часть нумерации S1–S7, но выполнен как подготовительный: [sprint-15-slice-O-checklist.md](sprint-15-slice-O-checklist.md) (декомпозиция god-files, отдельно от S4+).

### 2.2 Снимки: S4 — S6 (абстракция, CH-прототип, e2e между backend)

- **S4 — абстракция store:** тикет `tasks/20260503-s15-slice-4-snapshot-store-abstraction.json`, план/ревью: [sprint-15-slice-4-plan.md](sprint-15-slice-4-plan.md), [sprint-15-slice-4-review.md](sprint-15-slice-4-review.md) — `JsonFile` baseline, опциональный `Db`, явный селектор backend.
- **S5 — ClickHouse prototype:** `tasks/20260504-s15-slice-5-clickhouse-snapshot-prototype.json`, [sprint-15-slice-5-plan.md](sprint-15-slice-5-plan.md), [sprint-15-slice-5-review.md](sprint-15-slice-5-review.md), [sprint-15-slice-5-smoke.md](sprint-15-slice-5-smoke.md).
- **S6 — replay / согласованность + бенчи:** [sprint-15-slice-6-review.md](sprint-15-slice-6-review.md) (**вердикт PASS**), [sprint-15-slice-6-bench.md](sprint-15-slice-6-bench.md), [sprint-15-slice-6-testing.md](sprint-15-slice-6-testing.md), тикет `tasks/20260506-s15-slice-6-snapshot-backend-replay-benches.json` — в т.ч. `pwmd-ch-snap-import`, Criterion `snapshot_load`, mock-согласованность wire JsonFile vs CH.

### 2.3 Инкрементальное хранение — Slice 7, волны 1–4

Сводный тикет: `tasks/20260503-s15-slice-7-incremental-storage-architecture.json`.

- **Пред-архитектура:** [sprint-15-slice-7-pre-architecture-review.md](sprint-15-slice-7-pre-architecture-review.md) (частичное согласие дизайна, перечень обязательных уточнений по canonical height, checkpoint+tail, Json epochs, CH schema).
- **Волны 1–4:** план [sprint-15-slice-7-plan.md](sprint-15-slice-7-plan.md); ревью wave1–wave4 (`sprint-15-slice-7-wave*-review.md`); бенч-шаблон [sprint-15-slice-7-wave4-bench.md](sprint-15-slice-7-wave4-bench.md).
- **Коммиты (из оркестраторских записей тикета Slice 7):** в числе прочих задействованы **`cb3dc17`**, **`551ce84`** (wave4: DDL/`validators_accept`, `shard_balance`, runbook).
- **Операторская документация:** [runbook-store-snapshots.md](../runbook-store-snapshots.md); DDL: `tools/docker/sql/clickhouse_pwm_snapshots.sql`.
- **Расширенные архитектурные замечания по CH:** [sprint-15-ch-storage-architecture-review.md](sprint-15-ch-storage-architecture-review.md), [sprint-15-ch-data-model-scaling-review.md](sprint-15-ch-data-model-scaling-review.md) — зафиксированы ограничения прототипа (частота записи, monolithic JSON blob, blocking client).

---

## 3. Остаточные риски чеклиста §5 (R1–R5)

| ID | Риск | Статус на закрытии S15 |
|----|------|-------------------------|
| **R1** | Export без строгого target readiness preflight | **Снижен:** preflight/readiness и пошаговый UX по сценарию переноса зафиксированы в контур спринта; регрессии — через эксплуатационные регресс-пакеты следующих спринтов. |
| **R2** | Genesis/hash mismatch обнаруживается поздно | **Снижен:** guardrails статуса/genesis и запрет «тихого» здорового состояния при mismatch — по приёмке S15-S3 и связанным правкам `pwmd`. |
| **R3** | Foreign-балансы воспринимаются как authoritative | **Остаточный (medium→low):** семантика полей и маркеры — закрыты по приёмке S2; **режим «одного окна»** осознанно временный ([sprint-15-checklist.md](sprint-15-checklist.md) §S3.12). Дисциплина оператора и будущий explorer-слой остаются в backlog. |
| **R4** | DB backend вносит nondeterminism в replay | **Под контролем:** канонический JSON wire и тестовая эквивалентность путей (Slice 6); инкрементальный CH-путь требует сохранения того же контракта волн 3–4 — регрессии отслеживать в CI/тестах `pwmd`. |
| **R5** | Ops-сложность ClickHouse vs ценность MVP | **Принят как прототипный:** для продакшн-нагрузки см. storage/scaling reviews; baseline **`JsonFile`** остаётся безопасным откатом (rollback **RB3**, [sprint-15-checklist.md](sprint-15-checklist.md) §6). |

---

## 4. Carry-over backlog (после Sprint 15)

1. **Explorer-readiness** — колоночная модель блоков/checkpoints, поля для индексации транзакций/балансов шарда; см. pre-arch Slice 7 §explorer-oriented fields.
2. **`validators_accept` signing** — DDL и конфигурация зафиксированы в Slice 7 wave4; полная криптографическая/протокольная история подписей validator-set — вне scope закрытого MVP-демо спринта.
3. **Опционально Slice 6b** — checkpoint-only bootstrap и lazy-хвост блоков при старте ([sprint-15-slice-6-bench.md](sprint-15-slice-6-bench.md) §«Чекпоинт и хвост»); пересекается с инкрементальными epoch/checkpoint в Slice 7 и требует отдельного gate по корректности `tip_h`/canonical height.
4. **Perf / масштабирование CH** — уменьшение частоты/размера INSERT, неблокирующий I/O, редизайн строки на «block row», см. [sprint-15-ch-storage-architecture-review.md](sprint-15-ch-storage-architecture-review.md).

---

## 5. Вердикт по gate (coding / testing / review)

| Gate | Вердикт | Комментарий |
|------|---------|-------------|
| **Coding** | **PASS** | Контур спринта S15 (включая Slice 7 waves 1–4) реализован и смержен; известные ограничения CH задокументированы. |
| **Testing** | **PASS** | По slice-тикетам: workspace tests, feature-gated CH, бенчи `--no-run` где требовалось; см. `sprint-15-slice-6-testing.md` и делегации Slice 7. |
| **Review** | **PASS (with nits)** | Архитектурные reviews CH и pre-Slice-7 — **PARTIAL/HIGH** на отдельные будущие изменения, не как блокер закрытия спринта по MVP-demo критерию. |

По **negative checks** чеклиста S15-S7: block-level findings без владельца не остаются — перечисленные темы имеют явный перенос в §4.

---

## 6. GO / NO-GO и demo-ready (MVP plan)

- **Решение по sprint gate:** **GO** — переход к следующей итерации roadmap допустим при условии учёта carry-over §4 и мониторинга R3/R4/R5 в планировании Sprint 16+.
- **Demo-ready для MVP internal testnet:** **Да**, в смысле **воспроизводимого демо** межшардового сценария и снимков (JSON + опционально CH) по принятым runbook/тестам — **не** как обещание production-grade explorer или горизонтального масштабирования CH без доработок.

---

## 7. Ключевые артефакты (навигация)

| Тема | Документ / путь |
|------|-----------------|
| План спринта (genesis + snapshots) | [sprint-15-architecture-genesis-consistency-and-db-snapshots.md](../plans/sprint-15-architecture-genesis-consistency-and-db-snapshots.md) |
| Чеклист Sprint 15 | [sprint-15-checklist.md](sprint-15-checklist.md) |
| Runbook снимков | [runbook-store-snapshots.md](../runbook-store-snapshots.md) |
| Slice 6 | [sprint-15-slice-6-review.md](sprint-15-slice-6-review.md), [sprint-15-slice-6-bench.md](sprint-15-slice-6-bench.md) |
| Slice 7 | `tasks/20260503-s15-slice-7-incremental-storage-architecture.json`, [sprint-15-slice-7-pre-architecture-review.md](sprint-15-slice-7-pre-architecture-review.md) |
| CH scaling | [sprint-15-ch-storage-architecture-review.md](sprint-15-ch-storage-architecture-review.md) |
| Роуминг closeout | [sprint-15-s3-17-closeout.md](sprint-15-s3-17-closeout.md), [ROAMING_COMPLETION.md](../ROAMING_COMPLETION.md) |

---

_Ticket оркестратора:_ `tasks/20260504-s15-S7-sprint-closeout.json`.
