# V7-S1 Slice 1 — pipeline queue model и dispatch (pwm-review)

Дата: 2026-06-25  
Тикет: `20260625-v7-s1-slice1-review`  
Коммит: `6b295c2` — feat(pwmd): V7-S1 Slice 1 — pipeline queue model и dispatch  
Норматив: `docs/adr/0013-tx-pipeline-seda.md`, `docs/plans/mvp_v7s1.md` § Slice 1

## 1. Scope recap

Слайс заявляет **изолированный** первый шаг SEDA: типы bounded-очередей, метрики `enqueued/dequeued/rejected`, три ingress-пути (`client_tx`, `cluster_ready`, `data_broadcast`) через `dispatch()`. Без интеграции в `Chain::seal`, wire, snapshot, `lifecycle.rs`.

Затронуто:

- `crates/pwmd/src/lib.rs` — `pub mod pipeline`
- `crates/pwmd/src/pipeline/mod.rs`
- `crates/pwmd/src/pipeline/queue.rs` (159 LOC)
- `crates/pwmd/src/pipeline/dispatch.rs` (157 LOC)

`Cargo.toml` pwmd — **без новых зависимостей** (только существующий `tokio`/`pwm-core`).

## 2. Requirements fit

| Критерий Slice 1 (`mvp_v7s1.md`) | Статус |
|----------------------------------|--------|
| Типы очередей + dispatcher в `crates/pwmd/src/` | **Да** |
| Unit-тесты трёх путей dispatch | **Да** (`test_dispatch_*`) |
| Unit-тест rejection-on-full + метрики | **Частично** — в `queue.rs` есть; в `dispatch.rs` нет теста `DispatchError::*Full` |
| Нет wire/snapshot изменений | **Да** (модуль изолирован) |
| Нет интеграции seal | **Да** |
| ADR 0013: `tokio::sync::mpsc`, `AtomicU64` метрики | **Да** |

**Соответствие channel contracts ADR 0013 §2:**

- `ClientTxJob` — `tx: SignedTx` **во владении** + `oneshot::Sender<Result<(), String>>` для ответа. Семантика job ownership соблюдена; имя поля `reply` vs ADR `reply_tx` — косметика.
- `ClusterReadyBatch { txs: Vec<SignedTx> }` — ранняя стадия; ADR псевдокод упоминает `PreparedBatch` на более поздней стадии. Для Slice 1 приемлемо; переименование/обогащение ожидается в Slice 2–3.
- `DataBroadcastJob` — соответствует третьему пути «data-broadcast» из плана V7-S1 (ADR псевдокод `HistoryRequest` — другое имя, не блокер).

**Пробел по плану V7 (не блокер Slice 1):** в `mvp_v7.md` client job несёт привязку к сокету; в типе пока только `oneshot` — ожидаемо до ingress-интеграции (Slice 3).

## 3. Style and module shape

- `pipeline/mod.rs` — есть `//!` banner.
- `queue.rs`, `dispatch.rs` — без module `//!` (низкий приоритет).
- Имена production API короткие (`try_push`, `try_recv`, `dispatch`) — в политике ≤4 сегментов.
- Дублирование test helper `test_tx()` в двух `#[cfg(test)]` модулях — допустимо, можно вынести в Slice 2.
- `python scripts/check_entity_name_segments.py crates/pwmd/src/pipeline/` — **не прогнан** (shell недоступен в сессии); визуально нарушений нет.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

### Concurrency / parallelism

Компоненты: `BoundedQueue<T>` на `tokio::sync::mpsc`, счётчики `AtomicU64` (`Ordering::Relaxed`), `Clone` на sender-стороне для multi-producer.

Оценка:

1. **Shared mutable state** — состояние очереди внутри `mpsc`; снаружи только атомарные счётчики. Mutex/RwLock нет.
2. **Race windows** — `try_send`/`try_recv` thread-safe у tokio `mpsc`; при concurrent `try_push` гонка за последний слот разрешается каналом (один `Ok`, остальные `Full`). Счётчики eventually consistent — для debug-метрик достаточно.
3. **Deadlock** — синхронных блокировок нет; bounded rejection без блокирующего push.
4. **Cancellation** — не в scope (нет `.await` на hot dispatch path).
5. **Backpressure** — `try_push` → `Err(item)` + `rejected++`; `dispatch` мапит в `DispatchError::*Full`. Соответствует ADR.
6. **Send/Sync** — `SignedTx` owned в job; `oneshot::Sender` передаётся в job (не хранится в shared queue state после pop).
7. **Метрика depth** — ADR упоминает queue depth; реализованы cumulative counters, не текущая глубина. Для Slice 1 OK; для `/v1/metrics` в Slice 2 может понадобиться `len` или `enqueued - dequeued`.

Замечание: `TrySendError::Closed` учитывается как `rejected` — корректно для shutdown path.

## 4. Safety

- **Hot path** (`try_push`, `dispatch`, `try_recv`): нет `unwrap`/`expect`/`panic!` в production code — только в тестах.
- **DoS footgun:** переполнение очереди не паникует — возврат ошибки + метрика `rejected`.
- **Trust boundary:** типы пока не на wire; ingress не подключён.

## 5. Tests

**Покрыто:**

- `test_queue_rejection_on_full` — full + `rejected` counter.
- `test_queue_metrics` — enqueue/dequeue counters.
- `test_dispatch_client_tx`, `test_dispatch_cluster_ready`, `test_dispatch_data_broadcast` — happy path + метрики per-queue.

**Не покрыто (nits):**

- `dispatch()` при полной очереди → `DispatchError::ClientTxFull` / `ClusterReadyFull` / `DataBroadcastFull`.
- `try_push` после drop всех receivers (`Closed` → `rejected`).
- Concurrent multi-producer stress (опционально до Slice 2).

`cargo test -p pwmd pipeline` — **не прогнан** в этой сессии (shell failure); по структуре тесты компилируемы и соответствуют заявленному scope.

## 6. Verdict

**approve with nits** — Slice 1 post-condition выполнен для изолированного модуля; scope gate OUT соблюдён; блокеров для merge нет.

**Приоритет nits:**

1. **Medium:** добавить тесты `dispatch` rejection-on-full для каждого из трёх путей.
2. **Low:** `//!` banners на `queue.rs` / `dispatch.rs`.
3. **Low:** зафиксировать в handoff Slice 2 переход `ClusterReadyBatch` → prepared batch и поле socket/ingress handle в `ClientTxJob`.
4. **Low:** при экспорте метрик — уточнить, нужна ли instantaneous depth, не только cumulative.

## 7. Participation / token estimate

```text
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260625-v7-s1-slice1-pipeline-review.md
token_usage: { "source": "estimate", "input": null, "output": null, "total": 6500, "confidence": "low" }
```

**Glossary:** GLOSSARY.md: без изменений (нового жаргона не появилось).

**Вердикт одной строкой для оркестратора:** `PASS — изолированный SEDA queue+dispatch OK; nits: dispatch full-path tests, PreparedBatch/socket — Slice 2+.`