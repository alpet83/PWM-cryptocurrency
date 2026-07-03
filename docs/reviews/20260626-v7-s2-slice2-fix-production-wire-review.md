# V7-S2 Slice 2 fix — production worker wire + tip_height (pwm-review)

- date: 2026-06-26
- ticket: `20260626-v7-s2-slice2-fix-production-wire-review`
- commit: `3541450` (branch `mvp-v7`)
- normative: `docs/plans/mvp_v7s2.md` § Slice 2, `docs/adr/0013-tx-pipeline-seda.md`
- prior review: `docs/reviews/20260626-v7-s2-slice2-worker-precheck-review.md` (FAIL — production not connected)

## 1. Scope recap

Corrective fix for Slice 2 blockers: wire `WorkerPool`/`WorkerCtx` into daemon bootstrap, publish `tip_height` after seal, route non-roaming HTTP through worker dispatch + oneshot reply, and restore HTTP-level reject coverage.

| file | change |
|------|--------|
| `bootstrap.rs` | `worker_parts()` helper; `WorkerPool` + queues on all `App` paths |
| `state.rs` | `worker_queues`, `worker_tip_height`, `_worker_pool`, `_validated_rx` on `App` |
| `lifecycle.rs` | `worker_tip_height.store(h, Relaxed)` after successful main-loop seal |
| `api/handlers_tx.rs` | `run_worker_precheck()` — dispatch `ClientTxJob`, await oneshot; Err→422, Ok→ingress |
| `tests/http_status.rs` | `v1_stat_snap_tx_nl` — 16 unique sender/peer pairs; `v1_tx_underfunded_xfer_mempool` expects 422 |

## 2. Requirements fit

| Acceptance criterion | Verdict | Evidence |
|---------------------|---------|----------|
| `bootstrap.rs`: `WorkerPool`+`WorkerCtx` at daemon start | **PASS** | `worker_parts()` (`bootstrap.rs:48-67`) builds `DispatchQueues`, `WorkerCtx`, `WorkerPool::new(1,1,…)`; wired in `app_from_chain_boot` (`:113-124`), snapshot-load branch (`:251-269`), genesis fallback (`:334-352`) |
| `lifecycle.rs`: `tip_height.store` after each successful seal | **PASS** | `lifecycle.rs:1832-1834` — `app.worker_tip_height.store(h, Relaxed)` inside `Ok(())` after `state_snapshot.store` |
| `handlers_tx.rs`: non-roaming → worker channel + oneshot; Err→422, Ok→ingress | **PASS** | `_ =>` branch (`handlers_tx.rs:220-244`) calls `run_worker_precheck` then `tx_ingress.try_send`; `run_worker_precheck` (`:249-268`) dispatches `ClientTxJob`, maps `Ok(Err(reason))` → `UNPROCESSABLE_ENTITY` |
| Roaming paths untouched | **PASS** | `Export`/`Import`/`ClaimIPv4Batch` still direct `write` + `chain.seal` (`handlers_tx.rs:77-218`); no `run_worker_precheck` on that arm |
| `v1_stat_snap_tx_nl` uses 16 unique txs | **PASS** | `http_status.rs:532-540` — 16 pairs with distinct `sender_seed`/`peer_seed`; nonce 1 transfers, no repeated stale nonce |
| All prior review blockers closed | **PASS** | Production wire + `tip_height` publish + HTTP reject test address blockers #1–#2 and missing HTTP reject coverage from prior review §7 |

**Slice 2 interim state (expected):** `ValidatedTx` output is still not drained by lifecycle (Slice 3). Non-roaming admission is now worker-gated before ingress; seal loop still drains raw `tx_ingress` only.

## 3. Style and module shape

- `worker_parts`, `run_worker_precheck`, `WorkerParts` — ≤4-word production identifiers ✓
- Bootstrap duplication for worker init across three `App` constructors is repetitive but matches existing snapshot-init pattern; no new façade bloat
- `handlers_tx.rs` keeps roaming cancellation contract comment; worker path is a small helper — good separation

Entity segment check not run (shell unavailable).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

1. **Worker pool lifetime** — `_worker_pool: Arc<WorkerPool>` on `App` keeps OS-thread workers alive for process lifetime ✓

2. **Backpressure** — full client queue → `INSUFFICIENT_STORAGE` (`handlers_tx.rs:255-259`); full ingress → separate 507 path (`:222-226`)

3. **Dropped worker / canceled oneshot** — `rx.await` `Err(_)` → `SERVICE_UNAVAILABLE` (`:264-267`); avoids silent accept

4. **Roaming direct-seal does not update `worker_tip_height` / `state_snapshot`** — only main lifecycle seal publishes both (`lifecycle.rs:1827-1834`). Roaming HTTP seals (`handlers_tx.rs:110`) can advance chain tip without refreshing worker view until next lifecycle tick. Acceptable for roaming-only traffic; **nit** if mixed roaming + transfer load in same process without lifecycle seal between them.

5. **`ValidatedTx` try_send in worker** — `handle_client` ignores send failure (`worker.rs:295`); channel full would still return `Ok(())` to HTTP while dropping validated output. Harmless until Slice 3 wires consumer; note for handoff.

## 5. Tests

| Test | Covers |
|------|--------|
| `test_worker_client_tx`, `test_worker_rejects_bad_tx`, etc. | worker unit paths (unchanged, still present) |
| `v1_tx_underfunded_xfer_mempool` | HTTP → worker reject → 422; mempool and ingress empty (`http_status.rs:709-715`) |
| `v1_stat_snap_tx_nl` | concurrent `/v1/status` + `/v1/tx` with worker pool live; 16 unique accounts (`:530-618`) |

**Gaps (non-blocking for this fix ticket):**

- No explicit test that `worker_tip_height` tracks lifecycle seal (only indirect via concurrent status+tx smoke)
- No HTTP test for `StaleDuplicate` / `PolicyDenied` distinct status bodies
- `cargo test -p pwmd -p pwm-core` **UNVERIFIED** — shell unavailable in review session

## 6. Concurrency / parallelism

**Components:** `WorkerPool` OS threads (`blocking_recv` on `Mutex<Receiver>`), tokio HTTP handlers (`run_worker_precheck` + `oneshot`), `StateSnapshot` (`std::sync::RwLock`), `worker_tip_height` (`AtomicU64` Relaxed), `tx_ingress` tokio `mpsc`.

| Hazard | Assessment |
|--------|------------|
| Lock held across worker precheck | **None** — HTTP releases `inner` read lock before `run_worker_precheck`; worker uses `StateSnapshot::load()` |
| `tip_height` vs snapshot freshness | **Mitigated** — both updated in same lifecycle `Ok(seal)` block before next worker reads |
| Cross-runtime oneshot HTTP↔worker | **Valid** — standard pattern; worker sends reply from OS thread |
| Roaming seal vs worker snapshot | **Residual** — roaming HTTP seal bypasses snapshot/`tip_height` publish (see Safety §4) |
| Worker queue mutex + async dispatch | **OK** — `try_push` only; no await inside worker receiver lock |

**Test gap:** no interleaving test (lifecycle seal mid-flight HTTP precheck) for `tip_height`/`snapshot` ordering.

## 7. Findings (prioritized)

### Blocker

None — prior Slice 2 production-wire blockers are closed.

### Medium

1. **`ValidatedTx` channel still orphaned** — `_validated_rx` stored on `App` but not drained; Slice 3 scope. Documented for orchestrator handoff.

2. **Roaming direct-seal snapshot gap** — workers may run policy/precheck against pre-roaming-seal `tip_height` until lifecycle seal runs. Low probability in current harnesses; worth addressing when roaming + ingress mix under load.

### Low

3. **HTTP 422 body** — returns `TxRejectReason` `Display` string, not structured `tx_reject_json` used elsewhere. Functional; parity polish later.

4. **Duplicate `validate_tx_shape`** — HTTP (`handlers_tx.rs:62`) and worker (`worker.rs:304`) when path connected; redundant CPU only.

5. **`worker_parts` hard-coded `WorkerPool::new(1, 1, …)`** — fine for slice fix; config surface deferred.

## 8. Verdict

**Approve with nits** — corrective commit satisfies all ticket acceptance criteria and closes FAIL items from `20260626-v7-s2-slice2-worker-precheck-review.md`. Production non-roaming admission now traverses worker precheck; `tip_height` publishes on lifecycle seal; HTTP reject regression test restored.

Remaining nits are Slice 3 handoff (`ValidatedTx` drain) and roaming/snapshot consistency — not regressions introduced by this fix.

## 9. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260626-v7-s2-slice2-fix-production-wire-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 38000, "confidence": "medium" }`