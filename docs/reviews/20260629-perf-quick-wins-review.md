# Review: perf quick-wins — log, validate dedup, static checkpoints (2c6be19)

- date: 2026-06-29
- ticket: `20260629-perf-quick-wins-review`
- coding_ticket: `20260629-perf-quick-wins`
- commit: `2c6be19`
- prior analysis: `docs/reviews/20260629-flamegraph-json-hotspots-review.md`

## 1. Scope recap

Review commit `2c6be19` — three surgical fixes:

| # | change | files |
|---|--------|-------|
| 1 | `log_tx_commit_delta` INFO → guarded `debug!` | `lifecycle.rs:881-912` |
| 2 | Remove duplicate `validate_tx_shape` from RPC handler | `handlers_tx.rs`, `worker.rs:342` |
| 3 | `ProfileTime` checkpoint keys `&'static str` | `block_timing.rs:114-145`, `lifecycle.rs` call sites |

## 2. Requirements fit

| Focus area | Verdict | Evidence |
|------------|---------|----------|
| `log_tx_commit_delta` lazy eval | **PASS** | `tracing::enabled!(DEBUG)` early return (`:882-885`); `hex::encode` only inside loop after guard (`:887-909`) |
| Handler `validate_tx_shape` removed | **PASS** | No match in `handlers_tx.rs`; worker retains check (`worker.rs:342-344`) |
| Malformed JSON body | **PASS** | Axum `Json<SignedTx>` extractor unchanged (`handlers_tx.rs:48`) — serde deserialize errors before handler |
| Static checkpoint keys | **PASS** | `BTreeMap<&'static str, _>` (`block_timing.rs:114-115`); `checkpoint*` takes `&'static str`; lifecycle literals e.g. `"lease_gate_begin"` |
| Behavioral regression | **PASS** with nit | Pipeline path preserves `BAD_REQUEST` + `tx_reject_json` via `worker_reject_*` (`handlers_tx.rs:27-44`, `:275-280`) |
| Tests | **UNVERIFIED** | Shell unavailable; static analysis below |

## 3. Change-by-change analysis

### 1. `log_tx_commit_delta` (`lifecycle.rs:881-912`)

```881:885:crates/pwmd/src/lifecycle.rs
fn log_tx_commit_delta(before: &State, after: &State, height: u64, txs: &[SignedTx]) {
    if !tracing::enabled!(tracing::Level::DEBUG) {
        let _ = height;
        return;
    }
```

- **No `info!`** — downgraded to `debug!` (`:900-909`).
- **No eager `hex::encode`** when DEBUG off — correct hot-path win for ramp (default INFO).
- `height` kept in signature with `let _ = height` on both branches — symmetry preserved.

### 2. Duplicate `validate_tx_shape` removal

**Pipeline path (Transfer / Burn / etc.):**

- Handler calls `run_worker_precheck` → worker `precheck_client` → `validate_tx_shape` once.
- New helpers preserve prior HTTP contract for shape errors:

```27:44:crates/pwmd/src/api/handlers_tx.rs
fn worker_reject_status(reason: &TxRejectReason) -> StatusCode {
    match reason {
        TxRejectReason::ShapeInvalid(_) => StatusCode::BAD_REQUEST,
        _ => StatusCode::UNPROCESSABLE_ENTITY,
    }
}

fn worker_reject_msg(tx: &SignedTx, reason: &TxRejectReason) -> String {
    match reason {
        TxRejectReason::ShapeInvalid(detail) => tx_reject_json(
            tx,
            "preflight",
            detail,
            format!("tx validation failed: {detail}"),
        ),
        _ => reason.to_string(),
    }
}
```

- `TxRejectReason::ShapeInvalid(TxError)` from `worker.rs:344` feeds same `tx_reject_json` shape as former handler path.
- Tests like `v1_tx_burn_purpose_bad` (`http_status.rs:937-962`) use pipeline path — should remain `BAD_REQUEST` + `phase=preflight`.

**Direct-seal path (Export / Import / ClaimIPv4Batch):**

- Handler **no longer** calls `validate_tx_shape` before `g.chain.seal` (`handlers_tx.rs:100-140`).
- Shape/signature failures now surface as `500 INTERNAL_SERVER_ERROR` with `seal after roaming tx failed: {msg}` instead of `400` + `tx_reject_json`.
- **Mitigation:** Export readiness (`handlers_roaming.rs:47`) still runs `validate_tx_shape` before export tx submission in normal flows.
- **Gap:** Import / ClaimIPv4Batch malformed shape bypasses preflight JSON contract — **nit REGRESS-1** (low traffic vs ramp Transfer path).

### 3. Static checkpoint keys (`block_timing.rs`)

```114:145:crates/pwmd/src/block_timing.rs
    checkpoints_abs_ms: BTreeMap<&'static str, u64>,
    checkpoints_rel_ms: BTreeMap<&'static str, f64>,
    ...
    pub(crate) fn checkpoint(&mut self, name: &'static str) {
    ...
    pub(crate) fn checkpoint_at(&mut self, name: &'static str, timestamp_ms: u64) {
        ...
        self.checkpoints_abs_ms.insert(name, timestamp_ms);
        ...
        self.checkpoints_rel_ms.insert(name, rel_ms);
```

- Poll hot path (`lifecycle.rs:1776-1917`) passes string literals — **no per-poll `String::from`**.
- `json_stats_with_precision` still allocates `(*name).to_string()` when building seal profile JSON (`:166-174`) — acceptable; runs only on non-empty seal + timing enabled (not every 10ms poll).
- Existing test `json_stats_merge_schema` uses static checkpoint names (`block_timing.rs:1128-1138`) — compatible.

## 4. Style and module shape

- Minimal diff; no new abstractions.
- `worker_reject_status` / `worker_reject_msg` are appropriately local to `handlers_tx.rs`.

### Wire JSON / u128

Wire JSON / u128: not applicable (no wire/RFC contract change in this slice).

## 5. Safety

- No new panics or unwraps on hot paths.
- Log downgrade is observability-only — no consensus impact.

## 6. Tests

| area | assessment |
|------|------------|
| Pipeline shape reject (`assert_preflight_apply_parity`) | Should pass — worker path + `worker_reject_msg` |
| Direct-seal shape reject | **No dedicated test**; behavior change possible (500 vs 400) |
| `block_timing::json_stats_merge_schema` | Unchanged semantics |
| `cargo test` | **UNVERIFIED** |

## 7. Concurrency / parallelism

No new shared state. Removing handler-level `validate_tx_shape` moves crypto work to worker thread — better for RPC task latency under parallel submits. Checkpoint map uses `&'static str` keys — no cross-thread issue (seal loop single task).

## 8. Verdict

**Approve with nits**

Prioritized nits:

1. **REGRESS-1 (medium, direct-seal only):** Re-add `validate_tx_shape` (or shared helper) on Export/Import/Claim branch before `chain.seal`, **or** document that only pipeline txs rely on `400`+`tx_reject_json` and direct-seal errors are `500`.
2. **TEST-1 (low):** Run `cargo test -p pwmd` especially `http_status`, `http_export`, `block_timing`.
3. **TEST-2 (low):** Add test: invalid-shape Transfer → `400` + `phase=preflight` via worker (regression lock for quick-win).

Hot-path goals from flamegraph review are met for ramp Transfer workload.

## 9. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260629-perf-quick-wins-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 22000, "confidence": "medium" }`