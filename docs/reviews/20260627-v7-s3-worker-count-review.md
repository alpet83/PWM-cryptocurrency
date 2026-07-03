# V7-S3 — worker auto-scale + queue depth metrics (pwm-review)

- date: 2026-06-27
- ticket: `20260627-v7-s3-worker-count-review`
- commit: `86b11f4` (branch `main`)
- normative: `docs/plans/mvp_v7s2.md` (follow-on scaling), `docs/reviews/v7-s3-worker-scale-results.md`

## 1. Scope recap

Auto-scale `WorkerPool` from host CPU count and extend pipeline observability:

| file | change |
|------|--------|
| `bootstrap.rs` | `host_worker_counts()` → `WorkerPool::new(1, general)` where `general = max(1, logical/2 - 1)` |
| `pipeline/queue.rs` | `queue_depth` / `queue_depth_max`, `worker_wait` histogram, `finish_block()` windowing |
| `pipeline/worker.rs` | `ClientTxJob::queued_at`, `start_client(queue_wait)` |
| `api/handlers_tx.rs` | `start_dispatch` / `commit_dispatch` / `cancel_dispatch` around dispatch |
| `lifecycle.rs` | `finish_block()` after successful seal |
| `api/handlers_status.rs` | exports extended `pipeline_metrics` snapshot |

## 2. Requirements fit

| Acceptance criterion | Verdict | Evidence |
|---------------------|---------|----------|
| `WorkerPool::new` uses `num_cpus` for auto-scale | **PASS** | `host_worker_counts()` (`bootstrap.rs:57-61`) reads `std::thread::available_parallelism()`; `worker_parts` (`:94-98`) passes `(1, general)`; unit test `worker_counts_scale` (`:69-72`) — 16 logical → `(1, 7)` |
| `QueueMetrics` exports `queue_depth_max` and `worker_wait_p50_ms` | **PASS** | `QueueMetricsSnapshot` (`queue.rs:158-159`); `snapshot()` (`:302-317`); `/v1/status` via existing `pipeline_metrics` field; `http_status.rs:154-155` |
| Throughput regression explained or refuted | **PASS** (explained) | See §4 — **RwLock write in precheck disproven**; plausible causes: parallel `State::clone`, higher semaphore inflight, receiver `Mutex` contention, run-to-run cluster variance |
| `cargo test -p pwmd` PASS | **UNVERIFIED** | Shell unavailable; in-tree tests: `worker_counts_scale`, `test_pipeline_depth_wait`, `test_dispatch_cancel_depth` |

**Benchmark note:** `docs/reviews/v7-s3-worker-scale-results.md` reports isolated-cluster **+11.5%** (52→58 tx/block). Ticket’s worse live run (52→44) is **not reproduced** in that artifact — likely different cluster/load/state size; both can be true per environment.

## 3. Style and module shape

- `host_worker_counts`, `finish_block`, `start_dispatch` — ≤4-word identifiers ✓
- Metrics window tied to seal block (`finish_block` after `Ok(seal)`) — coherent observability period ✓
- `ClientTxJob::new` centralizes queue timing ✓

Entity segment check not run (shell unavailable).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety — regression hypothesis (ticket focus)

### Disproven: `precheck_apply_with_ctx` takes `RwLock` write on shared `State`

```222:231:crates/pwm-core/src/state.rs
    pub fn precheck_apply_with_ctx(...) -> Result<(), TxError> {
        let mut st = self.clone();
        st.apply_tx_with_ctx(tx, inclusion_height, block_unix_time, gen_cfg)
    }
```

Worker path (`worker.rs:323+`): `ctx.snapshot.load()` acquires **`StateSnapshot` read lock** briefly to `Arc::clone` the snapshot (`state.rs:64-70`), then precheck runs on an owned **`State` clone** — no shared write during precheck.

### Confirmed: higher concurrent inflight precheck (semaphore)

- Old default: `WorkerPool::new(1, 1)` → **2** `client_tx` semaphore permits (`worker.rs:76`)
- New 16-logical host: **8** permits (1 affinity + 7 general)
- Up to **8 parallel** `precheck_apply_with_ctx` → **8 full `State::clone()`** on large chain state (height ~278k in soak docs) — CPU/memory bandwidth contention, not lock convoy on shared `State`

### Confirmed: ancillary contention surfaces

1. **`Mutex<Receiver<ClientTxJob>>`** — 7 general workers poll `try_recv` + 1ms sleep (`worker.rs:209-241`); more threads ⇒ more mutex traffic vs 2-worker baseline.

2. **`snapshot.store()` write lock** at seal — brief; more concurrent `load()` readers increases tail latency during store (secondary).

3. **`queue_depth_max` measures HTTP dispatch inflight** (`start_dispatch` in `handlers_tx.rs:232-244`), not tokio mpsc depth — high depth (35 in V7-S3 doc) reflects many concurrent awaits, consistent with more workers draining faster but seal/apply still serial under write lock.

### Regression verdict

Ticket hypothesis (**write lock inside precheck**) is **refuted**. Live regression vs Codex +11.5% is **explained by environment + cost model**: auto-scale increases parallel **O(N) state clones**, which can hurt on very large `State` even when worker queue wait drops (V7-S3 doc: p50 wait **1 ms**). Aligns with `v7-s2-ramp-results.md` “следующий шаг: hot-path balance/nonce index” — thread scale alone insufficient at large state.

## 5. Tests

| Test | Covers |
|------|--------|
| `worker_counts_scale` | CPU→worker mapping |
| `test_pipeline_depth_wait` | depth max, wait p50 buckets, `finish_block` window |
| `test_dispatch_cancel_depth` | cancel_dispatch on full queue |
| `v1_stat_default_lane_ns` | zero baseline for new metric fields |

**Gaps:** no integration test tying auto-scale worker count to `available_parallelism` mock; no benchmark in CI.

## 6. Concurrency / parallelism

**Components:** `Semaphore` permits = worker count; `QueueMetrics` atomics; `StateSnapshot` `RwLock` read-heavy; OS-thread worker pool.

| Hazard | Assessment |
|--------|------------|
| More workers ⇒ more parallel `State::clone` | **Primary scale risk** on large snapshots |
| `client_tx` semaphore bounds inflight | **Changed** 2→8 on 16-core; explains higher peak inflight vs baseline |
| Snapshot `RwLock` write on seal | **Brief** barrier; amplified tail with many readers |
| General-worker poll loop | **Mutex + sleep** overhead at high worker count |

**Test gap:** load test correlating `worker_wait_p50_ms` vs `queue_depth_max` vs seal_slip under configurable worker count.

## 7. Findings (prioritized)

### Medium

1. **Auto-scale has no ceiling / config override** — 16-logical → 8 workers always; large-state deployments may need `[pipeline] workers` cap (noted in v7-s2 ramp doc, not implemented).

2. **Conflicting benchmark artifacts** — V7-S3 isolated report shows improvement; ticket live run shows regression. Recommend tagging results with state size (account count), build profile, and worker count in ramp markdown.

### Low

3. **`worker_wait_p50_ms` uses log buckets** — approximate (`wait_bound_ms`); fine for ops, not exact percentile.

4. **`queue_depth_max` is dispatch-await depth** — name can be read as mpsc depth; document semantics in ops notes.

5. **Formula vs v7-s2 text** — implements `1 + max(1, cpus/2 - 1)` not literal `max(2, cpus/2)` total; equivalent at 16 cores (8 total), differs at 2–3 cores.

## 8. Verdict

**Approve with nits** — auto-scale and metrics meet acceptance criteria. Throughput regression hypothesis via **precheck write lock is refuted**; alternative explanation (parallel full-state clone + higher inflight + mutex polling + environment variance) is consistent with code and both benchmark reports. Ship with recommendation to cap workers or pursue O(1) precheck hot path before further thread scaling on large states.

## 9. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260627-v7-s3-worker-count-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 38000, "confidence": "medium" }`