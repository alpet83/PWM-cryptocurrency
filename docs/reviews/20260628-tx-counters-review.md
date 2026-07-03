# Review: tx in-flight counters + seal loop fixes (72d1fdf)

- date: 2026-06-28
- ticket: `20260628-tx-counters-review`
- coding_ticket: `20260628-tx-inflight-counters`
- commit: `72d1fdf`

## 1. Scope recap

Review commit `72d1fdf`:

| area | change |
|------|--------|
| `pipeline/counters.rs` | NEW — `TX_COUNTER_{INCOMING,SEALED,REJECTED}` (`AtomicU64`, Relaxed) |
| `api/handlers_tx.rs` | `inc_incoming` / `inc_sealed` / `inc_rejected` on HTTP paths |
| `api/handlers_status.rs` | `tx_counters` in `/v1/status` |
| `lifecycle.rs` | `inc_sealed_by(N)` on seal OK; eviction `inc_rejected_by`; stale same-sender guard; `SEAL_POLL_INTERVAL_MS` 50→10; `seal_gate_profile` µs timings |

## 2. Requirements fit

| Focus area | Verdict | Evidence |
|------------|---------|----------|
| Counter coverage (no leaks/double-counts) | **PASS** with nits | Semantics are **monotonic observability**, not `incoming = sealed + rejected` (see §3) |
| `Relaxed` ordering | **PASS** | Sufficient for status-only monotonic counters (`counters.rs:19-42`) |
| `/v1/status` API contract | **PASS** | Additive JSON object; no fields removed (`types.rs:30`, `handlers_status.rs:253`) |
| nonce-eviction-guard | **PASS** | `lifecycle.rs:2057-2097` — `on_chain_nonce` default 0; scans `txs[i+1..]` same sender |
| `SEAL_POLL_INTERVAL_MS=10` | **PASS** with nits | Comment documents intent (`lifecycle.rs:58-59`); Windows 15 ms timer granularity |
| `cargo test` | **UNVERIFIED** | Shell unavailable |

## 3. Counter semantics (critical)

Counters are **not a conservation law**. Design:

| counter | when incremented |
|---------|------------------|
| `incoming` | Every `POST /v1/tx` entry (`handlers_tx.rs:32`) |
| `sealed` | Direct-path commit (`228/234`) or seal-loop batch size (`lifecycle.rs:1939`) |
| `rejected` | HTTP errors (`count_reject`, worker failures) + seal eviction drops (`2074-2076`) |

**Expected:** `incoming ≥ sealed + rejected` with **in-flight** gap (pipeline 204 before seal). One HTTP accept can later become `rejected` at seal without a second `incoming`. **No double `sealed`** on pipeline path (batch counts txs once).

### HTTP path coverage

| path | incoming | sealed | rejected |
|------|----------|--------|----------|
| Prefilter / validation / lock / shape errors | ✓ | | ✓ (`count_reject`) |
| Direct Export/Import/Claim seal OK | ✓ | ✓ | |
| Direct seal/snap failure | ✓ | | ✓ |
| Worker queue full / precheck fail / oneshot drop | ✓ | | ✓ |
| Pipeline 204 (queued) | ✓ | | (later seal or eviction) |
| Foreign relay early 204 | ✓ | | (terminal elsewhere) |

### Seal-loop coverage

| path | sealed | rejected |
|------|--------|----------|
| `Ok(seal)` | `inc_sealed_by(txs.len())` | |
| `tx:` eviction + stale guard | | `1 + stale_hashes.len()` |
| `tx:` but `first_bad_tx_ctx` None | | **none** (requeue full batch) |
| Non-`tx:` seal error | | **none** (requeue) |
| `next_apply_ctx` fail | | **none** (requeue, `continue`) |

## 4. Style and module shape

- `counters.rs` minimal, clear public API; module exported from `pipeline/mod.rs`.
- `count_reject` helper centralizes HTTP reject accounting.

### Wire JSON / u128

Wire JSON / u128: not applicable (status observability slice; `TxCounters` uses `u64` only).

## 5. Safety

1. **No panics on hot path** — `fetch_add` / `load` only.

2. **Torn snapshot** — `snapshot()` reads three atomics non-atomically; acceptable for ops metrics; document that ratios are approximate.

3. **stale_hashes guard** — `on_chain_nonce = 0` when sender missing (`2062-2063`). Only evicts same-sender txs **after** bad index with `nonce <= 0`. Risk: rare false stale if bad tx is nonce-0 on unknown sender and valid queued nonce-0 siblings exist — low at production scale.

4. **Evicted txs not in `replay`** — dropped from pool; `evicted_hashes` prevents re-admission; aligned with prior eviction design.

## 6. Tests

| exists | gap |
|--------|-----|
| `eviction_skip_set`, `dedup_seal_entries_removes_dup` | No test for **stale same-sender** eviction branch |
| `v1_stat_default_lane_ns` checks `pipeline_metrics` | **No assertion on `tx_counters`** |
| No `counters.rs` unit tests | |

## 7. Concurrency / parallelism

**Components:** three process-wide `static AtomicU64`; HTTP tokio tasks + OS worker threads + seal tokio task all `fetch_add(Relaxed)`.

| Hazard | Assessment |
|--------|------------|
| Data races on counters | **Safe** — atomics |
| Relaxed ordering | **OK** for monotonic stats; no acquire/release pairing needed |
| `SEAL_POLL_INTERVAL_MS=10` microcycle | **Low** — on seal `Err`, deadline += 10 ms; `turn_watch` at >20 cycles still warns; Windows may effective-sleep ≥15 ms reducing spin |
| Gate timing | **OK** — `lease_gate_us` / `cluster_gate_us` measured separately (`1764-1835`); `seal_gate_profile` logs µs + ms |

## 8. Findings (prioritized)

### Medium

1. **Counter semantics undocumented in API** — clients may assume `incoming == sealed + rejected`. Recommend one-line doc on `TxCounters` or `/v1/status` operator docs: in-flight gap expected.

2. **Eviction paths without `inc_rejected`** — `first_bad_tx_ctx` miss and non-`tx:` seal errors requeue without reject increment; evicted bad tx counted only when `drop_at` is `Some`.

3. **No integration test for `tx_counters`** — status test should assert zeros at boot and delta after `POST /v1/tx`.

### Low

4. **`seal_gate_profile` only on proposer** — attester skips log; intentional.

5. **`inc_sealed_by` on empty blocks** — zero increment; fine.

6. **Foreign relay 204** — `incoming` without local `sealed`/`rejected`; acceptable if metric is HTTP ingress only.

## 9. Verdict

**Approve with nits** — counter wiring is consistent for HTTP ingress / seal batch / explicit rejects and eviction drops; `Relaxed` atomics appropriate; API change is backward-compatible additive JSON; stale-sender guard is logically sound with `on_chain_nonce=0` default; 10 ms poll is reasonable with documented trade-off. Document in-flight semantics and add status/counter tests in follow-up.

## 10. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260628-tx-counters-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 38000, "confidence": "medium" }`