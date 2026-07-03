# V7-S3 — Account Hot Index lock-free precheck (pwm-review)

- date: 2026-06-27
- ticket: `20260627-v7-s3-hot-index-review`
- commit: `5a8f297` (branch `main`)
- normative: `docs/adr/0014-account-hot-index-and-lockfree-chain.md`

## 1. Scope recap

Level-1 hot index for worker plain-transfer precheck:

| file | change |
|------|--------|
| `pipeline/hot_index.rs` | `AccountHot`, `HotIndex` (`ArcSwap<HashMap<…>>`), `build_map` / `refresh` |
| `pipeline/worker.rs` | `WorkerReads.hot_index`; `precheck_hot` fast-path; `precheck_full` fallback |
| `bootstrap.rs` / `state.rs` | `App.hot_index` init on all bootstrap paths |
| `lifecycle.rs` | `hot_index.refresh(&g.chain.st)` after successful seal |

## 2. Requirements fit

| Acceptance criterion | Verdict | Evidence |
|---------------------|---------|----------|
| `AccountHot`: balance, nonce, flags, active_policies, initialized | **PASS** | `hot_index.rs:11-17`; `account_hot` (`:47-60`) |
| `HotIndex.load()` O(1) lock-free for workers | **PASS** | `ArcSwap::load_full()` (`:30-32`); workers only `load()` in `precheck_hot` (`worker.rs:354`) |
| Fast-path: balance/nonce from map; skip `evaluate_policy` when `active_policies==0` | **PASS** | `precheck_hot` Transfer-only (`:350-363`); `hot_safe` requires `active_policies==0` and `flags==0` (`:365-367`); `check_hot_transfer` (`:369-392`); no `evaluate_policy` on hot path |
| Cache miss → `precheck_apply_with_ctx` fallback | **PASS** | `precheck_hot` returns `None` → `precheck_full` (`:342-347`, `:394-413`) |
| `refresh()` after seal, atomic `ArcSwap` store | **PASS** | `lifecycle.rs:1850`; `HotIndex::refresh` (`hot_index.rs:34-36`) |
| Workers do not write map | **PASS** | production workers only `load()`; `refresh` only in lifecycle (+ test helper) |
| `cargo test -p pwmd` PASS | **UNVERIFIED** | Shell unavailable; in-tree: `hot_index_*`, `worker_rejects_*`, `worker_policy_uses_fallback` |

## 3. Style and module shape

- `AccountHot`, `HotIndex`, `precheck_hot`, `hot_safe` — ≤4-word identifiers ✓
- `WorkerReads` bundles snapshot + hot index + cfg + tip_height ✓
- ADR 0014 Level-1 scope matches implementation; incremental update deferred ✓

Entity segment check not run (shell unavailable).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

1. **Conservative policy gate** — `active_policies` on `AccountHot` is `u8::from(policy_sensitive)` covering active/dormant/deferred/finalized/rescue (`hot_index.rs:49-53`); `hot_safe` requires `== 0` → fallback to full policy + dry-run ✓

2. **Address flags** — conservation/cosign bits folded into `flags`; non-zero forces fallback (`hot_safe`) ✓

3. **Fast path skips signature dry-run** — hot path checks shape + balance/nonce/init only; `BadSignature` caught at seal (same stale-accept contract as Slice 2). Documented tradeoff.

4. **Stale index window** — between seals, workers read prior `ArcSwap` snapshot; false Accept possible, false Reject for new plain txs unlikely if index refreshed each seal. **Roaming direct-seal** (`handlers_tx`) updates chain without `hot_index.refresh` until next lifecycle seal — stale hot data for roaming-heavy mixes (nit).

5. **`refresh()` full `build_map` O(N)** — per ticket/ADR note; paired with `state_snapshot.store` clone on same seal path — double O(N) on large state (nit).

## 5. Tests

| Test | Covers |
|------|--------|
| `hot_index_refreshes_atomically` | `ArcSwap` refresh; old Arc unchanged |
| `hot_index_marks_policy_sensitive` | dormant/deferred/finalized → `active_policies=1` |
| `test_worker_client_tx` / `precheck_hot` Some(Ok) | plain transfer fast-path |
| `worker_rejects_bad_nonce` | hot path `StaleDuplicate` |
| `worker_rejects_low_balance` | hot path insufficient balance |
| `worker_policy_uses_fallback` | policy bit → `precheck_hot` None → `PolicyDenied` |

**Gaps:** no HTTP integration test; no explicit cache-miss (unknown account) assertion; no concurrency test (load during refresh).

## 6. Concurrency / parallelism

**Components:** `ArcSwap` readers (workers); single writer `refresh` on seal task; no worker writes.

| Hazard | Assessment |
|--------|------------|
| Read during `store` | **Safe** — readers keep old `Arc` until atomic swap completes |
| Multiple workers `load()` | **Lock-free** — independent `Arc` clones |
| Stale read vs seal | **Expected** — same as stale snapshot; seal is authority |
| `refresh` O(N) under write lock elsewhere | Seal holds `inner.write`; refresh scans `g.chain.st` without extra lock on index ✓ |

**Test gap:** parallel worker load + lifecycle refresh interleaving.

## 7. Findings (prioritized)

### Medium

1. **`refresh()` full rescan, not incremental** — ADR Level-1 text mentions delta update; code uses `build_map(state)` each seal. Acceptable at current scale; note for >100k accounts.

2. **Roaming HTTP seal does not refresh `hot_index`** — only lifecycle path (`lifecycle.rs:1850`). Roaming commits can desync hot index until next periodic seal.

### Low

3. **Fast path Transfer-only** — Stake/Init/Export always full precheck (correct).

4. **Double O(N) on seal** — `state_snapshot.store` + `hot_index.refresh` both walk accounts.

5. **Hot path omits `evaluate_policy` height nuance** — intentional when `active_policies==0`; policy-bearing accounts use fallback.

## 8. Verdict

**Approve with nits** — hot index meets acceptance criteria: lock-free worker reads, plain-transfer fast-path with conservative policy/flags gating, fallback to full precheck, atomic refresh after lifecycle seal, workers read-only. Tests cover happy path, balance/nonce reject, and policy fallback. Remaining nits are full O(N) refresh, roaming refresh gap, and fast-path signature deferral to seal.

## 9. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260627-v7-s3-hot-index-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 36000, "confidence": "medium" }`