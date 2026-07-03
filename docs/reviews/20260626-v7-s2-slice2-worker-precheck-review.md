# V7-S2 Slice 2 — worker full precheck (pwm-review)

- date: 2026-06-26
- ticket: `20260626-v7-s2-slice2-worker-precheck-review`
- commit: `5bf0d26` (branch `mvp-v7`)
- normative: `docs/plans/mvp_v7s2.md` § Slice 2, `docs/adr/0013-tx-pipeline-seda.md`

## 1. Scope recap

Slice 2 moves tip-aware precheck off the HTTP read-lock path into OS-thread workers:

| file | change |
|------|--------|
| `pipeline/worker.rs` | `WorkerCtx`, `precheck_client`, `ValidatedTx` output, reject mapping |
| `pipeline/queue.rs` | `TxRejectReason`, `ValidatedTx`, `ClientTxJob` reply type |
| `pipeline/mod.rs` | re-exports |
| `api/handlers_tx.rs` | remove `precheck_apply_with_ctx` from non-roaming path |
| `tests/http_status.rs` | `v1_tx_underfunded_xfer_mempool` — ingress accepts underfunded tx |

## 2. Requirements fit

| Acceptance criterion | Verdict | Evidence |
|---------------------|---------|----------|
| `WorkerPool` wired to `WorkerCtx` in **production** | **FAIL** | `WorkerPool` / `dispatch` / `ClientTxJob` only in `pipeline/*` tests; `lifecycle.rs` still drains `tx_ingress` only; `handlers_tx.rs:219` `try_send` bypasses workers |
| `precheck_client`: shape → policy → precheck | **PASS** | `worker.rs:300-318` |
| `TxRejectReason` maps `TxError` variants | **PARTIAL** | `precheck_err` (`worker.rs:328-333`): `BadNonce`/`DuplicateImport`/`AlreadyInit` → `StaleDuplicate`; policy → `PolicyDenied`; rest → `PrecheckFailed(String)` — no dedicated `Insufficient` etc. |
| `handlers_tx`: precheck removed non-roaming; roaming untouched | **PASS** | Roaming `Export/Import/ClaimIPv4` still `write` + `chain.seal` (`handlers_tx.rs:75-216`); `_ =>` branch ingress-only (`218-241`); no `precheck_apply` in file |
| Unit tests valid + invalid path | **PASS** | `test_worker_client_tx`, `test_worker_rejects_bad_tx` (`worker.rs:414-518`) |
| `cargo test -p pwmd -p pwm-core` PASS | **UNVERIFIED** | Shell unavailable in review session |

**Slice 2 post-condition gap:** worker precheck is implemented and tested in isolation, but **live RPC traffic does not traverse it**. Removing HTTP precheck without production worker wire-up creates an admission gap: invalid txs (e.g. underfunded transfer per `v1_tx_underfunded_xfer_mempool`) enqueue to `tx_ingress` and reach seal unchanged.

## 3. Style and module shape

- `WorkerCtx`, `precheck_client`, `precheck_err` — ≤4-word identifiers ✓
- `ValidatedTx` / `TxRejectReason` align with plan §Slice 2–3 staging
- No new façade bloat; worker module stays cohesive

Entity segment check not run (shell unavailable).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

1. **`tip_height` never updated** — `WorkerCtx.tip_height: Arc<AtomicU64>` read in `snapshot_height()` (`worker.rs:115-117`); no `store` in `lifecycle.rs` after seal. Production workers would run policy/precheck at height **0** regardless of chain tip.

2. **`validated_tx` channel has no consumer** in production — worker `try_send` succeeds in tests; no lifecycle drain of `ValidatedTx` yet (Slice 3 scope, but blocks meaningful production wire-up now).

3. **Behavioral regression (interim):** CHANGELOG historically documented HTTP `precheck_apply_tip` → **409** for underfunded txs; new test expects **204** + ingress queue. Intentional only if workers immediately reject; without wire-up, seal still sees bad txs.

4. **Duplicate `validate_tx_shape`** — HTTP (`handlers_tx.rs:60`) + worker (`precheck_client:301`) when path is connected; harmless but redundant.

## 5. Tests

| Test | Covers |
|------|--------|
| `test_worker_client_tx` | full precheck OK + `ValidatedTx` on channel |
| `test_worker_rejects_bad_tx` | bad nonce → `StaleDuplicate` |
| `v1_tx_underfunded_xfer_mempool` | HTTP no precheck → ingress (documents removal) |

**Missing blockers for slice closure:**

- Production smoke: HTTP → dispatch → worker → reply / validated queue
- `tip_height` update after seal reflected in worker precheck
- HTTP 409 parity test when worker rejects (underfunded → reject, not ingress)

## 6. Concurrency / parallelism

**Components:** `StateSnapshot::load()` in OS threads, `WorkerCtx.tip_height` (`Relaxed`), `tokio::mpsc` `validated_tx` from worker threads, HTTP `tx_ingress` on async runtime.

| Hazard | Assessment |
|--------|------------|
| Snapshot stale read | **Expected** per plan §Slice 2 nuance; `Arc<State>` immutability safe |
| `tip_height` Relaxed without seal publish | **Bug** if workers run — policy height diverges from snapshot state |
| `precheck_apply_with_ctx` clones `State` per tx | CPU cost O(accounts); acceptable for slice 2, not a lock hazard |
| Cross-runtime `validated_tx` send | Valid pattern; consumer must be tokio task (not yet wired) |

**Test gap:** no concurrent seal + worker `load()` freshness test.

## 7. Findings (prioritized)

### Blocker

1. **Production path not connected** — criterion «WorkerPool подключён к WorkerCtx в продакшн пути» не выполнен. Need: start `WorkerPool` in `lifecycle`/`run`, HTTP non-roaming → `dispatch(ClientTxJob)` + oneshot reply (or bridge ingress → worker), feed `WorkerCtx` from `app.state_snapshot`, `chain.cfg`, live tip.

### High

2. **`tip_height` atomic never updated on seal** — wire `app.state_snapshot` + `tip_height.store(g.chain.tip_h())` after each successful seal (or derive height from snapshot metadata).

### Medium

3. **`ValidatedTx` output orphaned** — worker sends to channel with no drain; blocks end-to-end slice 2 value until Slice 3, but should be noted in handoff.

4. **`TxRejectReason` mapping coarse** — `PolicyDecision::Reject(TxError::*)` all become `PolicyDenied`; `Insufficient` → `PrecheckFailed` string. OK for scaffold; document HTTP JSON parity when wiring replies.

### Low

5. **`precheck_client` could use `precheck_apply_tip`** instead of manual `saturating_add(1)` — equivalent to current logic.

## 8. Verdict

**Request changes** — worker precheck implementation and unit tests are sound, but **production integration is missing** (explicit acceptance criterion). Shipping HTTP precheck removal without live worker path regresses admission behavior (underfunded txs enter ingress).

**Before approve:** wire `WorkerPool` + `WorkerCtx` on daemon start; route non-roaming HTTP through worker dispatch; update `tip_height` after seal; add at least one HTTP-level reject test when worker returns `TxRejectReason`.

## 9. Participation

- `agent`: `pwm-review`
- `result`: `FAIL`
- `artifacts`: `docs/reviews/20260626-v7-s2-slice2-worker-precheck-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 42000, "confidence": "medium" }`