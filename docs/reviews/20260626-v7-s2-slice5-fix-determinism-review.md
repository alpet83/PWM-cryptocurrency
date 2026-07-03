# V7-S2 Slice 5 fix — real WorkerPool determinism harness (pwm-review)

- date: 2026-06-26
- ticket: `20260626-v7-s2-slice5-fix-determinism-review`
- commit: `67cb829` (branch `mvp-v7`)
- normative: `docs/plans/mvp_v7s2.md` § Slice 5
- prior review: `docs/reviews/20260626-v7-s2-slice5-benchmark-review.md` (FAIL — stub determinism test)

## 1. Scope recap

Replaces the no-op `chunks().flat_map` determinism stub with a real worker harness in `tests/determinism.rs`:

| component | role |
|-----------|------|
| `init_batch(50)` | 50 unique-sender `Transfer` txs funded in genesis cfg |
| `run_worker_pool(n)` | `WorkerPool::new(1, n-1)`, dispatch all txs, collect `ValidatedTx` completion order |
| `seal_batch` | `Chain::seal_entries(PreValidated…)` → `state::digest` |
| `determinism_1_vs_n_workers` | compare order (must differ) and final digest (must match) |

`dos_512_post_507_ready` unchanged from prior slice.

## 2. Requirements fit

| Acceptance criterion | Verdict | Evidence |
|---------------------|---------|----------|
| `determinism_1_vs_n_workers()` runs real `WorkerPool(1)` and `WorkerPool(8)` | **PASS** | `run_worker_pool`: `WorkerPool::new(1, workers.saturating_sub(1), …)` (`determinism.rs:115`) → 1 worker (affinity only) vs 8 workers (1 affinity + 7 general). `dispatch` + `handle_client` + `valid_rx` on real OS threads |
| Test checks validated output order differs (non-triviality) | **PASS** | `assert_ne!(single.order, parallel.order)` (`:174`) where `order` is `computed_account_id()` sequence from `valid_rx.blocking_recv` completion order |
| State digest identical after sealing both batches | **PASS** | `assert_eq!(seal_batch(…single…), seal_batch(…parallel…))` (`:175-178`) |
| `cargo test -p pwmd` PASS | **UNVERIFIED** | Shell unavailable in review session |

**Prior blocker closed:** harness exercises worker parallelism (general workers use `try_client` races per `worker.rs:209-217`), not sequential fake reordering.

## 3. Style and module shape

- `run_worker_pool`, `seal_batch`, `WorkerRun` — test helpers ≤5 segments ✓
- Fixture builders (`find_sender`, `clear_policies`) keep test self-contained ✓
- `Transfer` batch with policy pre-check in fixture (`evaluate_policy` assert `:119-126`) ✓

Entity segment check not run (shell unavailable).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

1. **Commutative workload** — 50 independent sender→same-recipient transfers; permuting block apply order should not change final balances/nonces. Appropriate for order-invariance digest test ✓

2. **`clear_policies`** — removes deferred/active policy noise so worker precheck matches seal path ✓

3. **Workers joined before assertions** — `pool.handles` joined (`:144-146`) before seal; no use-after-free on pool ✓

4. **`assert_ne!` stability** — relies on 7 general workers racing `try_recv` vs single FIFO affinity worker; reordering expected with 50 txs. Theoretical flake if orders collide; low risk given general-worker contention design.

## 5. Tests

| Test | Covers |
|------|--------|
| `determinism_1_vs_n_workers` | WorkerPool 1 vs 8, order differs, digest matches |
| `dos_512_post_507_ready` | unchanged admission saturation (out of fix scope) |

**Gaps (non-blocking):**

- No lifecycle `drain_validated` / multi-block `pool.take` integration
- Dispatch loop is single-threaded (50 sequential `dispatch` calls), not concurrent HTTP producers
- Production bootstrap uses `WorkerPool::new(1, 1)` not `1+7`

## 6. Concurrency / parallelism

**Components:** `WorkerPool` OS threads, `Mutex<Receiver>` + `try_recv` on general workers, `mpsc` validated output, test thread `blocking_recv` on oneshot replies.

| Hazard | Assessment |
|--------|------------|
| General vs affinity client job stealing | **Exercised** — 8-worker config enables `run_general_rx` `try_client` races; explains order divergence |
| Shared `WorkerCtx` / `StateSnapshot` | **OK** — read-only snapshot load in workers; no chain mutation until seal |
| Test collects validated before join | **OK** — channel drained while workers still running; joins after drain complete |

**Test gap:** concurrent multi-producer dispatch (HTTP-level) interleaved with seal loop not covered here.

## 7. Findings (prioritized)

### Blocker

None — prior Slice 5 determinism blocker is closed.

### Low

1. **Harness stops before lifecycle** — validated txs collected in test, not via `app._validated_rx` + seal loop; acceptable scoped property test.

2. **Single `seal_entries` batch** — all 50 txs in one block; real node may split at cap 64.

3. **`assert_ne!` order check** — could add multiset equality fallback if CI flake ever observed (not required now).

4. **Worker count 8 ≠ production 2** — stronger parallelism than bootstrap default; fine for determinism stress.

## 8. Verdict

**Approve with nits** — fix delivers a substantive WorkerPool determinism property test matching plan intent: real workers at 1 vs 8, non-trivial completion-order difference, identical final state digest after seal.

## 9. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260626-v7-s2-slice5-fix-determinism-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 26000, "confidence": "medium" }`