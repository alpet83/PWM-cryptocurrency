# Review: seal-timing-prio — empty-block JSONL suppression + thread priority (e9b3f7e)

- date: 2026-06-28
- ticket: `20260628-seal-timing-prio-review`
- coding_ticket: `20260628-seal-timing-prio`
- commit: `e9b3f7e`
- branch: `mvp-v7`

## 1. Scope recap

Review commit `e9b3f7e` per coding ticket brief:

| area | change |
|------|--------|
| `crates/pwmd/src/lifecycle.rs` | `block_timing::note_seal` gated by `if !txs.is_empty()` inside successful seal arm |
| `crates/pwmd/src/lifecycle.rs` | `thread_priority::set_current_thread_priority(ThreadPriority::Max)` at top of `spawn_seal_loop` async block (before first `.await`) |
| `crates/pwmd/Cargo.toml` | `thread-priority = "1"` (lock resolves to 1.2.0) |

Relates to MVP observability (`docs/FEATURES.md` § block_timing) and seal-loop latency work on `mvp-v7`. Out of scope: arc-swap, nonce-cache (prior reviews).

## 2. Requirements fit

| Focus area | Verdict | Evidence |
|------------|---------|----------|
| JSONL only for non-empty sealed blocks | **PASS** | `lifecycle.rs:1963-1989` — `note_seal` runs only when `!txs.is_empty()` |
| No regression on seal path / counters | **PASS** | Empty blocks still seal; `inc_sealed_by(0)` at `:1953`; `sealed height=` log at `h==1 \|\| h%10` unchanged (`:1939-1940`) |
| Thread priority on seal loop | **PASS** (best-effort) | `:1413-1414` before first `.await`; comment documents Tokio migration caveat |
| Downstream ramp analysis | **PASS** with doc nits | Ramp joins on tx `batch_height`; loaded runs seal txs — see §3 |

## 3. `note_seal` guard — correctness and downstream impact

### What still fires for every sealed height (including empty)

| hook | when | `lifecycle.rs` |
|------|------|----------------|
| `note_t0` | new grid slot open | `:1744-1757` |
| `note_gate_ok` | cluster gate passes | `:1838-1846` |
| `note_seal` | **only if `!txs.is_empty()`** | `:1963-1989` |

Upstream cluster/attest hooks (`note_send`, `note_att_*`) are unchanged in peer paths.

### Intentional behavior change

Previously every successful seal emitted one JSONL row (`block_timing` flush on `OpKind::Seal`). Empty blocks produced rows with `d_ms.prop_seal_commit` and `seal_slip_ms` but no tx-bearing workload — useful for idle-cadence / slip monitoring, noisy during loaded CY ramp.

**Suppression is coherent with the coding goal:** JSONL rows now mean “seal turn that committed at least one tx,” not “every canonical height.”

### Orphan pending records (low severity)

`note_t0` / `note_gate_ok` still enqueue ops and update `*.pending.json` for empty-block heights. Without `note_seal`, those keys are never removed via the Seal finalize path (`block_timing.rs:620-627`). Entries remain until `trim_pending_map_tail` (`PEND_MAX_RECORDS = 1500`, `:13`, `:877-892`). Bounded leak, not unbounded growth.

### Downstream tools

| consumer | impact |
|----------|--------|
| `scripts/_analyze_transfer_ramp.py` | Joins client `batch_height` to timing rows — ramp submits txs, so heights typically non-empty. `blocks_per_sec_est` uses `t0_ms` deltas between client heights that have rows (`:125-146`); skips missing heights — acceptable for tx-window cadence, not full chain height density. |
| `scripts/cy_cluster_transfer_ramp_soak.py` | `read_block_timing_row` returns `None` for empty heights — same as before for heights without rows; no crash path. |
| `scripts/analyze_tx_distribution.py` | Already filters `n > 0` blocks (`:20-32`) — unaffected. |
| `docs/runbooks/cy-cluster-transfer-ramp-throughput.md` | States `blocks_per_sec_est` from `t0_ms` delta (`:86`) — still valid for tx-bearing rows; **idle-only soak loses per-height JSONL entirely** after this change. |

### Doc drift (nit)

`docs/FEATURES.md:27` still says “одна строка JSON на каждый успешно sealed блок.” Implementation now excludes empty blocks. Recommend a one-line doc update in a polish/doc ticket.

**Regression assessment:** No seal correctness regression. Observability regression is **scoped to empty-block JSONL cadence** — acceptable tradeoff if documented; operators diagnosing idle `head_stall` should use `sealed height=` logs or chain tip, not JSONL row count per height.

## 4. `thread-priority` integration

### API and dependency

- Crate: `thread-priority` 1.2.0 (`Cargo.lock:2348-2359`).
- `ThreadPriority::Max` is a documented enum variant (docs.rs 1.2.0).
- **No Cargo feature flags required** — cross-platform via `libc` (Linux) / `winapi` (Windows).
- `thread-priority = "1"` semver range is appropriate.

### Placement vs Tokio

```1411:1418:crates/pwmd/src/lifecycle.rs
pub fn spawn_seal_loop(app: App) {
    tokio::spawn(async move {
        // Best-effort only: Tokio may migrate this task to another worker thread after awaits.
        let _ = thread_priority::set_current_thread_priority(thread_priority::ThreadPriority::Max);
        let bph = {
            let g = app.inner.read().await;
```

- Priority is set on the **Tokio worker thread that first polls** this task, synchronously before the first `.await`. That is the maximum effect achievable inside `tokio::spawn` without a dedicated OS thread.
- After any `.await`, the runtime may migrate the task; priority does **not** follow the task. Comment correctly states this.
- `let _ =` discards `Result` — matches best-effort observability patterns elsewhere (`try_flush_once`, tx_events send). On Linux without `CAP_SYS_NICE` / RLIMIT, `setpriority` may fail silently.

### Platform / shared-worker caveat (nit)

Raising priority on a **shared** Tokio worker briefly elevates whatever else runs on that thread until migration. `ThreadPriority::Max` is aggressive for a long-lived loop task. Acceptable for lab tuning; production should consider a dedicated `std::thread` or `spawn_blocking` seal driver if priority becomes a hard requirement.

**Not request_changes:** current approach is explicitly best-effort with an accurate comment.

## 5. Style and module shape

- No new production identifiers; existing `lifecycle.rs` `//!` banner present (`:1-2`).
- Dependency addition is minimal and scoped to `pwmd`.
- English comment on Tokio caveat — policy OK.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 6. Safety

- `note_seal` guard: no panic surface; optional `block_timing` unchanged.
- `inc_sealed_by(u64::try_from(txs.len()).unwrap_or(u64::MAX))` pre-existing at `:1953`.
- Thread priority: no new trust boundary; failure is silent (availability/ops nit only).

## 7. Tests

| area | coverage |
|------|----------|
| `block_timing::seal_flush_row_ok` | Non-empty seal path — still valid (`block_timing.rs:1035-1066`) |
| Empty-block `note_seal` skip | **Missing** — optional unit/integration asserting no JSONL line when `txs` empty |
| Thread priority | **Not testable** in CI without elevated caps — acceptable |

`cargo test` / `cargo check`: **UNVERIFIED** (shell unavailable in review session).

## 8. Concurrency / parallelism

Components: `spawn_seal_loop` (single Tokio task), `block_timing` queue (`Arc<Mutex<DefQ>>`), unchanged seal write lock (`app.inner.write().await`).

| hazard | assessment |
|--------|------------|
| New shared mutable state | None introduced |
| Lock across `.await` | Unchanged seal path |
| Thread priority + worker pool | Mild: priority applies to host worker, not task; may affect unrelated tasks on same worker until migration |
| `note_seal` skip + pending map | Serial flush under file lock — no new race; orphan pend keys bounded by trim |

Test gaps: no stress test for pending-map growth under long empty-block soak; no interleaving test needed for this slice.

## 9. Verdict

**Approve with nits**

Prioritized nits:

1. **DOC-1 (low):** Update `docs/FEATURES.md` §1.1 — JSONL row is per **non-empty** sealed block (or “tx-bearing seal”).
2. **DOC-2 (low):** Note in ramp runbook that idle/empty-chain JSONL no longer has one row per height.
3. **OBS-1 (low):** Optional `tracing::debug!` when `set_current_thread_priority` returns `Err` — aids lab diagnosis without failing startup.
4. **TEST-1 (low):** Optional test: seal empty block with timing enabled → JSONL line count unchanged.

No blockers for merge on correctness or concurrency grounds.

## 10. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260628-seal-timing-prio-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 28000, "confidence": "medium" }`