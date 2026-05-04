# Sprint 15 checklist (cross-shard consistency + genesis guardrails + optional DB snapshots)

Конвейер на каждый код-слайс: `pwm-coding` -> `pwm-testing` -> `pwm-review` -> решение оркестратора.

## 1) Scope и цели спринта

- Закрыть системные нестыковки межшардового потока `EXPORT -> handoff/provenance -> IMPORT`.
- Убрать двусмысленность foreign-balance видимости (локальный view vs authoritative home-shard truth).
- Ввести guardrails равенства genesis/hash при подключении шардов и явные диагностики.
- Подготовить optional snapshot backend abstraction: `JsonFile` baseline + `Db` интерфейс; прототип `ClickHouse` как кандидат.
- Закрыть sprint e2e-проверками, negative-наборами и closeout-решением.

## 2) Out of scope

- Полный production rollout DB backend (нужен только прототип и smoke).
- Изменение базовой модели консенсуса/финалити.
- Расширение в explorer/UI beyond sprint acceptance.

## 3) Слайсы (полные, без micro-nits)

### S15-S3.12 track — closeout (S3.12.9)

Трек **S15-S3.12** (peer sessions, foreign balance/init через trusted path, wire/decode стабильность, live validation на `node-*.ps1`) закрыт документально в **`docs/reviews/sprint-15-s3-12-9-closeout.md`**.

**Важно для архитектуры:** режим **«одного окна»** (упрощённое наблюдение чужого шарда через trusted peer + RPC без отдельного read-слоя) — **временное решение MVP**. Он **не масштабируется**: при массовом использовании может **перегружать сеть** и нагрузку на ноды. Для последующих релизов целесообразны **централизованные read-сервисы** (global explorer) и **подписка клиента** на обновления по адресам. Реализация **federation table HTTP** (`GET /v1/federation/shards` и смежное) остаётся в **S15-S3.13**.

### S15-S3.16 / S3.17 — cross-shard credit, observability, closeout

- **S3.16 cycle2:** TUI завершение Import после `relayed`, шаг 5 баланса; `pwmd` логи relay/handoff/snapshot digest; расследования: `docs/reviews/sprint-15-s3-16-do-snapshot-root-cause.md`, `docs/reviews/sprint-15-s3-16-cycle2-relay-journal-review.md`.
- **S3.17:** итог отладки в **`docs/ROAMING_COMPLETION.md`**, closeout **`docs/reviews/sprint-15-s3-17-closeout.md`**, тикет `tasks/20260501-s15-slice3-17-roaming-completion-closeout.json`, актуализация `ROAMING-SAMPLE`, `pwm-tui`, `pwmd`, `rfc/9`, tester guide, `MVP-checklist`.

### S15-O — оптимизация кодовой базы (жирные файлы, дубли)

Внеочередной слайс **перед** продолжением крупной реализации по плану. Источник: **`docs/CODEBASE_REFACTORING.md`** (сторонний аудит). План и безопасный чеклист: **`docs/reviews/sprint-15-slice-O-plan.md`**, **`docs/reviews/sprint-15-slice-O-checklist.md`**. Тикет: **`tasks/20260502-s15-slice-O-codebase-cleanup.json`**. Полная декомпозиция god-files — только под-слайсами **O.x**, не смешивать с **S15-S4** без gate.

### S15-S0 — Architecture freeze и task contract

**Цель слайса**
- Зафиксировать единый контракт по семантике балансов, preflight/readiness и storage backend границам.

**Acceptance criteria**
- Есть согласованный sprint contract с явно зафиксированными полями: `local_state_balance`, `authoritative_home_balance`, `spendable_on_this_shard`.
- Есть явный список in-scope/out-of-scope и границы rollback на последующие слайсы.
- Оркестратор публикует task IDs/порядок `S15-S1..S15-S7`.

**Минимальные negative checks**
- Проверка, что ни один downstream slice не стартует без freeze-документа.
- Проверка, что нет конфликтующих формулировок балансовой семантики между планами и review.

---

### S15-S1 — Cross-shard transfer/state consistency hardening

**Цель слайса**
- Стабилизировать операторский и протокольный путь `EXPORT -> IMPORT` без зависших/непрозрачных состояний.

**Acceptance criteria**
- Source-side preflight перед `EXPORT` обязателен и возвращает явный статус readiness target-side.
- Ошибки handoff/import классифицированы по понятным категориям (retryable vs terminal) в runtime/API.
- E2E happy-path `CY -> DO` воспроизводим по runbook без ручных обходов.

**Минимальные negative checks**
- `EXPORT` при target-not-ready отклоняется до списания средств.
- Просроченный/битый intent не приводит к silent-stuck состоянию; оператор получает recovery hint.

---

### S15-S2 — Foreign-balance visibility semantics

**Цель слайса**
- Сделать видимость foreign-балансов недвусмысленной в API/CLI/TUI.

**Acceptance criteria**
- Для foreign-адресов значения помечаются как local-view-only либо скрываются по дефолту (выбран один режим и задокументирован).
- Поля локального и authoritative баланса не смешиваются в одном числовом выводе.
- История/адресная витрина не маскирует статус foreign-данных как spendable-local.

**Минимальные negative checks**
- UI/API не показывает foreign local-view как spendable без маркера.
- Проверка stale foreign-данных: метка устаревания или отказ в authoritative claim присутствует.

---

### S15-S3 — Genesis/hash guardrails для join shard

**Цель слайса**
- Блокировать подключение/работу шарда при genesis mismatch и сделать диагностику очевидной до пользовательских tx.

**Acceptance criteria**
- `/v1/status` (или эквивалентный status contract) содержит effective genesis/hash для каждого узла.
- Join path валидирует hash/bundle-consistency до приема tx.
- Есть операторский recovery path: mismatch detection -> stop -> fix bundle -> restart verify.

**Минимальные negative checks**
- Нода с несовпадающим genesis/hash не принимает пользовательские tx.
- Mismatch не деградирует в неявное частично-рабочее состояние (нет false healthy).

---

### S15-S4 — Snapshot backend abstraction (`JsonFile` + `Db` interface)

**Цель слайса**
- Вынести snapshot storage за интерфейс с сохранением replay-determinism для baseline JSON.

**Acceptance criteria**
- `JsonFile` остается рабочим baseline backend по умолчанию.
- `Db` backend подключается через явный backend selector, без влияния на default path.
- Контракт сериализации/десериализации фиксирован и одинаково валиден для replay.

**Минимальные negative checks**
- Некорректный backend selector не приводит к silent fallback; возвращается явная ошибка.
- Replay на JSON не ломается после введения абстракции.

---

### S15-S5 — ClickHouse prototype backend (optional)

**Цель слайса**
- Поднять минимальный Docker-прототип snapshot persistence в ClickHouse как кандидат DB backend.

**Acceptance criteria**
- Прототип умеет write/read snapshot state для smoke-сценария.
- Конфиг запуска и reset/teardown сценарий документирован для оператора.
- Есть short decision note по practical fit: throughput, determinism risk, ops complexity.

**Минимальные negative checks**
- Недоступность ClickHouse не ломает baseline JSON путь.
- Ошибки записи в DB не маскируются как успешный snapshot commit.

---

### S15-S6 — Replay/consistency e2e across backends

**Цель слайса**
- Подтвердить, что JSON и DB path дают согласованное поведение cross-shard состояния.

**Acceptance criteria**
- Один и тот же сценарий `EXPORT/IMPORT` на JSON и DB дает одинаковые итоговые инварианты.
- Replay после restart сохраняет idempotency и не допускает двойного импорта.
- Отчет тестирования фиксирует расхождения (если есть) и решение: fix now или carry-over.

**Минимальные negative checks**
- Duplicate import отклоняется на обоих backend path.
- Corrupted/partial snapshot не приводит к тихой потере replay guard.

---

### S15-S7 — Sprint closeout и decision gate

**Цель слайса**
- Зафиксировать итог спринта, остаточные риски и решение о переходе к следующему sprint stage.

**Acceptance criteria**
- Есть consolidated closeout note: что завершено, что carry-over, почему.
- Есть финальный verdict по трем gate: coding/testing/review.
- Есть короткий backlog на следующую итерацию (explorer-readiness, backend hardening, perf).

**Минимальные negative checks**
- Нет незакрытых block-level findings без явного owner и target sprint.
- Нет go-decision при провале хотя бы одного из обязательных gate.

**Консолидированный closeout и решение по gate:** [`docs/reviews/sprint-15-S7-closeout.md`](sprint-15-S7-closeout.md) (2026-05-04).

## 4) Предложение практичных ID/названий задач оркестратора

- `S15-S0-ARCH-FREEZE` — Architecture freeze и sprint contract.
- `S15-S1-XSHARD-HARDEN` — Cross-shard transfer/state consistency hardening.
- `S15-S2-FOREIGN-BAL-SEM` — Foreign-balance visibility semantics.
- `S15-S3-GENESIS-GUARD` — Genesis/hash guardrails for shard join.
- `S15-S4-SNAPSHOT-ABSTRACTION` — Snapshot backend abstraction (`JsonFile` + `Db`).
- `S15-S5-CLICKHOUSE-PROTOTYPE` — Optional ClickHouse snapshot backend prototype.
- `S15-S6-E2E-BACKEND-CONSISTENCY` — Replay/consistency e2e across backends.
- `S15-S7-CLOSEOUT` — E2E итог, риски, решение по спринту.

## 5) Риски спринта

- **R1 (High):** Source-side export без строгого target readiness preflight.
- **R2 (High):** Genesis/hash mismatch обнаруживается поздно (после tx).
- **R3 (Medium):** UI/API foreign-балансы интерпретируются как authoritative.
- **R4 (Medium):** DB backend вносит nondeterminism в snapshot/replay.
- **R5 (Low/Medium):** Операционная сложность ClickHouse превышает ценность MVP-спринта.

## 6) План rollback

- **RB1:** При проблемах S15-S1/S15-S2 оставить только проверенный baseline flow и отключить спорные UX-представления foreign-данных (feature-guard/strict mode).
- **RB2:** При проблемах S15-S3 запретить shard join при неполной валидации и вернуться к известному валидному genesis bundle.
- **RB3:** При проблемах S15-S4/S15-S5 переключение на `JsonFile` как единственный write backend; DB path в режим `disabled`.
- **RB4:** При провале S15-S6 фиксировать carry-over на следующий sprint, не делая release gate pass.

## 7) Якоря выравнивания (обязательная проверка)

- `docs/plans/mvp_v1_testnet_multi-sprint.md` (секция Sprint 15).
- `docs/plans/sprint-15-architecture-genesis-consistency-and-db-snapshots.md`.
- `docs/reviews/sprint-14-slice31-genesis-balance-consistency-review.md`.
