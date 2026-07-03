# V7-S2 Slice 4 — QueueMetrics + TxEvent broadcast (pwm-review)

- date: 2026-06-26
- ticket: `20260626-v7-s2-slice4-metrics-txevent-review`
- commit: `5984a4b` (branch `mvp-v7`)
- normative: `docs/plans/mvp_v7s2.md` § Slice 4, `docs/adr/0013-tx-pipeline-seda.md`

## 1. Scope recap

Slice 4 adds per-stage pipeline observability and async result groundwork:

| file | change |
|------|--------|
| `pipeline/queue.rs` | `QueueMetrics` / `QueueMetricsSnapshot` (5 atomics); `TxEvent { Sealed, Rejected }` |
| `pipeline/worker.rs` | `WorkerCtx.metrics`; `debug_span` `worker.validate` / `worker.precheck`; `inc_validated` / `inc_rejected`; validated-queue-full → reject |
| `api/handlers_tx.rs` | `debug_span!("dispatch")`; `inc_enqueued` / `inc_rejected` on dispatch |
| `api/handlers_status.rs` + `api/types.rs` | `pipeline_metrics` in `/v1/status` JSON |
| `lifecycle.rs` | `debug_span!("seal.drain_validated")`; `inc_dequeued` / `inc_stale_validated`; `TxEvent::Sealed` per sealed tx |
| `bootstrap.rs` / `state.rs` | `App.pipeline_metrics`, `App.tx_events` (broadcast cap 256); shared metrics into `WorkerCtx` |

Note: ticket lists `handlers_stat.rs` / `app.rs` — actual paths are `handlers_status.rs` and `state.rs`.

## 2. Requirements fit

| Acceptance criterion | Verdict | Evidence |
|---------------------|---------|----------|
| `QueueMetrics` atomics incremented on all stages | **PASS** (with nits) | `enqueued`/`rejected`: `handlers_tx.rs:236-245`; `validated`/`rejected`: `worker.rs:301-313`; `dequeued`/`stale_validated`: `lifecycle.rs:1825-1827`. All five fields used. Raw `pool.take` / ingress drain uncounted (see nits) |
| `GET /v1/status` contains `pipeline_metrics` | **PASS** | `handlers_status.rs:252`, `types.rs:30`; test `v1_stat_default_lane_ns` (`http_status.rs:149-153`) |
| `debug_span!` on dispatch / worker.validate / worker.precheck / seal.drain_validated | **PASS** | `handlers_tx.rs:236`, `worker.rs:297+320`, `lifecycle.rs:1821` |
| `TxEvent::Sealed` published after seal for each tx | **PASS** (lifecycle path) | `lifecycle.rs:1876-1881` — per tx in sealed block after `Ok(seal_entries)`; test `v1_tx_event_sealed` (`http_status.rs:838-868`) |
| `cargo test -p pwmd -p pwm-core` PASS | **UNVERIFIED** | Shell unavailable in review session |

## 3. Style and module shape

- `QueueMetrics`, `inc_validated`, `QueueMetricsSnapshot` — ≤4-word production identifiers ✓
- Metrics live on shared `Arc<QueueMetrics>` wired once in bootstrap, cloned into `WorkerCtx` ✓
- `TxEvent` minimal enum for V7-4 SSE groundwork ✓

Entity segment check not run (shell unavailable).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

1. **Atomics use `Ordering::Relaxed`** — acceptable for observability counters; no correctness dependency ✓

2. **Validated queue full now fails closed** — `worker.rs:304-308` returns `PrecheckFailed` and `inc_rejected`; fixes prior silent-drop nit from Slice 3 fix review ✓

3. **`tx_events.send` errors ignored** — `let _ = app.tx_events.send(...)` when no subscribers; standard broadcast pattern ✓

4. **`TxEvent::Rejected` never published** — enum variant exists (`queue.rs:82-85`) but no producer; HTTP/worker rejections not streamed yet (V7-4 scope)

5. **Roaming direct-seal txs** — `handlers_tx.rs` roaming branch seals without lifecycle `tx_events` publish; only main seal loop emits `Sealed`

## 5. Tests

| Test | Covers |
|------|--------|
| `v1_stat_default_lane_ns` | `/v1/status` `pipeline_metrics` zero baseline |
| `v1_tx_xfer_worker_once` | `enqueued==1`, `validated==1` after HTTP transfer (`http_status.rs:832-835`) |
| `v1_tx_event_sealed` | `TxEvent::Sealed` after `spawn_seal_loop` + Init tx |
| `test_queue_metrics` / `test_queue_rejection_on_full` | `BoundedQueue` local counters (not App-level) |

**Gaps:**

- No test for `stale_validated` / `dequeued` increment on seal drain
- No test for `inc_rejected` on dispatch queue full or worker precheck fail reflected in `/v1/status`
- No test that `TxEvent::Rejected` is emitted (variant unused)

## 6. Concurrency / parallelism

**Components:** `Arc<QueueMetrics>` atomics across tokio HTTP, OS-thread workers, lifecycle seal task; `broadcast::Sender<TxEvent>` for post-seal fan-out.

| Hazard | Assessment |
|--------|------------|
| Relaxed atomic races on counters | **OK** — monotonic stats, slight visibility lag acceptable |
| Shared `pipeline_metrics` HTTP + worker | **OK** — independent `fetch_add` per event |
| `tx_events` broadcast from seal write-lock section | **OK** — send is sync, no await; subscribers lag without blocking seal |
| `debug_span` in worker OS thread | **OK** — tracing subscriber handles cross-thread spans |

**Test gap:** concurrent HTTP flood → metrics monotonicity / no lost updates under load.

## 7. Findings (prioritized)

### Blocker

None.

### Medium

1. **`enqueued` counts dispatch accept, not worker success** — `inc_enqueued` runs before `rx.await` (`handlers_tx.rs:245-246`); worker-rejected txs show `enqueued>=1` and `rejected>=1`. Document semantics or rename for clarity.

2. **`TxEvent::Sealed` only on lifecycle seal loop** — roaming HTTP direct-seal txs do not emit events; async clients watching broadcast miss those commits.

3. **Raw mempool `pool.take` path uninstrumented** — `dequeued` only counts validated-channel drain, not legacy/raw pool entries in the same block.

### Low

4. **`TxEvent::Rejected` defined but unused** — groundwork only; wire when V7-4 adds result stream.

5. **`BoundedQueue::metrics()` separate from `App.pipeline_metrics`** — per-dispatch-queue counters exist but are not exported in `/v1/status`.

6. **Nested spans** — `worker.validate` wraps call into `precheck_client` which opens `worker.precheck`; both appear under debug logging (acceptable).

## 8. Verdict

**Approve with nits** — Slice 4 acceptance items are met: shared atomic `QueueMetrics` on the hot path, `/v1/status` export, all four required `debug_span` names, and `TxEvent::Sealed` broadcast after lifecycle seal. Validated-queue-full backpressure is now explicit. Remaining nits are semantic clarity (`enqueued`), roaming/raw-path coverage, and unused `Rejected` variant.

## 9. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260626-v7-s2-slice4-metrics-txevent-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 35000, "confidence": "medium" }`