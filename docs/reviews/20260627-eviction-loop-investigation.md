# V7-S3 — eviction cascade investigation (lifecycle.rs / Mpool)

- date: 2026-06-27
- ticket: `20260627-eviction-loop-investigation`
- coding_ticket: `20260627-v7s3-fs-perf-hex-async-append`
- normative context: `docs/tickets/grok-eviction-loop-investigation.md`, `docs/reviews/v7-s2-ramp-results.md`

## 1. Scope recap

Investigation ticket (not a code slice): explain node-side **eviction busy-loop** during ramp soak DDoS (`head_stall` at level≈40), duplicate `tx_id` in `accepted: queued via worker`, why mempool stays at 63–64 txs, and propose a minimal fix preserving tx-recovery semantics.

Focus files:

| file | role |
|------|------|
| `crates/pwmd/src/lifecycle.rs:1813-1993` | seal batch assembly, eviction, `prepend_block` |
| `crates/pwm-core/src/mempool.rs:49-54` | `prepend_block` — no dedup |
| `crates/pwm-core/src/chain.rs:170-199` | `seal_entries` atomic abort |
| `crates/pwmd/src/pipeline/worker.rs:314-336` | validated enqueue path |

Log target `logs/2026-06-27/pwmd-cy-proposer-123813.log` — **not present in workspace**; analysis uses embedded excerpts from `docs/tickets/grok-eviction-loop-investigation.md` and prior review `20260625-v7-s1-log-timing-report.md`.

## 2. Requirements fit

| Criterion | Verdict | Evidence |
|-----------|---------|----------|
| Explain duplicate `tx_id` in pool / logs | **PASS** | See §3.1 — duplicates originate **before** seal; `prepend_block` does **not** re-run workers |
| Explain pool stuck at 63–64 | **PASS** | See §3.2 — cap-64 batch + one eviction/tick + ingress refill + deadline not advanced |
| Propose fix A/B/C with pseudo-diff | **PASS** | See §5 — recommend **C′ + dedup** (superset of ticket options) |
| Log pattern analysis | **PARTIAL** | Raw logs missing; pattern confirmed from ticket + prior soak reports |

## 3. Root-cause analysis

### 3.1 Duplicate `tx_id` — not from `prepend_block` revalidation

**Mechanism (seal batch assembly):**

```1818:1839:crates/pwmd/src/lifecycle.rs
let block_cap = 64usize;
let mut entries = Vec::with_capacity(block_cap);
// 1) drain validated_rx → PreValidated (up to 64)
// 2) pool.take(remaining) → Raw
entries.extend(g.pool.take(remaining).into_iter().map(SealEntry::Raw));
```

**`prepend_block` path:** `SealAbort` carries `Vec<SignedTx>` only (`chain.rs:171-177`). On eviction, kept txs are requeued as **Raw** via `Mpool::prepend_block` — they re-enter only through `pool.take`, **not** `validated_rx` / workers.

```49:54:crates/pwm-core/src/mempool.rs
pub fn prepend_block(&mut self, txs: Vec<SignedTx>) {
    for tx in txs.into_iter().rev() {
        self.q.push_front(tx);
    }
}
```

Therefore **two `accepted: queued via worker` lines for the same `tx_id`** mean the tx was admitted by HTTP/workers **twice** before any seal eviction — not caused by requeue.

**Confirmed sources of duplicate admission:**

1. **Benchmark bug (pre-rotation-fix)** — `pick_senders()` cursor=0 reused same 40 senders each block → two in-flight txs per sender with **same nonce** (`docs/reviews/v7-s2-ramp-results.md` Прогон 3). Workers precheck at snapshot height accepts both; both land in `validated_rx` → same seal batch can contain duplicate `tx_hash` → second apply fails `BadNonce`.

2. **Historical HTTP dual-path (fixed `28d05fe`)** — non-roaming HTTP fed both `validated_rx` and `tx_ingress`; same tx as `PreValidated` + `Raw` in one batch. Closed for HTTP; `tx_ingress` drain still runs but has no HTTP producer post-fix.

3. **No pool / batch dedup** — `Mpool` is plain FIFO; `entries` vec has no `tx_hash` collapse before `seal_entries`.

**Answer to ticket Q1:** `prepend_block` does **not** convert PreValidated → worker re-validation. Duplicate `tx_id` logs are upstream duplicate **acceptance**, not eviction requeue.

### 3.2 Why pool stays at 63–64 and blocks never seal

**Per-tick seal failure handler:**

```1958:1993:crates/pwmd/src/lifecycle.rs
Err((e, txs)) => {
    let replay = if e.starts_with("tx: ") {
        // first_bad_tx_ctx → evict index i, keep rest
        ...
    } else { txs };
    g.pool.prepend_block(replay);
}
```

**Atomic seal:** any single bad tx aborts the whole block; state unchanged (`chain.rs:187-199`).

**Why size plateaus at ~64:**

| step | effect |
|------|--------|
| `take(64)` / validated drain | draws up to 64 txs |
| `seal_entries` fails | 1 tx evicted, **63** `prepend_block` |
| same tick loop restarts | `validated_rx` + `tx_ingress` drain **before** next `take` |
| equilibrium | ingress adds ≥1 tx per eviction → net **~64** |

**Why ~30 evictions without drain:** under DDoS many txs have `nonce != account.nonce` (duplicates + stale pending). Each iteration evicts **one** bad tx at a **different index** (batch order shifts) but refills to cap — **O(bad_txs)** evictions before a clean batch, not O(1).

**Busy-loop timing:** on `Err`, `next_seal_time_ms` is **not** advanced (`lifecycle.rs:1800-1801` comment). `should_attempt_seal` stays true; Err path has **no** `poll_pause` sleep (unlike gate-blocked paths). Loop can microcycle until `turn_watch_microcycles > 20` (`lifecycle.rs:1490-1497`). Observed ~650 ms between log lines likely includes gate/write-lock work, not intentional backoff.

**`first_bad_tx_ctx` caveat:** simulates all txs via `apply_tx_with_ctx` (`lifecycle.rs:833-846`), ignoring `SealEntry::PreValidated` fast-path. For mixed batches, evicted index may differ from `seal_entries` failure index (secondary; duplicate-hash issue is primary for DDoS case).

### 3.3 Seal skips whole block, not individual txs

`seal_entries` is all-or-nothing. Eviction is **lifecycle recovery**: drop first simulated failure, requeue remainder. Valid txs are not lost; they spin until batch is bad-tx-free **and** chain can advance nonces.

### 3.4 Log analysis (embedded evidence)

From ticket excerpts (12:51:28–12:51:30, height≈299406, level=40):

| timestamp | event | inference |
|-----------|-------|-----------|
| 12:51:28.653 | evict index **33**, requeue **34** | early storm; pool not yet full |
| 12:51:29.279 | evict index **30**, requeue **63** | pool at cap |
| 12:51:29.946 / 12:51:30.597 | evict index **38**, requeue **63** | **same index 38** twice → same bad tx or stable ordering at tail |
| ~30 repeats / ~650 ms | no `sealed height=` | head_stall |

Prior soak (`20260625-v7-s1-log-timing-report.md`): eviction storm at height **201996** while `seal_suppressed_by_cluster` / attester sync flap — cluster gate can **compound** stall but is not root cause of bad-nonce eviction.

**Workers:** requeued Raw txs skip worker; storm load is **seal-side** `apply_tx_with_ctx` ×64 + eviction simulation, not 8× re-validation of requeued set.

**Post-rotation-fix:** Прогон 4 reached level **68** without DDoS (`v7-s2-ramp-results.md`) — client duplicate nonce eliminated; node eviction loop **can still occur** if many distinct bad-nonce txs fill pool (e.g. flood, sync tx batch), but is **much less likely** at ramp cadence.

## 4. Safety

1. **DoS footgun (confirmed):** eviction + immediate retry burns CPU on write-locked seal path while chain head frozen.
2. **No dedup on `prepend_block`:** cap not re-checked (`mempool.rs:50-53`); safe when `replay.len() ≤ take.len()` but duplicates inflate logical work.
3. **Tx recovery invariant:** current design intentionally requeues good txs; fix must not drop kept batch on eviction.

## 5. Recommended fix (pwm-coding handoff)

**Reject ticket variant A alone** (sleep after prepend) — masks spin, does not reduce bad batches.

**Reject variant B alone** (inline `seal(replay)` without pool) — helps latency but does not stop `validated_rx` from re-merging duplicates; `SealAbort` still flattens `PreValidated` metadata.

**Recommend combined minimal fix (C′ + dedup):**

### 5.1 Dedup at batch assembly (ticket option A adapted)

Before `seal_entries`, collapse `entries` by `tx_hash` (first wins; preserve order). Stops same-block duplicate nonce from duplicate admission.

```rust
// lifecycle.rs — helper
fn dedup_seal_entries(entries: &mut Vec<SealEntry>) {
    let mut seen = HashSet::new();
    entries.retain(|e| seen.insert(e.tx_hash()));
}
```

### 5.2 Slot-local evicted skip set (ticket option C refined)

Track `evicted_hashes: HashSet<[u8;32]>` for current seal **deadline slot**. On eviction, insert `txs[i].tx_hash()`. When draining `validated_rx` / `pool.take` / building entries, skip hashes in set. Clear set when `next_seal_time_ms` advances on successful seal.

Prevents the **same** bad tx from re-entering batch every 50 ms microcycle.

### 5.3 Optional `prepend_block` dedup by `tx_hash`

```rust
// mempool.rs
pub fn prepend_block_dedup(&mut self, txs: Vec<SignedTx>) {
    let mut seen = HashSet::new();
    for tx in txs.into_iter().rev() {
        if seen.insert(tx.tx_hash()) {
            self.q.push_front(tx);
        }
    }
}
```

Use on eviction path only (preserve FIFO for normal `prepend_block` callers).

### 5.4 Break busy-loop without losing recovery

On tx-eviction `Err`, bump deadline one poll tick so gate paths get breathing room:

```rust
// lifecycle.rs, after prepend_block(replay) on tx-eviction path:
let now_ms = crate::current_time_ms().unwrap_or(0);
next_seal_time_ms = now_ms.saturating_add(SEAL_POLL_INTERVAL_MS);
```

Does **not** drop txs; avoids `turn_watch` spin. Alternative: `continue` + explicit `poll_pause` sleep on eviction-only branch.

### 5.5 Tests to add

- `dedup_seal_entries` — two identical `tx_hash` → one entry
- eviction skip-set — evicted hash excluded on next assembly in same slot
- integration: duplicate `validated_rx` enqueue → single block tx count 1

## 6. Style and module shape

Investigation only — no production diff. Proposed helpers `dedup_seal_entries` / `prepend_block_dedup` fit ≤4-word production naming policy.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 7. Tests

Existing: `mempool::mpool_undo_bad_seal` covers prepend-after-fail (`mempool.rs:98-115`). Missing: dedup, eviction skip-set, seal-loop microcycle cap, duplicate validated enqueue.

## 8. Concurrency / parallelism

**Components:** seal task holds `inner.write()` across drain + `seal_entries` + `prepend_block`; workers send to `validated_rx` without lock; `tx_ingress` try_lock drain same critical section.

| Hazard | Assessment |
|--------|------------|
| validated_rx + pool merge race | **Serialized** under write lock ✓ |
| Eviction retry without sleep | **CPU livelock** on proposer — confirmed |
| Worker / seal nonce drift | **Expected** — precheck snapshot vs seal tip; eviction handles if one bad tx per batch |
| `prepend_block` + concurrent push | **Same write lock** — no race |

**Test gap:** concurrent validated enqueue during eviction storm.

## 9. Findings (prioritized)

### High

1. **Eviction microcycle** — `next_seal_time_ms` not advanced on `Err`; no sleep after `prepend_block` → seal slot spins while head frozen.

2. **No batch dedup** — duplicate `tx_hash` in `entries` guarantees `BadNonce` at second position.

### Medium

3. **One-eviction-per-attempt** — with N bad txs in pool, needs N failed seals; ingress keeps cap at 64.

4. **`first_bad_tx_ctx` / `SealEntry` mismatch** — may evict wrong index for PreValidated-heavy batches.

### Low

5. **`prepend_block` no cap check** — pre-existing nit (`pwm-seal-mempool-20260419.md`).

## 10. Verdict

**PASS** — mechanism explained with file evidence; duplicate logs traced to pre-seal admission (benchmark rotation bug + no dedup), pool plateau explained (cap-64 + one eviction + ingress refill + busy retry). Recommended fix: **dedup at assembly + slot evicted-hash skip + poll-tick deadline bump on eviction** — minimal, preserves tx-recovery, compatible with cluster gate. Storm unlikely to recur at ramp level 68 with rotation fix unless new bad-nonce flood path appears.

## 11. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260627-eviction-loop-investigation.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 42000, "confidence": "medium" }`