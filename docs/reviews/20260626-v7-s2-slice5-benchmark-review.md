# V7-S2 Slice 5 — determinism + DoS + ramp soak (pwm-review)

- date: 2026-06-26
- ticket: `20260626-v7-s2-slice5-benchmark-review`
- commit: `e586623` (branch `mvp-v7`)
- normative: `docs/plans/mvp_v7s2.md` § Slice 5

## 1. Scope recap

Slice 5 adds CI-safe pipeline pressure/determinism tests, ramp soak script CLI aliases, and a results doc:

| file | change |
|------|--------|
| `tests/determinism.rs` | `determinism_1_vs_n_workers`, `dos_512_post_507_ready` |
| `tests/mod.rs` | `mod determinism;` |
| `scripts/cy_cluster_transfer_ramp_soak.py` | `--url`, `--duration`, `--target-tps` CLI aliases |
| `docs/reviews/v7-s2-ramp-results.md` | gate table + live-soak command template |

## 2. Requirements fit

| Acceptance criterion | Verdict | Evidence |
|---------------------|---------|----------|
| `determinism_1_vs_n_workers`: state_hash identical at 1 vs 8 workers | **FAIL** | Test exists (`determinism.rs:60-62`) and `assert_eq!` passes, but `run_pipeline` never starts `WorkerPool`, never dispatches through workers, and `workers` only changes `chunks(n)` size — `flat_map` preserves original tx order for any `n≥1`. Same seal sequence ⇒ trivially identical `state_hash`. Does not satisfy plan § Slice 5 property test |
| DoS: 512 POST → 507, `/v1/status` ready after | **PASS** | `dos_512_post_507_ready` (`determinism.rs:92-135`): replaces `worker_queues` with cap-1 `DispatchQueues`, prefills queue, 512 concurrent POSTs all `INSUFFICIENT_STORAGE`, then `ready==true` |
| `cy_cluster_transfer_ramp_soak.py` with `--url` / `--duration` / `--target-tps` | **PASS** | `scripts/cy_cluster_transfer_ramp_soak.py:588,602,604` — aliases on `--rpc`, `--max-txs-per-block`, `--soak-sec` |
| `docs/reviews/v7-s2-ramp-results.md` exists | **PASS** | Present; documents coding gate and live command |
| `cargo test -p pwmd -p pwm-core` PASS | **UNVERIFIED** | Shell unavailable in review session |

**Plan § Slice 5 post-condition gap:** ramp doc states `live 60s soak | not run in coding slice`; no evidence of sustained ≥50 tx/s, 0 evictions, or 0 seal-determinism errors from a cluster run.

## 3. Style and module shape

- `determinism.rs` module banner and test names within budget ✓
- DoS helper `fill_client_queue` — clear bounded-queue saturation ✓
- Ramp script argparse aliases additive, backward-compatible ✓

Entity segment check not run (shell unavailable).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

1. **DoS test isolates dispatch saturation** — swaps `app.worker_queues` to a fresh cap-1 queue whose receivers are **not** wired to `_worker_pool` workers (`determinism.rs:95-96`). Producers fill queue; no consumer drains during flood. Valid for 507 contract test; not representative of production worker+validated path under load.

2. **DoS test timeout** — 10s `tokio::time::timeout` around JoinSet prevents hung test ✓

3. **Determinism test** — single-threaded `Chain::seal_entries` only; gives no signal on worker reordering, channel races, or seal-batch composition hazards.

## 5. Tests

| Test | Covers |
|------|--------|
| `determinism_1_vs_n_workers` | **Misleading** — state hash equality without parallel workers |
| `dos_512_post_507_ready` | HTTP 507 under saturated client dispatch queue; node stays ready |

**Missing per plan § Slice 5:**

- Real `run_pipeline(txs, workers=N)` using `WorkerPool` + `dispatch` + validated drain + `seal_entries` (or in-process harness) comparing final `state_hash` at N=1 vs N=8
- Live or recorded ramp soak meeting ≥50 tx/s sustained with eviction/determinism log checks
- DoS under default bootstrap worker pool (optional complement to isolated queue test)

## 6. Concurrency / parallelism

**Components (DoS test):** 512 tokio tasks via `JoinSet`, shared `router_dev` service, saturated `BoundedQueue` dispatch, orphaned worker threads on old receivers.

| Hazard | Assessment |
|--------|------------|
| Determinism test parallelism | **Not exercised** — no worker threads, no channel interleaving |
| DoS flood + status probe | **OK** — sequential status after flood completes; no shared mutable test state |
| Queue disconnected from workers | **Test artifact** — intentional saturation; masks real worker drain behavior |

**Test gap:** interleaved worker completions with variable worker count affecting seal entry order (the actual V7-S2 determinism concern).

## 7. Findings (prioritized)

### Blocker

1. **`determinism_1_vs_n_workers` does not test worker parallelism** — `run_pipeline` (`determinism.rs:41-57`) seals txs sequentially on one `Chain`; `workers` parameter does not alter ordering (mathematically `chunks(k).flat_map` ≡ identity order). Plan property test requires `run_pipeline(&txs, workers=1)` vs `workers=8` through the real pipeline. Current test would pass even if worker pool reordering were broken.

### High

2. **Live ramp soak not executed** — `v7-s2-ramp-results.md` explicitly records `live 60s soak | not run`. Sprint acceptance (≥50 tx/s, 0 evictions) unproven; only script + doc scaffold delivered.

3. **DoS test uses synthetic queue setup** — not default `WorkerPool` + validated path; proves handler 507 mapping, not full-node overload resilience.

### Low

4. **`--target-tps` aliases `--max-txs-per-block`** — per-block cap, not wall-clock TPS meter; acceptable alias but semantics differ from literal “transactions per second”.

5. **`init_batch(50)` uses Init txs only** — no Transfer/policy/precheck diversity in determinism fixture.

## 8. Verdict

**Request changes** — DoS admission test and ramp script CLI aliases are useful and meet their narrow ticket items. The flagship **determinism** criterion is not substantively implemented: the test name and plan reference worker counts, but the code never runs workers or varies execution order. Ramp results doc exists but documents absent live soak evidence.

**Before approve:** replace `run_pipeline` with a harness that exercises `WorkerPool` at 1 vs 8 workers through dispatch → validated → `seal_entries`, then compare `state_hash`; optionally attach live ramp log excerpt or mark sprint soak as deferred with orchestrator waiver.

## 9. Participation

- `agent`: `pwm-review`
- `result`: `FAIL`
- `artifacts`: `docs/reviews/20260626-v7-s2-slice5-benchmark-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 32000, "confidence": "medium" }`