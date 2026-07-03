# Review: ArcSwap snapshot + worker pool + inc_rejected (249004e)

- date: 2026-06-28
- ticket: `20260628-arc-swap-review`
- coding_ticket: `20260628-arc-swap-snapshot`
- commit: `249004e`
- prior review: `docs/reviews/20260628-tx-counters-review.md`

## 1. Scope recap

| change | file |
|--------|------|
| `StateSnapshot` → `Arc<ArcSwap<State>>` | `state.rs` |
| Seal publish `store(Arc::new(st.clone()))` | `lifecycle.rs:1902-1903` |
| Worker reads `snapshot.load()` → `Arc<State>` | `worker.rs:399` |
| `worker_counts`: `(logical/2-1)` → `saturating_sub(2).max(1)` | `bootstrap.rs:69-90` |
| `inc_rejected_by(txs.len())` on full-batch requeue | `lifecycle.rs:2104-2111` |
| `TxCounters` doc comment | `counters.rs:10-12` |

## 2. Requirements fit

| Focus area | Verdict | Evidence |
|------------|---------|----------|
| ArcSwap correctness / callsites | **PASS** | `load_full()` / `store()`; no `unsafe`; test `snapshot_loads_stored` (`state.rs:387-398`) |
| `store` visibility for workers | **PASS** | Atomic pointer swap; next `load()` sees new `Arc`; held old `Arc` stays valid until drop (RCU) |
| `worker_counts` formula | **PASS** | `saturating_sub(2).max(1)` — 1-core→1, 2-core→1, 4→2, 16→14; tests updated (`bootstrap.rs:86-89`) |
| `inc_rejected` on requeue branches | **PARTIAL** | Wired with TODO; semantics stretch “rejected” (see §8) |
| `TxCounters` doc accuracy | **FAIL** (doc only) | `sealed + rejected <= incoming` false after batch reject increments |

## 3. ArcSwap analysis

```54:71:crates/pwmd/src/state.rs
pub struct StateSnapshot {
    inner: Arc<ArcSwap<pwm_core::state::State>>,
}
// load() -> Arc<State> via load_full()
// store() -> ArcSwap::store(state)
```

- **No Guard type** — API returns owned `Arc<State>`; lifetime is refcounted, not borrow-guarded. Correct for cross-thread use.
- **`precheck_full`** (`worker.rs:399-411`) — `let state = ctx.reads.snapshot.load()` then `state.evaluate_policy` / `state.precheck_apply_with_ctx` via `Arc` deref. Same pattern as former `Arc::clone` after lock.
- **Seal path** — still under `inner.write()`; `store` no longer blocks concurrent readers (improvement vs `RwLock`).
- **`rcu` not required** — single-writer `store` + multi-reader `load_full` matches HotIndex pattern (`hot_index.rs:34-35`).
- **Stale precheck** — workers may simulate on snapshot N while seal advances to N+1; handled by `validated_at_height` + seal eviction (unchanged contract).

## 4. Worker pool scaling

| logical CPUs | affinity | general (new) | general (old) | total threads |
|--------------|----------|---------------|---------------|---------------|
| 1 | 1 | 1 | 1 | 2 |
| 2 | 1 | 1 | 1 | 2 |
| 4 | 1 | 2 | 1 | 3 |
| 16 | 1 | 14 | 7 | 15 |

2-core → 1 general is intentional minimum (`max(1)`). 1-core uses `saturating_sub` — no underflow.

**Trade-off:** more general workers increase contention on `Mutex<Receiver>` per queue; acceptable for soak goal (more parallel precheck).

## 5. Style and module shape

- `StateSnapshot` API surface minimal (`new`, `load`, `store`).
- `arc-swap = "1"` already in `pwmd/Cargo.toml`.

### Wire JSON / u128

Wire JSON / u128: not applicable (in-process snapshot / status counters only).

## 6. Tests

| test | covers |
|------|--------|
| `snapshot_loads_stored` | ArcSwap store visible on load |
| `worker_counts_scale` | new formula 1/4/16 |
| Worker tests use `snapshot.load()` / `store` | `worker.rs:730-737` |

**Gap:** no concurrency test (load during store); no test for requeue `inc_rejected` branches.

## 7. Concurrency / parallelism

**Components:** `ArcSwap<State>` readers (workers); writer (seal task `store`); expanded worker pool.

| Hazard | Assessment |
|--------|------------|
| Read during `store` | **Safe** — atomic swap; readers keep old `Arc` |
| Seal `store` under `write` lock | **OK** — snapshot matches `g.chain.st` at commit point |
| More OS threads | **Medium** — mutex queue contention may rise; monitor soak |
| `Relaxed` tx counters + torn snapshot | Unchanged from prior slice |

## 8. Findings (prioritized)

### Medium

1. **`TxCounters` doc invariant wrong** — `counters.rs:11` states `sealed + rejected <= incoming`. After `inc_rejected_by(txs.len())` on requeue (`lifecycle.rs:2105,2111`), one failed 64-tx seal adds 64 to `rejected` while txs return to pool — **`rejected + sealed` can exceed `incoming`** (tx-level vs HTTP-level). Fix doc to: incoming = HTTP ingress; sealed/rejected = tx outcomes; no strict sum bound.

2. **`inc_rejected` on requeue is observability proxy, not terminal reject** — TODO comments acknowledge (`2104,2110`). Inflates `rejected` on transient seal failures (reward invariant, `first_bad_tx_ctx` miss). Acceptable for pressure signal if documented; misleading vs HTTP 422 rejects. Consider `seal_retry_tx` counter later.

3. **`next_apply_ctx` fail branch still no counter** — `lifecycle.rs:2045-2051` `continue` after prepend without `inc_rejected` — inconsistent with other requeue paths.

### Low

4. **`store` clones full State** — `Arc::new(g.chain.st.clone())` before swap — same cost as before; ArcSwap removes lock contention only.

5. **15 workers on 16 CPUs** — leaves little headroom for seal/tokio/BlockWriter; monitor CPU saturation.

## 9. Verdict

**Approve with nits** — ArcSwap migration is correct and improves precheck read concurrency; worker scaling formula uses `saturating_sub` correctly; requeue `inc_rejected` is a deliberate pressure hack but **must fix `TxCounters` doc comment** (code comment inaccuracy, not ArcSwap bug).

## 10. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260628-arc-swap-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 35000, "confidence": "medium" }`