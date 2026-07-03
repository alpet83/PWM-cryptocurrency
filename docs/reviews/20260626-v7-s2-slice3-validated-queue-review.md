# V7-S2 Slice 3 — ValidatedQueue drain + SealEntry fast-path (pwm-review)

- date: 2026-06-26
- ticket: `20260626-v7-s2-slice3-validated-queue-review`
- commit: `ea3d3f1` (branch `mvp-v7`)
- normative: `docs/plans/mvp_v7s2.md` § Slice 3, `docs/adr/0013-tx-pipeline-seda.md`

## 1. Scope recap

Slice 3 connects worker `ValidatedTx` output to the seal loop and adds a policy fast-path in `pwm-core`:

| file | change |
|------|--------|
| `pwm-core/chain.rs` | `SealEntry { Raw, PreValidated }`, `seal_entries()`, unit tests |
| `pwm-core/state.rs` | `apply_prechecked_tx()` (`skip_policy=true` via `apply_tx_impl`) |
| `pwm-core/lib.rs` | re-export `SealEntry` |
| `lifecycle.rs` | drain `_validated_rx` → `SealEntry::PreValidated`, then `pool.take` → `Raw`; call `seal_entries` |
| `pipeline/queue.rs` | `ValidatedTx { tx, validated_at_height }` |
| `pipeline/worker.rs` | emit `validated_at_height: snapshot_height` on success |

## 2. Requirements fit

| Acceptance criterion | Verdict | Evidence |
|---------------------|---------|----------|
| `lifecycle.rs` drains `validated_rx` before `pool.take(64)` | **PASS** | `lifecycle.rs:1817-1831` — validated `try_recv` loop fills `entries` first; `remaining = block_cap - entries.len()`; `g.pool.take(remaining)` after |
| `SealEntry { Raw, PreValidated }` in pwm-core | **PASS** | `chain.rs:16-18`, exported `lib.rs:27` |
| `PreValidated` at `at_height == tip` skips `evaluate_policy` | **PASS** | `chain.rs:190-192` → `apply_prechecked_tx`; `state.rs:366-374` sets `skip_policy=true`; test `seal_fresh_prechecked_skip_policy` (`chain.rs:410-458`) |
| Stale `PreValidated` uses normal path | **PASS** | `chain.rs:193-195` → `apply_tx_with_ctx`; test `seal_stale_prechecked_uses_policy` (`chain.rs:461-502`) |
| `Chain::seal` determinism preserved | **PARTIAL** | `seal()` still maps to all-`Raw` entries (`chain.rs:165-167`); atomic clone-and-apply unchanged. **Regression:** duplicate admission (below) reintroduces `seal_skip: evicting unapplicable tx` for normal HTTP traffic |
| `cargo test -p pwmd -p pwm-core` PASS | **UNVERIFIED** | Shell unavailable in review session |

**Plan deviation:** `mvp_v7s2.md` § Slice 3 says drain `ValidatedQueue` → `g.pool`; implementation feeds validated txs **directly** into `seal_entries` while HTTP still enqueues the same txs into `tx_ingress` → `g.pool`. End-to-end path is not single-queue.

## 3. Style and module shape

- `SealEntry`, `seal_entries`, `apply_prechecked_tx` — ≤4-word production identifiers ✓
- `ValidatedTx.validated_at_height` aligns with plan field name ✓
- `seal_entries` reuses existing seal body; minimal API surface extension ✓

Entity segment check not run (shell unavailable).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

1. **Fast-path is hint-only** — `apply_prechecked_tx` still runs full `apply_tx_impl` except `evaluate_policy`; block still atomic (`chain.rs:181-200`, commit only after all entries OK). Matches plan § Slice 3.

2. **Stale path insurance** — `at_height != tip_before` falls back to `apply_tx_with_ctx` with full policy. Correct per plan stale-snapshot nuance.

3. **BLOCKER — duplicate tx in seal batch** — non-roaming HTTP (`handlers_tx.rs:221-227`) awaits worker precheck (worker `try_send`s `ValidatedTx` at `worker.rs:295`) **and** `tx_ingress.try_send(tx)`. Lifecycle (`lifecycle.rs:1812-1831`) drains ingress → `g.pool` **and** validated → `PreValidated`, then merges both into one `entries` vec. Same `SignedTx` can appear twice (PreValidated + Raw) in one block → second apply fails `BadNonce` → `seal_skip: evicting unapplicable tx` (`lifecycle.rs:1957-1962`). Reintroduces the eviction storm Slice 3 was meant to eliminate.

4. **`validated_tx` try_send still silent** — worker returns `Ok(())` to HTTP even if `try_send` drops validated output (`worker.rs:295`); HTTP then relies on ingress-only path. Secondary; does not fix duplicate when both channels accept.

5. **Seal-fail replay loses fast-path metadata** — `SealAbort` carries `Vec<SignedTx>` only; `prepend_block` requeues as Raw (`lifecycle.rs:1978`). Safe (full policy on retry), but loses precheck hint.

6. **`first_bad_tx_ctx` ignores `SealEntry` semantics** — eviction simulation always uses `apply_tx_with_ctx` (`lifecycle.rs:832-845`), not `apply_prechecked_tx`. May mis-rank failing index for mixed batches (secondary to duplicate issue).

## 5. Tests

| Test | Covers |
|------|--------|
| `seal_fresh_prechecked_skip_policy` | fresh `PreValidated` applies transfer under `SenderFilter` bit policy |
| `seal_stale_prechecked_uses_policy` | stale height re-runs policy, seal aborts |
| `test_worker_client_tx` | worker emits `validated_at_height` |

**Missing (blockers for slice closure):**

- Lifecycle/integration: HTTP tx appears once in seal batch (no duplicate PreValidated+Raw)
- Seal loop drain ordering under concurrent worker + ingress load
- `first_bad_tx_ctx` parity with `seal_entries` entry kinds
- End-to-end: zero `seal_skip` evictions for worker-admitted HTTP txs

## 6. Concurrency / parallelism

**Components:** OS-thread worker `validated_tx` send, tokio HTTP `tx_ingress` send, lifecycle `try_lock` on both receivers under `inner.write()`, `seal_entries` under same write lock.

| Hazard | Assessment |
|--------|------------|
| Dual-channel duplicate | **Bug** — systematic for HTTP non-roaming path (worker completes before ingress send) |
| `try_lock` on `_validated_rx` | **Latency only** — contended lock skips validated drain for one tick; no duplicate by itself |
| `tip_height` / snapshot vs `at_height` | **OK** — worker stores `snapshot_height`; fast-path compares to `tip_before` at seal |
| Cross-runtime channels | **Valid** — tokio `mpsc` for ingress + validated; drained synchronously inside seal write lock |

**Test gap:** no concurrent HTTP submit + seal-loop interleaving test; no dedup assertion on `entries` vec.

## 7. Findings (prioritized)

### Blocker

1. **Dual admission duplicates txs in seal batch** — after worker precheck succeeds, HTTP still sends the same tx to `tx_ingress` while worker also enqueues `ValidatedTx`. Lifecycle merges both into `seal_entries` in one tick. Fix: stop ingress enqueue after successful worker precheck **or** dedupe by `tx_hash` when building `entries` **or** drain validated into `g.pool` with a single take (per plan) and remove parallel ingress for worker-validated txs.

### High

2. **No lifecycle/integration test for validated drain** — pwm-core unit tests cover `SealEntry` logic; pwmd seal loop wiring untested end-to-end.

3. **`seal_skip` eviction regression** — duplicate nonce in batch triggers eviction path the sprint targets for elimination (`mvp_v7s2.md` acceptance: zero `seal_skip: evicting unapplicable tx`).

### Medium

4. **Plan merge path not implemented** — validated bypasses `g.pool`; ingress path still active for same txs.

5. **`first_bad_tx_ctx` / `SealAbort` flatten PreValidated** — replay and eviction simulation treat all txs as Raw policy path.

### Low

6. **Worker `evaluate_policy` at `snapshot_height` vs seal Raw at `inclusion_height`** — pre-existing Slice 2 nuance; fast-path skips re-check. Acceptable per plan false-accept tolerance; document for deferred-policy edge cases.

## 8. Verdict

**Request changes** — `SealEntry` / `seal_entries` / stale-vs-fresh logic in pwm-core is sound and well-tested, and lifecycle drain ordering (validated before `pool.take`) meets the literal acceptance item. Production HTTP path still feeds the same transaction through **both** validated and ingress channels, causing duplicate seal entries and `seal_skip` evictions.

**Before approve:** remove duplicate admission (ingress **or** validated, not both for worker-accepted txs) and add at least one integration test proving a single HTTP tx produces one seal entry.

## 9. Participation

- `agent`: `pwm-review`
- `result`: `FAIL`
- `artifacts`: `docs/reviews/20260626-v7-s2-slice3-validated-queue-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 42000, "confidence": "medium" }`