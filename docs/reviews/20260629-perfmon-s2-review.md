# Review: perfmon S2 — hot-path instrumentation (37c2ae8)

- date: 2026-06-29
- ticket: `20260629-perfmon-s2-review`
- coding_ticket: `20260628-perfmon-s2`
- commit: `37c2ae8`
- prior: S1 `2fb5b65` (approved with nits)

## 1. Scope recap

Review commit `37c2ae8` — wire `perfmon` statics into hot paths:

| site | file | entity | scope |
|------|------|--------|-------|
| ed25519 / shape | `pipeline/worker.rs:341-343` | `PERF_ED25519` | `validate_tx_shape` in `precheck_client` |
| pool drain | `lifecycle.rs:1871-1901` | `PERF_POOL_DRAIN` | ingress + `_validated_rx` + `pool.take` |
| chain seal | `lifecycle.rs:1906-1908` | `PERF_CHAIN_SEAL` | `g.chain.seal_entries` |
| state apply (replay) | `lifecycle.rs:861-863` | `PERF_STATE_APPLY` | per-tx in `first_bad_tx_ctx` |
| registry anchor | `lifecycle.rs:2442` | `REGISTRY` | `debug!` at `run_with` startup |
| S1 nits | `perfmon.rs:28`, `:143-158` | — | await doc + `perf_scope_end_no_double` |

## 2. Requirements fit

| Focus area | Verdict | Evidence |
|------------|---------|----------|
| lifecycle scopes vs `.await` | **PASS** | `pool_scope` / `seal_scope` begin after `write().await` (`:1860-1908`); no `.await` between `begin`/`end` |
| `PERF_POOL_DRAIN` granularity | **PASS** with nit | Single scope covers combined drain — reasonable S2 aggregate; lock-failure paths still `end(true)` |
| `PERF_STATE_APPLY` success semantics | **PASS** | `end(apply_result.is_ok())` (`:863`) — OK = tx applies to sim state; Err = fail count |
| `PERF_ED25519` in worker | **PASS** | Sync `precheck_client` on `std::thread` worker — no yield; `validate_tx_shape` includes `verify_sig` (`pwm-core/tx.rs:618-630`) |
| `perf_scope_end_no_double` | **PASS** | Asserts `calls==1`, `success==1`, `fail==0` after `end(true)` + drop (`perfmon.rs:143-157`) |
| S1 await doc comment | **PASS** | `perfmon.rs:28` — explicit |
| dead_code / REGISTRY | **PASS** with nit | All four `PERF_*` referenced; `REGISTRY` touched at startup — module `#![allow(dead_code)]` still broad |

## 3. Call-site analysis

### `PERF_ED25519` (`worker.rs:339-343`)

```339:343:crates/pwmd/src/pipeline/worker.rs
fn precheck_client(tx: &SignedTx, ctx: &WorkerCtx) -> Result<ValidatedTx, TxRejectReason> {
    let _span = debug_span!("worker.precheck").entered();
    let sig_scope = perfmon::PERF_ED25519.begin();
    let shape_result = validate_tx_shape(tx);
    sig_scope.end(shape_result.is_ok());
```

- **Success flag:** `is_ok()` on full `validate_tx_shape` (sig + domain + body rules) — correct for “precheck shape+sig passed.”
- **No double-count:** explicit `end()`; `ended` guard prevents Drop recount.
- **Hot path:** `validate_tx_shape` runs once before `precheck_hot` short-circuit — sig verified even when hot path later succeeds (`:346-348`).
- **Nit (NAMING-1):** static name `ed25519_verify` understates non-crypto shape failures counted as `fail`.

### `PERF_POOL_DRAIN` (`lifecycle.rs:1871-1901`)

- Covers: `tx_ingress` try-drain, `_validated_rx` batch (≤64), `g.pool.take(remaining)`, dedup/skip prep.
- **Does not cross `.await`** — held under `write` guard, sync only.
- **`end(true)` always** — even when `try_lock` on ingress/validated_rx fails (drain skipped). Wall time still recorded; success bit does not signal lock contention. Acceptable for timing-only S2; nit for S3 export semantics.
- **Granularity nit (METRIC-1):** combined ingress + validated + pool in one entity — cannot attribute stall to one source without split counters later.

### `PERF_CHAIN_SEAL` (`lifecycle.rs:1906-1908`)

```1906:1908:crates/pwmd/src/lifecycle.rs
            let seal_scope = perfmon::PERF_CHAIN_SEAL.begin();
            let seal_result = g.chain.seal_entries(entries);
            seal_scope.end(seal_result.is_ok());
```

- **Success flag:** `seal_result.is_ok()` — seal commit OK vs `SealAbort` — correct.
- Includes full `seal_entries` work (apply loop, rewards, header) — appropriate aggregate hot-path metric.
- On `Err`, `first_bad_tx_ctx` runs separately with own `PERF_STATE_APPLY` scopes (`:2080`).

### `PERF_STATE_APPLY` (`lifecycle.rs:852-868`)

```860:863:crates/pwmd/src/lifecycle.rs
    for (i, tx) in txs.iter().enumerate() {
        let apply_scope = perfmon::PERF_STATE_APPLY.begin();
        let apply_result = sim.apply_tx_with_ctx(tx, blk_h, blk_ts, gen_cfg);
        apply_scope.end(apply_result.is_ok());
```

- **Semantics:** per-tx sim apply during seal-failure replay — `success` = tx valid under current chain state.
- **Not on happy-path seal:** production applies inside `seal_entries` are counted only via `PERF_CHAIN_SEAL` wall time, not per-tx `state_apply` calls.
- **Nit (COVERAGE-1):** `state_apply` counter reflects error-replay/diagnostic path volume, not steady-state apply throughput — document when exporting metrics.

## 4. S1 nit closure

| S1 nit | S2 status |
|--------|-----------|
| PerfScope `.await` doc | **Fixed** — `perfmon.rs:28` |
| `perf_scope_end_no_double` test | **Fixed** — `perfmon.rs:143-158` |
| REGISTRY linkage | **Fixed** — `run_with` debug + all statics used |
| Module `allow(dead_code)` | **Open** — still module-wide (`:2`) |

## 5. Style and module shape

- Instrumentation is minimal RAII at call sites; no new long identifiers.
- English comments unchanged at new sites (none added — fine).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 6. Safety

- No new panics; scopes use existing `end`/`Drop` guard.
- Counters remain observability-only (not yet on `/v1/status`).

## 7. Tests

| test | status |
|------|--------|
| `perf_scope_end_ok` | present |
| `perf_scope_drop_fail` | present |
| `perf_scope_end_no_double` | present (S2) |
| Integration: counters increment under load | **missing** — acceptable for S2 |

`cargo test -p pwmd perfmon`: **UNVERIFIED** (shell unavailable).

## 8. Concurrency / parallelism

Components: lock-free `AtomicU64` on static `PerfEntity` values; workers (`std::thread`) and seal loop (`tokio`) may increment same entity concurrently.

| hazard | assessment |
|--------|------------|
| Shared `PerfEntity` statics | `Relaxed fetch_add` — correct; snapshot may show brief field skew |
| `PerfScope` across `.await` | **None** at instrumented sites — scopes are sync between begin/end |
| Seal loop `write().await` then sync scopes | Correct pattern — wall time excludes lock wait |
| Worker threads + seal loop on `PERF_*` | No mutex on counters — no deadlock risk |

Test gaps: no concurrent hammer on one `PerfEntity` (low priority).

## 9. Verdict

**Approve with nits**

Prioritized nits:

1. **COVERAGE-1 (low):** `PERF_STATE_APPLY` counts replay applies in `first_bad_tx_ctx` only — happy-path per-tx apply is folded into `PERF_CHAIN_SEAL`; document for operators.
2. **METRIC-1 (low):** `PERF_POOL_DRAIN` is one combined scope; consider split if ingress vs validated stalls need attribution.
3. **SEMANTIC-1 (low):** `PERF_POOL_DRAIN` always `end(true)` — success bit is not a drain-health signal.
4. **NAMING-1 (low):** `ed25519_verify` entity includes non-sig `validate_tx_shape` failures.
5. **EXPORT-1 (low):** Wire `REGISTRY` snapshots into `/v1/status` (follow-up slice).
6. **LINT-1 (low):** Narrow `#![allow(dead_code)]` now that statics are referenced.

No blockers on placement, double-count, or async-boundary correctness.

## 10. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260629-perfmon-s2-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 26000, "confidence": "medium" }`