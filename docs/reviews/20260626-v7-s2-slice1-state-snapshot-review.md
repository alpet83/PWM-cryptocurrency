# V7-S2 Slice 1 — StateSnapshot + TxEntry/TxOrigin (pwm-review)

- date: 2026-06-26
- ticket: `20260626-v7-s2-slice1-state-snapshot-review`
- commit: `a877822` (branch `mvp-v7`)
- normative: `docs/plans/mvp_v7s2.md` § Slice 1, `docs/adr/0013-tx-pipeline-seda.md`

## 1. Scope recap

Slice 1 adds worker-readable chain state without the seal write lock:

| file | change |
|------|--------|
| `crates/pwmd/src/state.rs` | `StateSnapshot` (`std::sync::RwLock<Arc<State>>`), `App.state_snapshot`, unit test |
| `crates/pwmd/src/bootstrap.rs` | init `state_snapshot` in all `App` construction paths |
| `crates/pwmd/src/lifecycle.rs` | `state_snapshot.store()` after successful `Chain::seal` in main seal loop |
| `crates/pwmd/src/pipeline/queue.rs` | `TxOrigin`, `TxEntryState`, `TxEntry` |
| `crates/pwmd/src/pipeline/worker.rs` | `WorkerPool::new` / `spawn_worker` take `Arc<StateSnapshot>`; `load()` in worker loops |
| `crates/pwmd/src/pipeline/mod.rs` | re-exports |

Out of scope (per V7-S1/S2 plan): full precheck in workers, production `WorkerPool` lifecycle wire-up, `ValidatedQueue`.

## 2. Requirements fit

| Acceptance criterion | Verdict | Evidence |
|---------------------|---------|----------|
| `StateSnapshot` uses `std::sync::RwLock` | **PASS** | `state.rs:21,53` — `StdRwLock`, not `tokio::sync` |
| Workers use `load()` → `Arc<State>`, not `State` clone | **PASS** (worker side) | `worker.rs:171,183` — `snapshot.load()`; **NIT** seal path clones `State` on store (below) |
| `store()` after successful `Chain::seal` in lifecycle | **PASS** | `lifecycle.rs:1826-1831` — inside `Ok(())` only, after `after_chain_seal` checkpoint |
| `bootstrap.rs` init in all App constructors | **PASS** | `bootstrap.rs:83` (`app_from_chain_boot`), `:216` (snapshot load branch), `:294` (genesis fallback) |
| `TxEntry` / `TxOrigin` / `TxEntryState` cover plan scenarios | **PARTIAL** | Types match plan shape (`queue.rs:26-45`); `Rejected { reason: String }` not `TxRejectReason`; types **unused** outside re-export |
| Unit test `snapshot_loads_stored` | **PASS** | `state.rs:385-395` |
| No lock held longer than needed | **PASS** | `load()`/`store()` scope locks to clone/swap `Arc`; worker holds `Arc` not `RwLock` |
| `cargo test -p pwmd` PASS | **UNVERIFIED** | Shell unavailable in review session; no commit log artifact in repo |

## 3. Style and module shape

- New production identifiers (`StateSnapshot`, `TxEntry`, `TxOrigin`, `TxEntryState`) are ≤4 snake segments ✓
- `StateSnapshot` API is minimal (`new`, `load`, `store`) with `//!` context on `state.rs` module banner already present
- `queue.rs` types follow existing pipeline naming; no new large blobs in `main.rs` / façade

Entity segment check not run (shell unavailable).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

- **Poisoned lock:** `load`/`store` use `.expect("... poisoned")` — consistent with nearby `Mutex` usage; acceptable for daemon fatal path
- **Stale snapshot reads:** workers may read pre-seal `Arc<State>` while seal holds `inner` write lock — intentional for slice 1; slice 2 must handle `snapshot_height` staleness
- **Alternate seal paths** (`handlers_tx.rs`, `handlers_lab_seal.rs`, sync catch-up) call `chain.seal` without `state_snapshot.store()` — workers can lag behind RPC-immediate seals (medium nit)

## 5. Tests

| Test | Location | Covers |
|------|----------|--------|
| `snapshot_loads_stored` | `state.rs` | `store`/`load` swap |
| Worker tests | `worker.rs` | pass `StateSnapshot` into `spawn_worker`; no assertion on snapshot freshness |

**Missing (non-blocking for slice 1):**

- Integration: seal loop updates snapshot → worker `load()` sees new `fee_pool` / tip-derived field
- `WorkerPool::new` smoke with `app.state_snapshot` clone

## 6. Concurrency / parallelism

**Components:** `StateSnapshot` (`std::sync::RwLock<Arc<State>>`), tokio seal task (`lifecycle.rs`), OS-thread workers (`worker.rs`).

| Hazard | Assessment |
|--------|------------|
| `std::sync::RwLock` across tokio + OS threads | **Correct** choice vs `tokio::sync::RwLock` for sync worker threads |
| Lock held across `.await` | **No** — `store()` called while `inner.write()` held, but `StateSnapshot` lock is brief (no await inside) |
| Stale `Arc` in worker | **Expected** — worker keeps loaded `Arc` for job duration; seal publishes new `Arc` via `store` |
| `store` during `inner.write()` | Seal serialization prevents concurrent seals; workers can still `load()` old `Arc` concurrently — safe immutability |
| Channel / backpressure | Unchanged in this slice |

**Test gap:** no stress test interleaving `store()` + concurrent `load()` from multiple worker threads (low risk with `Arc` immutability).

## 7. Findings (prioritized)

### Medium

1. **Seal path clones full `State` on every block** — `lifecycle.rs:1830` `Arc::new(g.chain.st.clone())`. Plan §Slice 1 warns workers must `arc_clone`, not `State::clone()`. Workers comply; **publisher** clones O(accounts) per seal. Acceptable scaffold for slice 1; track for slice 2 perf (consider `Arc::clone` of in-chain `Arc` if `Chain.st` becomes `Arc<State>`).

2. **`store()` only on main lifecycle seal loop** — grep shows `state_snapshot.store` at single site. RPC/lab/sync `chain.seal` bypass snapshot. Until all seals funnel through lifecycle, workers may observe stale tips.

### Low / nits

3. **`WorkerPool::new` not wired to `app.state_snapshot` in production** — signature ready; only tests call `spawn_worker` with snapshot. Matches incremental V7-S1 isolation; slice 2 hookup required.

4. **`TxEntry` types unused** — exported from `pipeline/mod.rs` but ingress still `SignedTx` only. Fine for future-proofing; `Rejected { reason: String }` diverges from plan `TxRejectReason`.

5. **Bootstrap one-time `st.clone()`** — `bootstrap.rs:83,216,294` clones `State` into initial `Arc` at boot (once per App).

6. **Worker `load()` result unused** — `_state = snapshot.load()` until slice 2 precheck; plumbing only.

## 8. Verdict

**Approve with nits** — slice 1 post-conditions met for `StateSnapshot` plumbing, bootstrap init, lifecycle `store()` ordering, and unit test. No blockers for slice 2 precheck work. Address medium nits (alternate seal paths, O(N) `store` clone) before relying on snapshot for correctness at scale.

## 9. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260626-v7-s2-slice1-state-snapshot-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 45000, "confidence": "medium" }`