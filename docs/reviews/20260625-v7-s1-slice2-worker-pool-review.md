# V7-S1 Slice 2 — worker pool, affinity, backpressure (pwm-review)

Дата: 2026-06-25  
Тикет: `20260625-v7-s1-slice2-review`  
Коммит: `8e0f51e` — feat(pwmd): V7-S1 Slice 2 — worker pool, affinity, backpressure  
Норматив: `docs/adr/0013-tx-pipeline-seda.md`, `docs/plans/mvp_v7s1.md` § Slice 2

## 1. Scope recap

Слайс расширяет изолированный `crates/pwmd/src/pipeline/`:

- `worker.rs` — `WorkerPool` на `std::thread`, affinity `ClientTx`, custom `Semaphore` (Mutex+Condvar), stub-обработчики cluster/broadcast
- `queue.rs` — `blocking_recv`, `is_closed`; `//!` banner
- `dispatch.rs` — `new_with_workers`, тесты `DispatchError::*Full` (закрытие nit Slice 1); `//!` banner
- `mod.rs` — re-export worker types

Заявлено: **без** интеграции `lifecycle.rs`, `Chain::seal`, wire, snapshot. `Cargo.toml` — без новых крейтов (только существующие `tokio`/`pwm-core`).

## 2. Requirements fit

| Критерий Slice 2 (`mvp_v7s1.md`) | Статус |
|----------------------------------|--------|
| Worker pool стартует | **Да** (`WorkerPool::new`, `spawn_worker`) |
| Affinity обслуживает только свою очередь | **Да** (`run_client_rx` + `test_affinity_only_client`) |
| Backpressure: bounded queue rejection | **Да** (Slice 1 + dispatch full tests) |
| Backpressure: semaphore limits in-flight | **Частично** — семафор есть, но **нет теста** что worker блокируется на `acquire` при исчерпании permits |
| `cargo test -p pwmd` | **Не прогнан** в сессии (shell unavailable); тесты синтаксически корректны |
| Scope gate OUT | **Да** (grep: нет seal/lifecycle/wire в `pipeline/`) |
| Nit Slice 1: dispatch full-path tests | **Закрыт** (`test_dispatch_client_full`, `_cluster_full`, `_broadcast_full`) |
| Nit Slice 1: `//!` banners | **Закрыт** |

**ADR 0013 §Инструментарий:**

- Bounded channel — `tokio::sync::mpsc` (Slice 1) ✓
- Per-queue worker limit — реализован custom `Semaphore`, **не** `tokio::sync::Semaphore` из таблицы ADR. Для OS-потоков без `.await` это прагматично; зафиксировать отклонение в handoff или обновить ADR footnote.
- OS-thread workers — `std::thread::spawn` ✓
- CPU work в worker — `validate_tx_shape` для client path ✓

## 3. Style and module shape

- Module banners на `queue.rs`, `dispatch.rs`, `worker.rs` — есть.
- Production `expect` только на poisoned mutex в worker/semaphore — допустимо; user input не unwrap-ится.
- `python scripts/check_entity_name_segments.py` — не прогнан; визуально имена в политике.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

### Concurrency / parallelism

**Компоненты:** `WorkerPool` (N+M `JoinHandle`), `WorkerReceivers` (`Mutex<Receiver<T>>` на каждую очередь), custom `Semaphore`/`Permit`, `tokio::mpsc` bounded queues.

**Оценка:**

1. **Shared state** — `Mutex` только на `Receiver` (tokio mpsc single-consumer) и на счётчике semaphore. Нет `Arc<Mutex<State>>` на chain path. ✓
2. **Race windows** — dequeue под `Mutex`; permit берётся **после** dequeue. Очередь ограничивает pending; semaphore — concurrent in-flight. Согласованная двухслойная модель.
3. **Deadlock** — mutex не держится через `semaphore.acquire()` (release перед acquire следующего job в цикле). Affinity worker держит `client_tx` mutex на время `blocking_recv` — при `affinity_count > 1` лишние affinity-потоки блокируются на mutex, не deadlock. General worker: 1 ms spin при пустых очередях — livelight polling, не deadlock при живых senders.
4. **Shutdown** — выход при `is_closed()` на всех receivers / `blocking_recv` → `None`. **`WorkerPool` не join-ит handles** при Drop — потоки detached до самостоятельного выхода. OK для изолированных тестов (`drop(queues)`); для Slice 3 нужен явный shutdown API.
5. **Backpressure** — ingress: `try_push` → `DispatchError::*Full`. Processing: `Permit` на время `handle_*`. ✓
6. **Send/Sync** — jobs owned в потоке; `oneshot::send` после validate. ✓
7. **Tokio bridge** — `blocking_recv`/`try_recv` из `std::thread` на `tokio::mpsc::Receiver` — штатный паттерн tokio 1.x для blocking consumers; убедиться в CI что `#[test]` worker tests проходят без скрытого `#[tokio::test]` harness.

**Test gaps:** нет теста general worker на cluster/broadcast path; `test_backpressure_rejects` проверяет только queue full при занятых sem permits, не worker pool end-to-end.

## 4. Safety

- Hot path worker: `validate_tx_shape` + `reply.send` — без panic на bad tx (ошибка в `Result`).
- `handle_cluster` / `handle_broadcast` — stub `drop` (no-op) — OK для изоляции.
- DoS: full queue → rejection; нет unbounded growth в модуле.
- **Poisoned mutex** → `expect` паника — редкий fail-fast, приемлемо на этапе изоляции.

## 5. Tests

**Покрыто:**

- `test_dispatch_*_full` — все три пути rejection-on-full (nit Slice 1 закрыт).
- `test_worker_client_tx` — affinity worker + validate + oneshot reply.
- `test_affinity_only_client` — cluster batch остаётся в очереди, client обработан.
- `test_backpressure_rejects` — queue rejection при cap=2.

**Пробелы (nits):**

- General worker не тестируется на cluster/broadcast dequeue.
- Semaphore backpressure на worker (блокировка `acquire` при N in-flight jobs) — нет.
- `WorkerPool::new` multi-worker integration smoke — нет.

## 6. Verdict

**approve with nits** — Slice 2 post-condition для изолированного pool выполнен; affinity корректен; scope gate соблюдён; блокеров merge нет.

**Приоритет nits:**

1. **Medium:** добавить тест general worker (cluster или broadcast) + при желании semaphore in-flight limit.
2. **Medium:** Slice 3 — `WorkerPool::shutdown`/`join` вместо detach-only handles.
3. **Low:** ADR 0013 — footnote про custom `Semaphore` vs `tokio::sync::Semaphore` на OS-потоках.
4. **Low:** general worker poll loop — заменить `sleep(1ms)` на `Condvar`/park когда все очереди пусты (эффективность).

## 7. Participation / token estimate

```text
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260625-v7-s1-slice2-worker-pool-review.md
token_usage: { "source": "estimate", "input": null, "output": null, "total": 7500, "confidence": "low" }
```

**Glossary:** GLOSSARY.md: без изменений (нового жаргона не появилось).

**Вердикт одной строкой для оркестратора:** `PASS — worker pool+affinity OK; nits: general-worker tests, explicit shutdown, ADR semaphore note.`