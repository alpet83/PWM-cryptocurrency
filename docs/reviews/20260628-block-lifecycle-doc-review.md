# Review: `docs/block-lifecycle.md` accuracy, benchmarks, scalability

- date: 2026-06-28
- ticket: `20260628-block-lifecycle-doc-review`
- artifact: `docs/block-lifecycle.md` (updated in-place)
- code baseline: `eec711d` / V7-S3

## 1. Scope recap

Review and correct operator-facing lifecycle doc: worker/seal path accuracy, `seal_phase_timings` coverage map, scalability section with O-complexity and soak context (~100 accounts, 72–88 tx/block, slip 170–400 ms).

## 2. Requirements fit

| Criterion | Verdict | Evidence |
|-----------|---------|----------|
| Worker affinity/general/Mutex/semaphore accuracy | **PASS** | Corrected vs `worker.rs:217-260`, `bootstrap.rs:69-72` |
| Seal loop scheduler/gates/eviction | **PASS** | Added `gate_recheck`, `skip_evicted`/`dedup`, deadline bump (`lifecycle.rs:1793-2071`) |
| `seal_phase_timings` phase map | **PASS** | §10 — 6 measured phases + explicit not-covered list (`chain.rs` test body) |
| Scalability section | **PASS** | §11 — O-table, block-interval ceiling, optimization paths, soak refs |
| Doc saved with fixes | **PASS** | `docs/block-lifecycle.md` rewritten |

## 3. Corrections applied (prior doc errors)

| was wrong / missing | fix |
|---------------------|-----|
| General worker only polls `client_tx` | General round-robins three queues (`worker.rs:227-255`) |
| Fixed "8 workers" implied | Documented `host_worker_counts`: 1 + `(cpus/2-1).max(1)` |
| Queue cap unspecified | caps 256 / validated 4096 (`bootstrap.rs:99-100`) |
| Seal error → simple requeue | Eviction + `evicted_hashes` + `dedup` + 50 ms backoff |
| `verify_sig` in production hot path | `debug_assert!` only in release (`chain.rs:235-238`) |
| Post-seal order | `enqueue_sealed_block` after `drop(g)`; autosnap flushes writer first |
| BlockWriter error handling | Fail-fast skip + warn (`block_writer.rs:113-118`) |
| Missing `sync_epoch_to_tip` before writer init | §7 |
| No benchmark / scalability sections | §10, §11 |

## 4. Style and module shape

Documentation-only slice; prose in Russian matching prior doc. No production code changes.

### Wire JSON / u128

Wire JSON / u128: not applicable (documentation slice).

## 5. Safety / accuracy notes

1. **`seal_phase_timings` name collision in `chain.rs`** — short DeterministicHeight test and full microbench share similar naming; doc §10 notes this so operators grep stdout correctly.

2. **Orphan benchmark fragment** — end of `chain.rs` `mod tests` may not compile as-is (module closes before benchmark constants). Doc describes intended phases from benchmark body; recommend pwm-coding repair in separate ticket.

3. **Slip numbers** — sourced from `v7-s2-ramp-results.md` (debug soak 1256 ms peak; user context 170–400 ms release) — doc cites both patterns.

## 6. Tests

Doc review only. Benchmark invocation documented: `cargo test -p pwm-core seal_phase_timings -- --nocapture`.

## 7. Concurrency / parallelism

Doc now accurately states: write-lock serializes seal+post indices; workers parallel under semaphores; BlockWriter async after lock release; blocking `mpsc::send` backpressure documented.

## 8. Findings (prioritized)

### Medium

1. **`chain.rs` `seal_phase_timings` source integrity** — benchmark body appears outside closed `mod tests`; operators following doc may get compile failure until code fix.

### Low

2. **Soak numbers mix debug/release** — §11 references both; keep release soak trace when available.

3. **`first_bad_tx_ctx` vs PreValidated** — eviction simulation caveat not in lifecycle doc (out of scope for block lifecycle, noted in eviction review).

## 9. Verdict

**Approve with nits** — doc now matches V7-S3 code paths, includes benchmark marker table and scalability analysis. Residual nit: repair `seal_phase_timings` test harness in `chain.rs` (code ticket, not doc).

## 10. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/block-lifecycle.md`, `docs/reviews/20260628-block-lifecycle-doc-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 45000, "confidence": "medium" }`