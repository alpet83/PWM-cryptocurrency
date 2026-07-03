# Review: perfmon S1 — PerfEntity/PerfScope core module (2fb5b65)

- date: 2026-06-28
- ticket: `20260628-perfmon-s1-review`
- coding_ticket: `20260628-perfmon-s1`
- commit: `2fb5b65`

## 1. Scope recap

Review commit `2fb5b65` — new module `crates/pwmd/src/perfmon.rs` only:

| area | change |
|------|--------|
| `PerfEntity` | Static counters: `calls`, `success`, `wall_ns` (`AtomicU64`, Relaxed) |
| `PerfScope` | RAII guard; `end(success)` or `Drop` → `finish` |
| `PerfSnapshot` | Serializable point-in-time view (`Serialize`) |
| `REGISTRY` | `&[&PerfEntity]` over four preset statics |
| `lib.rs` | `pub(crate) mod perfmon;` |
| tests | `perf_scope_end_ok`, `perf_scope_drop_fail` |

No call-site hooks yet (S1 scaffolding). Relates to V7 perf observability groundwork.

## 2. Requirements fit

| Focus area | Verdict | Evidence |
|------------|---------|----------|
| Relaxed memory ordering | **PASS** | All `fetch_add` / `load` use `Relaxed` (`:53-55`, `:78-82`) — correct for monotonic process-local observability counters |
| RAII double-count on `end()` + `Drop` | **PASS** | `finish` gates on `ended` (`:73-75`); `end(mut self)` sets `ended=true` before implicit `Drop` (`:68-70`, `:86-89`) |
| `PerfScope` across `.await` | **PASS** with usage nit | `Instant` + `&'static PerfEntity` are `Send` — compiler allows hold across await; wall time would include scheduler wait (see §8) |
| `REGISTRY` linker retention | **PASS** with nit | Array references all four statics (`:97-102`); keeps symbols alive **when `REGISTRY` is referenced**. No external use yet — see nit |
| Serde on `PerfSnapshot` | **PASS** | `serde.workspace = true` already in `pwmd/Cargo.toml:40`; derive-only `Serialize`, no new dep |

## 3. Memory ordering analysis

`Relaxed` is appropriate here:

- Counters are **not** used for control-flow synchronization (unlike pipeline `TxCounters` / lease flags).
- Each `finish` performs three independent `fetch_add` operations; no cross-field invariant must be atomic at increment time.
- `snapshot()` may observe brief skew (e.g. `calls` incremented before `wall_ns` visible) — acceptable for status/export perf metrics.

`AcqRel` / `SeqCst` on `success` would not fix snapshot consistency without a snapshot lock or a single packed atomic — out of scope for S1.

**Optional later improvement (nit):** document that `fail` and `avg_ns_per_call` are best-effort under concurrent writers; or add a `snapshot_all(registry)` that reads each entity sequentially for export-only paths.

## 4. RAII / double-count proof

`finish` path:

```72:83:crates/pwmd/src/perfmon.rs
    fn finish(&mut self, success: bool) {
        if self.ended {
            return;
        }
        self.ended = true;
        let elapsed = self.started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        self.entity.calls.fetch_add(1, Ordering::Relaxed);
        if success {
            self.entity.success.fetch_add(1, Ordering::Relaxed);
        }
        self.entity.wall_ns.fetch_add(elapsed, Ordering::Relaxed);
    }
```

- `scope.end(true)` consumes `self`, calls `finish(true)` (`ended=true`), then `Drop` runs on the same value and `finish(false)` returns immediately — **one** increment set.
- Drop-without-`end`: `finish(false)` once — tested in `perf_scope_drop_fail`.

`elapsed` clamp via `.min(u128::from(u64::MAX))` avoids truncation panic on exotic durations — good defensive cast.

## 5. Style and module shape

- Module `//!` banner present (`:1-2`).
- Identifiers within policy: prod types `PerfEntity` / `PerfScope` / `PerfSnapshot`; test fns `perf_scope_end_ok`, `perf_scope_drop_fail` (≤5 segments).
- `#![allow(dead_code)]` at module scope (`:2`) — reasonable until S2 hooks reference `REGISTRY` / `PERF_*`.
- `check_entity_name_segments.py`: **UNVERIFIED** (shell unavailable).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 6. Safety

- No panics on hot path (`finish` uses saturating atomics and clamped cast).
- `PerfEntity::new` is `const fn` — static initialization safe.
- Trust boundary: snapshots are local observability only (not yet wired to RPC).

## 7. Tests

| case | covered |
|------|---------|
| `end(true)` → calls=1, success=1, wall_ns>0 | `perf_scope_end_ok` |
| drop without `end` → fail=1 | `perf_scope_drop_fail` |
| `end()` then `Drop` no double-count | implicit via `ended` flag — **no explicit test** (nit) |
| concurrent `finish` on one entity | not tested — acceptable for S1 |

`cargo test -p pwmd perfmon`: **UNVERIFIED** (shell unavailable).

## 8. Concurrency / parallelism

Components: four `static PerfEntity` values, lock-free `AtomicU64` increments, `REGISTRY` slice for export iteration.

| hazard | assessment |
|--------|------------|
| Shared mutable state | `AtomicU64` only — no `Mutex`; correct for counter hot path |
| Race windows | Concurrent `finish` on same entity: each call counted once; snapshot may be briefly inconsistent across fields — OK for perf export |
| `PerfScope` across `.await` | **Usage hazard:** wall_ns includes time task is parked; hook authors should scope narrowly or call `end` before await |
| `Send` / `Sync` | `PerfScope` is `Send` (can migrate across Tokio workers) — entity ref is `'static` |

Test gaps: no multi-threaded hammer test on one `PerfEntity` (low priority for S1).

## 9. Verdict

**Approve with nits**

Prioritized nits:

1. **USAGE-1 (low):** Add `///` on `PerfScope` — do not hold across `.await` unless wall time should include scheduler latency.
2. **HOOK-1 (low):** Until S2 references `REGISTRY` (or `PERF_*`), LTO may strip unused statics from the final binary — ensure first hook touches `REGISTRY`.
3. **TEST-1 (low):** Add `perf_scope_end_no_double` — `end(true)` then rely on `ended` guard (assert calls==1 after drop).
4. **LINT-1 (low):** Replace module-wide `#![allow(dead_code)]` with targeted `allow` on statics once hooks land.

No blockers on memory ordering or RAII correctness.

## 10. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260628-perfmon-s1-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 22000, "confidence": "medium" }`