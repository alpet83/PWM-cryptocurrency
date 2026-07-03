# ADR 0013: Tx Pipeline — SEDA-архитектура и методология разработки

## Статус

Принято (V7-S1 normative contract).

## Контекст

В V6 вся обработка транзакций происходит последовательно внутри `Chain::seal` → `apply_tx_with_ctx` → `drain_conservation`. Результат — потолок ~3 tx/s. V7-S1 вводит параллельную pre-processing. Нужно дать архитектуре точное имя, выбрать инструменты без велосипедов и зафиксировать методологию разработки и отладки.

## Решение

### Паттерн: SEDA (Staged Event-Driven Architecture)

SEDA — архитектура, в которой обработка запроса разбита на **стадии** (stages), соединённые **bounded-очередями**. Каждая стадия имеет свой пул потоков. Backpressure реализуется насыщением очереди: когда очередь полна, отправитель получает rejection или блокируется — вместо того, чтобы обрушить систему.

Применительно к PWM pwmd:

```
Ingress (tokio)
    │
    ▼
Dispatcher (orchestrator-owned, OS thread)
    ├──► Queue[Client]  (bounded) ──► WorkerPool[CPU] ──► prepared_batch
    ├──► Queue[Mempool] (bounded) ──► WorkerPool[CPU] ──► prepared_batch
    └──► Queue[DataBroadcast] ──────► WorkerPool[IO]
                                            │
                           prepared_batch ◄─┘
                                │
                           Orchestrator (главный поток)
                                │
                           Chain::seal (атомарный коммит)
```

**Ключевые принципы SEDA в этом контексте:**
- Каждая стадия изолирована: падение одной стадии не роняет другие.
- Backpressure распространяется вверх по очередям, не через механизмы panic/unwrap.
- `Chain::seal` — единственная точка мутации состояния; всё до неё — подготовка и может быть параллельным.
- Оркестратор не делает тяжёлую работу; он только перемещает данные между стадиями и коммитит.

### Инструментарий (без новых крейтов сверх уже имеющихся)

| Задача | Инструмент | Обоснование |
|--------|-----------|-------------|
| Bounded channel с backpressure | `tokio::sync::mpsc::channel(N)` | Уже в `tokio = {features = ["full"]}`. `blocking_send()` из OS-потоков. |
| Per-queue лимит воркеров | `tokio::sync::Semaphore` | Там же. `acquire()` блокирует воркера при насыщении. |
| Готовность / wake orchestrator | `tokio::sync::Notify` | Уже используется (`seal_wake`). |
| CPU-bound worker threads | `std::thread::spawn` | Не tokio-задача: CPU-bound работа в tokio-потоке блокирует runtime. |
| Атомарные метрики (queue depth) | `std::sync::atomic::AtomicU64` | Уже используется в codebase. Zero overhead. |
| Результаты воркер → оркестратор | `std::sync::mpsc::sync_channel(N)` | Rendezvous/bounded без tokio зависимости на worker-side. |

**Возможное дополнение (если нужен MPMC без tokio):** `crossbeam-channel` — маленький (800 строк ядра), MIT, de-facto стандарт в Rust для high-perf каналов. Добавлять только если `std::sync::mpsc` окажется недостаточным.

**Не добавлять в V7-S1:** `rayon` (для брутфорса уже рассматривался, для pipeline не нужен — там не fork-join), `parking_lot` (overhead не оправдан при текущем профиле), любые actor-framework крейты.

### Разделение OS-потоков и tokio

```
tokio runtime (async)          │  OS threads (sync)
────────────────────────────────┼────────────────────────────────
HTTP/RPC ingress                │  validate_tx_shape()
Dispatcher (channel send)       │  evaluate_policy()
Orchestrator seal_wake.notify() │  apply_tx() — dry run
Snapshot I/O (если async)      │  prepared_batch → channel.send()
```

Правило: **всё, что трогает состояние блокчейна (State, Chain), выполняется в OS-потоках через каналы.** Tokio-задачи занимаются только сетевым вводом-выводом и координацией.

---

## Методология разработки

### 1. Pure functions first

`validate_tx_shape`, `evaluate_policy`, `apply_tx` уже являются чистыми функциями в `pwm-core` (принимают `&State`, не держат shared state). Убедиться в этом в Slice 0. Если есть скрытые зависимости — изолировать до начала Slice 1.

**Паттерн worker-задачи:**
```rust
// Всё необходимое передаётся во владение — никаких Arc<Mutex<State>> на горячем пути
struct ClientJob { tx: SignedTx, reply_tx: oneshot::Sender<Result<()>> }

fn worker_loop(jobs: Receiver<ClientJob>, state_snapshot: Arc<State>) {
    for job in jobs {
        let result = validate_tx_shape(&job.tx)
            .and_then(|_| evaluate_policy(&job.tx, &state_snapshot));
        let _ = job.reply_tx.send(result);
    }
}
```

### 2. Channel contracts до wire-up

Определить типы сообщений для каждой очереди как enum/struct, дать им названия, написать unit-тест на сериализацию/десериализацию (если нужно). Только после этого — соединять стадии.

```rust
enum DispatchMsg {
    ClientTx(ClientJob),
    ClusterReady(PreparedBatch),
    HistoryRequest(BackfillJob),
}
```

### 3. Worker в изоляции — раньше интеграции

Каждый worker-тип тестируется отдельно: отправить N сообщений через канал, проверить N корректных ответов. Никакого оркестратора в этом тесте.

```rust
#[test]
fn test_validate_worker_rejects_bad_sig() {
    let (tx, rx) = std::sync::mpsc::sync_channel(10);
    let worker = std::thread::spawn(move || worker_loop(rx, mock_state()));
    tx.send(ClientJob { tx: bad_tx(), ... }).unwrap();
    // проверить reply
}
```

### 4. Метрики с первого дня (не после)

Добавить `AtomicU64` счётчики при создании каждой очереди: `enqueued`, `dequeued`, `rejected` (queue full). Выставить через `/v1/metrics` или добавить в существующий `GET /v1/status`. Это инструмент отладки производительности, не опция.

### 5. Интеграция последней, harness — с Slice 0

Полная интеграция (Slice 3) только после того, как каждый worker протестирован изолированно. Но `cy_cluster_transfer_ramp_soak.py` запускается уже в Slice 0 для baseline — это не финальный прогон, а точка отсчёта.

---

## Методология отладки

### Профилирование (Slice 0, обязательно)

```bash
# В Docker-среде:
cargo build --release -p pwmd
perf record --call-graph dwarf ./target/release/pwmd
perf report

# Альтернатива без perf:
cargo install flamegraph
cargo flamegraph --bin pwmd -- [args]
```

Существующий `block_timing.rs` — использовать как первую точку измерения задержек на горячем пути seal.

### Tracing spans на каждую стадию

```rust
// Добавить при реализации каждой стадии:
let _span = tracing::debug_span!("dispatch", queue = "client").entered();
let _span = tracing::debug_span!("worker.validate", tx_id = %tx.id()).entered();
let _span = tracing::debug_span!("orchestrator.seal", batch_size = batch.len()).entered();
```

При `RUST_LOG=debug` появляется полная картина задержек per-stage.

### Детектирование дедлоков

**ThreadSanitizer (самый быстрый способ):**
```bash
RUSTFLAGS="-Z sanitizer=thread" cargo +nightly test -p pwmd 2>&1 | grep -A5 "WARNING: ThreadSanitizer"
```

**`loom` (для тонких гонок в custom concurrent структурах):**  
Добавить как `dev-dependency = { version = "0.7", features = ["checkpoint"] }`. Использовать только для unit-тестов новых concurrent примитивов (queue, semaphore wrapper). Не применять ко всей кодовой базе.

**Простой bounded-timeout тест на дедлок:**
```rust
// В интеграционном тесте:
let result = std::thread::spawn(|| pipeline.process(batch))
    .join_timeout(Duration::from_secs(5));
assert!(result.is_ok(), "pipeline deadlocked");
```

### Property-тест детерминизма (обязательный gate Slice 4)

```rust
// Один и тот же batс должен давать идентичный результат при 1 и N воркерах
#[test]
fn determinism_1_vs_n_workers() {
    let txs = generate_test_batch(100);
    let result_1 = run_pipeline_with_workers(&txs, 1);
    let result_n = run_pipeline_with_workers(&txs, 8);
    assert_eq!(result_1.final_state_hash, result_n.final_state_hash);
}
```

Если нужны разнообразные входные наборы — `proptest` как `dev-dependency` (маленький, MIT).

---

## Последствия

- Код pipeline изолирован в `crates/pwmd/src/pipeline/` (новый модуль); `Chain::seal` не меняет сигнатуры.
- Любое изменение типов сообщений между стадиями — breaking change внутри sprint, требует обновления тестов всех воркеров.
- При выборе BFT в ADR V7-6: кандидат обязан поддерживать SEDA-совместимый proposal path (не блокировать диспетчер).

## Ссылки

- Matt Welsh, "SEDA: An Architecture for Well-Conditioned, Scalable Internet Services" (SOSP 2001)
- `tokio::sync` docs — `mpsc`, `Semaphore`, `Notify`
- `docs/plans/mvp_v7s1.md` — Sprint 1 plan с pre/post-условиями слайсов
- `scripts/cy_cluster_transfer_ramp_soak.py` — основной harness измерения
