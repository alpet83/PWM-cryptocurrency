# Review: perf batch2 — seal digest gate + Arc&lt;SignedTx&gt; RPC (da1a108)

- date: 2026-06-29
- ticket: `20260629-perf-seal-arc-review`
- coding_tickets: `20260629-perf-seal-digest`, `20260629-perf-rpc-arc-tx`
- commits: `a319f89` (seal `st_before` gate + digest annotation), `da1a108` (Arc&lt;SignedTx&gt; RPC path)
- HEAD reviewed: `da1a108818bb2bfbd4dc0ac055f3f8fe97b8b61f`
- prior analysis: `docs/reviews/20260629-flamegraph-round2-review.md`
- scope: `lifecycle.rs` seal path, `chain.rs` digest, `handlers_tx.rs`, `pipeline/queue.rs`, `pipeline/worker.rs`, `pipeline/dispatch.rs`

## 1. Scope recap

Round-2 flamegraph review ranked **seal `st_before` clone** and **RPC `tx.clone()`** as top sprint items. This batch delivers:

| Commit | Claim | Files |
|--------|-------|-------|
| `a319f89` | Gate unconditional `st_before` clone; document `digest(st)` bincode cost | `lifecycle.rs`, `pwm-core/chain.rs` |
| `da1a108` | Parse `SignedTx` once in handler; pass `Arc<SignedTx>` to worker queue | `handlers_tx.rs`, `pipeline/queue.rs`, `pipeline/worker.rs`, `pipeline/dispatch.rs`, tests |

## 2. Requirements fit

| Focus area | Verdict | Evidence |
|------------|---------|----------|
| 1. `st_before` gate correctness | **PASS** | `tracing::enabled!(Level::DEBUG).then(|| g.chain.st.clone())` (`lifecycle.rs:1907`); consumers gated (`:1968-1970`) |
| 2. Covers all formerly-unconditional clone cases for debug delta | **PASS** with nit | Only debug-delta clone was unconditional; `sealed_state` clone (`:1918`) remains intentional for `state_snapshot.store` |
| 3. `digest(st)` annotation | **PASS** (defer impl) | `chain.rs:220-222` PERF comment; incremental redesign correctly deferred |
| 4. `Arc<SignedTx>` single-parse path | **PASS** | `Json<SignedTx>` once (`handlers_tx.rs:49`); `Arc::new(tx)` pipeline branch (`:261`); `ClientTxJob { tx: Arc<SignedTx> }` (`queue.rs:15-16`); worker `job.tx.as_ref()` (`worker.rs:318`) |
| 5. Error handling unchanged | **PASS** | Axum JSON errors pre-handler; `worker_reject_msg(tx.as_ref(), …)` (`handlers_tx.rs:292`); status mapping via `worker_reject_*` unchanged |
| 6. `cargo check` + `cargo test -p pwmd --lib` | **PARTIAL** | `cargo check -p pwmd`: **PASS** (Windows `cargo.exe`, exit 0). `cargo test -p pwmd --lib`: **510 passed, 1 failed** — known flake `v1_tx_event_sealed` (`issues-report.md`, `http_status.rs:860-882`) |

## 3. Change-by-change analysis

### `a319f89` — seal `st_before` gate

```1907:1971:crates/pwmd/src/lifecycle.rs
            let st_before = tracing::enabled!(tracing::Level::DEBUG).then(|| g.chain.st.clone());
            // ...
                        if let Some(st_before) = st_before.as_ref() {
                            log_tx_debug(st_before, &g.chain.st, h, &txs);
                            log_tx_commit_delta(st_before, &g.chain.st, h, &txs);
                        }
```

- Condition matches `log_tx_commit_delta` internal guard (`:882-885`) — no behavioral skew when DEBUG off.
- `log_tx_debug` is now correctly gated too (previously ran with unconditional `st_before`).
- **Not gated (by design):** `sealed_state = Arc::new(g.chain.st.clone())` (`:1918`) — required for `StateSnapshot` / worker reads; separate follow-up from round-2 item 1.
- **Still on hot path:** `chain.seal_entries` internal `st = self.st.clone()` (`pwm-core/chain.rs:181`) and `digest(&st)` (`:220-222`).

### `a319f89` — digest annotation

```220:222:crates/pwm-core/src/chain.rs
        // PERF: digest(&State) serializes the full state and still runs on the seal critical path;
        // a future state-root redesign should make this incremental or off-path.
        let state_root = digest(&st);
```

- Annotation is accurate: `digest` = `bincode::serialize(st)` + blake3 (`state.rs:160-162`).
- **Sufficient for this slice** — documents the remaining seal hotspot without risking consensus semantics. Implementation belongs in a dedicated state-root ticket.

### `da1a108` — `Arc<SignedTx>` RPC path

```259:271:crates/pwmd/src/api/handlers_tx.rs
        _ => {
            let tx_id = tx_id_hex(&tx);
            run_worker_precheck(&a, Arc::new(tx)).await?;
            // ...
        }

async fn run_worker_precheck(a: &App, tx: Arc<SignedTx>) -> Result<(), (StatusCode, String)> {
    let job = ClientTxJob::new(Arc::clone(&tx), reply);
```

- **No double JSON parse:** Axum `Json` extractor deserializes once; owned `SignedTx` moves into `Arc`.
- **No extra handler→worker `SignedTx` clone:** replaces prior `ClientTxJob::new(tx.clone(), …)`.
- `tx_id_hex(&tx)` captured before `Arc::new(tx)` — logging preserved.
- Direct-seal branch still uses `tx.clone()` for `g.chain.seal` (`:147`) — out of scope; roaming/import path only.

Worker acceptance still clones once into `ValidatedTx`:

```419:423:crates/pwmd/src/pipeline/worker.rs
fn validated_tx(tx: &SignedTx, snapshot_height: u64) -> ValidatedTx {
    ValidatedTx {
        tx: tx.clone(),
        validated_at_height: snapshot_height,
    }
}
```

This is expected until `ValidatedTx` also carries `Arc<SignedTx>` — nit, not a blocker for this slice.

Tests/dispatch helpers updated to `Arc::new(test_tx(...))` (`dispatch.rs:102-104`, `worker.rs:533-535`, `determinism.rs:129`).

## 4. Style and module shape

- Identifiers within policy (`st_before`, `run_worker_precheck`, `ClientTxJob`).
- `use std::sync::Arc` added only where needed.
- English PERF comment on seal path — appropriate.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 5. Safety

- No `unsafe`; no new `unwrap` on error paths in touched code.
- `Arc<SignedTx>` is immutable sharing — no interior mutability hazard.
- Trust boundary unchanged: malformed JSON rejected by Axum before handler logic.

## 6. Tests

| Area | Coverage |
|------|----------|
| `dispatch` unit tests | Updated for `Arc<SignedTx>` (`dispatch.rs:102-114`) |
| Worker unit tests | `ClientTxJob::new(Arc::new(tx), …)` (`worker.rs:533-535`) |
| HTTP pipeline | `v1_tx_worker_pipeline` paths in `http_status.rs` (static) |
| Full lib suite | 510/511 pass at `da1a108`; `v1_tx_event_sealed` timeout — **pre-existing flake**, not introduced by Arc change (same test passed in isolation per `issues-report.md`) |

**Gap:** no micro-benchmark proving clone elimination; acceptable for perf slice with static + compile proof.

## 7. Concurrency / parallelism

Components: RPC handler task → `BoundedQueue<ClientTxJob>` → worker threads → `validated_tx` mpsc.

- `Arc<SignedTx>`: `Send + Sync` shared immutably across threads — idiomatic, no data races.
- `Arc::clone` on dispatch is refcount-only — cheap vs `SignedTx` deep clone.
- No locks held across `.await` with shared tx payload.
- Channel semantics unchanged; backpressure via bounded queue preserved.

No new deadlock/race surfaces observed in this diff.

## 8. BLOCKERs

None.

## 9. Nits (non-blocking)

1. **NIT-1:** `validated_tx` still deep-clones `SignedTx` at worker accept (`worker.rs:421`) — follow-up `Arc` through `ValidatedTx` for full ingress win.
2. **NIT-2:** `sealed_state` full-state clone every successful seal (`lifecycle.rs:1918`) — round-2 follow-up; consider reusing post-seal `Arc` without second clone.
3. **NIT-3:** Mirror PERF note on `state::digest()` doc comment (`state.rs:159-162`) for discoverability.
4. **NIT-4:** `v1_tx_event_sealed` flake under full lib suite — unrelated; already in `issues-report.md`.

## 10. Verdict

**Approve with nits** — both commits satisfy round-2 sprint goals: debug-only `st_before` clone, digest hotspot documented, single-parse `Arc` RPC→worker path without behavior regression. `cargo check` clean; lib tests effectively green modulo known flake.

## 11. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260629-perf-seal-arc-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 42000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260629-perf-seal-arc-review.md'
git commit -m 'docs(perf): seal st_before gate and Arc SignedTx RPC review (da1a108)'
```