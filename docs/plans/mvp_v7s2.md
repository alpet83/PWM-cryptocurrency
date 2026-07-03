---
name: V7-S2 Sprint Plan — SEDA Full Integration
status: deferred
deferred_at: 2026-06-29
deferral_note: >
  После V7-S1 (flamegraph + perf quick-wins) bottleneck сместился на P2P wire JSON,
  а не CPU pipeline. SEDA-архитектура не даст прироста пока wire не заменён бинарным
  кодеком. P2P wire codec запланирован в Фазе 4 совместно с переходом на CometBFT.
  SEDA может быть актуальна после wire codec — пересмотреть при планировании Фазы 4.
related:
  - docs/adr/0013-tx-pipeline-seda.md
  - docs/plans/mvp_v7.md
  - docs/plans/perf-optimization-spectrum.md
---

# V7-S2: SEDA — полная интеграция (workers делают реальную работу)

## Контекст

V7-S1 заложил scaffold: очереди, диспетчер, WorkerPool, tx ingress канал.
Но CPU-work из `Chain::seal` не вынесен: workers делают только `validate_tx_shape`,
а `evaluate_policy` + `precheck_apply_with_ctx` остаются внутри seal loop.

Сeal всё ещё:
1. берёт write lock для полной обработки каждого tx
2. сам вызывает `evaluate_policy` + `apply_tx_with_ctx` per-tx последовательно
3. тратит seal slots на eviction плохих tx (в логах: `seal skip: evicting unapplicable tx`)

Цель V7-S2: workers выполняют полный precheck на state snapshot → seal получает
already-validated batch → eviction исчезает → seal становится тонким коммит-слоем.

**Ожидаемый результат:** ≥50 tx/s с кластером (proposer + attester) при sustained ≥60s.

---

## Архитектура после V7-S2

```
HTTP ingress
    │  validate_tx_shape (no lock)
    │  try_send → TxIngressChannel
    ▼
WorkerPool (OS threads, affinity=ClientTx)
    │  arc_clone(state_snapshot)           ← дёшево, без lock
    │  evaluate_policy(tx, snapshot)
    │  precheck_apply_with_ctx(tx, snapshot)
    │  → ValidatedTx { tx, snapshot_height }   (на output channel)
    │  → rejected: reply 422 через oneshot
    ▼
ValidatedQueue (bounded mpsc)
    │
    ▼
Seal loop (write lock)
    │  drain ValidatedQueue
    │  если snapshot_height == current_tip → fast-path (policy skip)
    │  если stale → нормальный apply (страховка корректности)
    │  Chain::seal(batch)
    │  обновить state_snapshot атомарно
    ▼
    block committed
```

---

## Критерий приёмки спринта

`python scripts/cy_cluster_transfer_ramp_soak.py` на кластере (proposer + attester)
даёт **sustained ≥50 tx/s за ≥60 секунд** при:
- `seal_skip: evicting unapplicable tx` = 0 в логах за период теста
- 0 seal-детерминизм-ошибок
- `cargo test -p pwmd -p pwm-core` PASS

---

## Slice 1 — State snapshot для workers

**Pre-condition:** V7-S1 Slice 3 (TxIngressChannel) стабилен.

**Post-condition:**
- В `App` добавлено поле `state_snapshot: Arc<ArcState>` где
  `type ArcState = ArcSwap<Arc<State>>` (или `RwLock<Arc<State>>` с cheap clone).
- После каждого `Chain::seal` в lifecycle.rs snapshot атомарно обновляется.
- Workers получают `Arc<State>` через `snapshot.load()` без write lock.
- Unit-тест: обновление snapshot после seal виднo в worker-стороне.

**Ключевые файлы:**
- `crates/pwmd/src/state.rs` — добавить `state_snapshot`
- `crates/pwmd/src/lifecycle.rs` — обновлять snapshot после seal
- `crates/pwmd/src/pipeline/worker.rs` — передавать snapshot в worker_loop

**Нюанс:** `Arc<State>` clone — O(1), копия указателя. `State` clone — O(N аккаунтов), дорого.
Snapshot — это `Arc<State>`, не `State`. Worker делает `arc_clone` (не `state.clone()`).

**Добавить `TxEntry` + `TxOrigin` (future-proof, CONCEPT_ROADMAP R13):**
```rust
pub enum TxOrigin {
    DirectHttp,                                   // клиент напрямую
    HelperNode { helper_id: u16, batch_id: u64 }, // через sentry (уровень 1 кластера)
}
pub enum TxEntryState {
    Pending,
    Validated { at_height: u64 },
    Sealed    { block_height: u64 },
    Rejected  { reason: TxRejectReason },
}
pub struct TxEntry {
    pub tx:             SignedTx,
    pub ingress_height: u64,
    pub state:          TxEntryState,
    pub origin:         TxOrigin,   // маршрутизация результата при seal
}
```
В V7-S2 используется только `TxOrigin::DirectHttp`. Поле `origin` позволит V7-4 добавить
sentry-node протокол без рефакторинга структур.

---

## Slice 2 — Workers выполняют полный precheck

**Pre-condition:** Slice 1 PASS.

**Post-condition:**
- `worker_loop` выполняет:
  1. `validate_tx_shape(&tx)` (уже есть)
  2. `snapshot.evaluate_policy(&tx, snapshot_height)` → PolicyDecision
  3. `snapshot.precheck_apply_with_ctx(&tx, next_h, next_ts, &cfg)` → dry-run
- Результат: `ValidatedTx { tx, snapshot_height }` или reject через `reply` oneshot.
- `handlers_tx.rs`: убрать `precheck_apply_with_ctx` под read lock (workers делают это).
- Unit-тест worker с валидным и невалидным tx: reply корректен.

**Нюанс stale snapshot:**
Если snapshot отстаёт на N блоков — precheck даёт ложный Accept (tx уже применён
в более новом блоке). Это допустимо: `Chain::seal` отклонит дубль при apply.
Stale snapshot не может дать ложный Reject для новых tx (только Accept).

---

## Slice 3 — ValidatedQueue + seal fast-path

**Pre-condition:** Slice 2 PASS.

**Post-condition:**
- Новый тип `ValidatedTx { tx: SignedTx, validated_at_height: u64 }` в `pipeline/queue.rs`.
- `WorkerPool` output: `mpsc::Sender<ValidatedTx>` (bounded, cap 256).
- `lifecycle.rs` seal loop:
  - Дренирует `ValidatedQueue` → `g.pool` перед `pool.take(64)`.
  - Если `validated_at_height == g.chain.tip_h()` → `Chain::seal` использует fast-path
    (передаёт hint `pre_validated=true`, seal пропускает `evaluate_policy`).
  - Если stale → нормальный path (страховка).
- `cargo test -p pwmd` PASS.

**Важно:** fast-path в `Chain::seal` — только hint, не bypass. Seal всё равно применяет
`apply_tx_with_ctx` (мутирует State). Только `evaluate_policy` пропускается при свежем snapshot.

---

## Slice 4 — Метрики per-stage и observability

**Pre-condition:** Slice 3 PASS.

**Post-condition:**
- `QueueMetrics` для каждой стадии: `enqueued / dequeued / rejected / validated / stale_validated`.
- Метрики доступны через `/v1/status` или `/v1/metrics` (JSON, additive).
- `tracing::debug_span!` на каждой стадии: `dispatch`, `worker.validate`, `worker.precheck`, `seal.drain_validated`.
- При `RUST_LOG=debug` — полная картина задержек per-stage.
- **Async result stream (groundwork):** после seal оркестратор публикует `TxEvent::Sealed { txid, block_height }`
  в `tokio::sync::broadcast::Sender<TxEvent>`. Сам SSE/WS хендлер — scope V7-4, но channel
  создаётся здесь чтобы не переделывать orchestrator позже.

---

## Slice 5 — Benchmark, determinism, DoS

**Pre-condition:** Slice 4 PASS.

**Post-condition:**
1. **Property-тест детерминизма:**
   ```rust
   fn determinism_1_vs_n_workers() {
       let txs = generate_test_batch(50);
       assert_eq!(
           run_pipeline(&txs, workers=1).state_hash,
           run_pipeline(&txs, workers=8).state_hash
       );
   }
   ```
   PASS.

2. **DoS-тест:** 512 параллельных запросов → сервер отвечает 507 (не crash, не deadlock).
   `GET /v1/status` = `ready` после флуда.

3. **Ramp soak:** `python scripts/cy_cluster_transfer_ramp_soak.py` →
   sustained ≥50 tx/s за ≥60 блоков, 0 evictions, 0 seal-детерминизм-ошибок.

4. Результаты коммитятся в `docs/reviews/v7-s2-ramp-results.md`.

---

## Известные риски

| Риск | Вероятность | Контрмера |
|------|-------------|-----------|
| Stale snapshot → ложный Accept → seal eviction | Средняя | fast-path только при свежем snapshot; eviction rate в метриках |
| State clone дорогой при большом числе аккаунтов | Низкая (Arc<State> не клонируем) | Убедиться что workers берут Arc, не State |
| prop_seal_commit spike при ≥50 tx (I/O snapshot) | Средняя | Отключить autosnapshot в benchmark; профилировать отдельно |
| Nonce overlap при высоких уровнях ramp | Решено | head-wait fix (commit 15138ee) |
| Worker starvation при burst (все permits заняты) | Низкая | Bounded semaphore + backpressure уже есть |

---

## Rollback

Если ≥50 tx/s не достигнуто после Slice 5:
- Если bottleneck — snapshot I/O: переход к Tier 2 (CH async write вместо fsync).
- Если bottleneck — attest latency > seal_interval: конфиг `attest_timeout_ms`, peer tuning.
- Если bottleneck — `apply_tx_with_ctx` CPU: profiling → ADR для immutable State (Tier 1 потолок).
- Pipeline scaffold (V7-S1 + V7-S2) остаётся — он полезен независимо от throughput.
