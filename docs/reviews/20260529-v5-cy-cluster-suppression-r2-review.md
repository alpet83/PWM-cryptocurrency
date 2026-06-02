# Review R2: residual CY cluster suppressions after timing-align fix

**Date:** 2026-05-29  
**Agent:** orchestrator (pwm-review worker unavailable; review written in `docs/reviews/` per pipeline)  
**Ticket:** `20260529-v5-cy-cluster-suppression-r2-review`  
**Scope:** review-only RCA; no product edits in this slice  

---

## 1. Scope recap

Owner reports that `20260529-v5-cy-cluster-attest-timing-align-coding` **helped partially** but `seal_suppressed_by_cluster` warnings **continue** during CY soak.

This R2 pass compares what R1 requested vs what coding shipped, explains why residual suppressions are expected, ranks further root causes, and proposes a minimal genesis/code-only coding slice.

Reviewed:

- `tasks/done/20260529-v5-cy-cluster-attest-timing-align-coding.json` (`files_touched` vs acceptance criteria)
- `docs/reviews/20260529-v5-cy-cluster-attest-suppression-review.md`
- `docs/reviews/20260513-cy-lab-sync-vs-cluster-priority-review.md`
- `crates/pwmd/src/lifecycle.rs` (`cluster_timing_ms`, `apply_cluster_timing`, `spawn_seal_loop`, `run_cluster_gate`)
- `crates/pwmd/src/transport/peer_session/seed/session/steady_session.rs`
- `crates/pwmd/src/transport/peer_session/mod.rs` (`send_cluster_prop`, `record_cluster_propose_originated`)
- `cy-cluster-proposer.ps1`, `cy-cluster-common.ps1`
- `docs/runbooks/v5-cy-cluster-precloseout-soak.md`

No fresh post-fix owner logs were present under `tmp/` at review time.

---

## 2. R1 vs shipped (gap analysis)

| R1 recommendation | Shipped in timing-align? | Evidence |
|---|---|---|
| Derive `attest_timeout_ms` / `tx_catchup_ms` from BPH | **Yes** | `lifecycle.rs` `cluster_timing_ms` → BPH=3600: catchup=1000, attest=2000 |
| Cap proposer heartbeat to `<= seal_interval_ms` | **Yes** | `cluster_prop_ms` + `apply_cluster_timing` on proposer |
| Propose cadence not slower than seal (decouple / seal-aligned) | **Partial / No** | `steady_session.rs` **unchanged**; `peer_session/mod.rs` **unchanged** |
| Use `tx_catchup_ms` in cluster path or document | **No** | Still configure+log only; no runtime consumer in cluster gate |
| Tests + runbook | **Yes** | lifecycle tests; runbook healthy thresholds updated |

**Coding `files_touched`:** `lifecycle.rs`, `config.rs`, `main.rs`, runbook only — **not** `steady_session.rs` despite ticket `artifacts.files` listing it.

This gap explains **partial improvement**: longer attest window and faster heartbeat reduce `quorum_timeout` pressure, but **do not remove structural desync** between the seal task and the peer steady loop.

---

## 3. Root-cause taxonomy (ranked)

### P0 — Seal loop and propose path are still decoupled

Two independent Tokio tasks:

1. **`spawn_seal_loop`**: `interval(seal_interval_ms)` → `run_cluster_gate` → seal  
2. **Peer steady session**: `sleep(heartbeat_interval_ms)` → outbound work → `send_cluster_prop` → read/drain  

Even with `heartbeat_interval_ms = 1000` (capped), each steady iteration **starts with sleep**, then runs heartbeat, cross-shard facts, account views, **sync tx batch**, and only then `send_cluster_prop`:

```45:93:crates/pwmd/src/transport/peer_session/seed/session/steady_session.rs
    while live {
        tokio::time::sleep(std::time::Duration::from_millis(hb)).await;
        // ... heartbeat, cross_shard, account_views, sync_tx_batch ...
        if let Err(err) = send_cluster_prop(&app, &cfg, stream, &remote).await {
```

**Effective propose period** ≈ `hb + T_work` where `T_work` is unbounded under catch-up. Seal ticks every `seal_ms` regardless.

`run_cluster_gate` requires round state for `tip_h+1` **before** seal:

```399:407:crates/pwmd/src/lifecycle.rs
    let Some(state) = hs.cluster_attest.rounds.get(&(next_h, round)) else {
        warn!(
            "seal_suppressed_by_cluster reason=quorum_pending detail=missing_round_state height={} round={} ...",
```

Round state is created in `record_cluster_propose_originated`, called from `send_cluster_prop` — **after** sync work in the steady loop, not on the seal tick.

**Expected log signature (steady):** `missing_round_state` on most seal ticks until the peer loop reaches `send_cluster_prop`; occasional `attestations_missing` until attester ACK within `attest_timeout_ms=2000`.

**Why owner sees “partial” fix:** attest timeout doubled (1000→2000) and heartbeat capped (1500→1000) shrink timeout storms and shorten average wait — they do **not** align propose with seal.

### P0 — Seal tick does not open local round state

`spawn_seal_loop` calls `run_cluster_gate` with **no** preceding `record_cluster_propose_originated` / `mk_cluster_prop` on the proposer path (`lifecycle.rs` ~532). Local state only appears when the transport loop sends propose.

Minimal alignment fix (for coding): on proposer + cluster enabled, at each seal tick **before** `run_cluster_gate`, derive `mk_cluster_prop` and call `record_cluster_propose_originated` (wire send can remain on peer loop or be triggered via existing transport hooks).

### P1 — Sync still precedes propose on the wire (20260513 still applies)

Inbound steady loop handles sync variants before cluster routing. Under catch-up, attester/proposer can delay ACK processing. With 1s seal / 2s attest this is **less brittle** than R1 but not eliminated.

Relevant prior art: `docs/reviews/20260513-cy-lab-sync-vs-cluster-priority-review.md` (PASS_WITH_NITS).

### P1 — Startup window (acceptable noise)

Until first peer session completes handshake and reaches `send_cluster_prop`, **all** seal ticks see `missing_round_state`. Runbook already allows **single/burst startup** suppressions.

**Distinguish:** if suppressions persist **after** `cluster propose sent` and `cluster attest accepted` appear regularly while head advances → steady-state P0, not startup.

### P2 — `tx_catchup_ms` still non-functional

Configured to `seal_ms` but unused in `run_cluster_gate` / propose path — operators may assume a second timing lever that does nothing.

### P2 — CLI overrides (ruled out for CY lab)

`cy-cluster-proposer.ps1` does **not** pass `--cluster-attest-timeout-ms` / `--cluster-tx-catchup-ms`. Defaults are overwritten by `apply_cluster_timing` when cluster enabled — **not** a CY launcher issue.

### P2 — Duplicate sessions / reconnect (historical)

20260513 logs showed dual proposer↔attester sessions multiplying propose traffic. Not re-verified without fresh logs; keep as soak monitoring item.

---

## 4. Suppression reason → cause map

| Log detail | Typical phase | Primary cause (R2) |
|---|---|---|
| `missing_round_state` | Startup + steady | No round entry yet; seal tick ahead of `send_cluster_prop` / no seal-tick record |
| `attestations_missing` | Steady | Wire ACK not processed in time; sync/read-loop delay; attester catch-up |
| `quorum_timeout` | Steady (rarer after fix) | `elapsed > attest_timeout_ms` (now 2000ms @ BPH=3600) |
| `binding_incomplete` / `proposer_not_member` | Misconfig | Not cadence-related |

**Healthy steady target (runbook-aligned):** suppressions **≪** sealed blocks after first ~1–2 sealed heights; not one suppression per seal tick.

---

## 5. Quantification (static + historical)

No fresh owner logs in workspace. Historical scan (pre timing-align) remains illustrative only.

**Static model @ BPH=3600 after timing-align:**

| Parameter | Value |
|---|---:|
| `seal_interval_ms` | 1000 |
| `heartbeat_interval_ms` (proposer) | 1000 (capped) |
| `attest_timeout_ms` | 2000 |
| Min time to first propose in iteration | 1000 sleep + T_work |
| Seal attempts per iteration (worst case) | ≥ 1, often 2 if T_work > 0 |

**Upper bound steady `missing_round_state` rate** (if T_work ≈ 0): ~50% of seal ticks (propose at end of iteration, seal at start of next). With sync work, higher.

Owner “partial help” is consistent with moving from **~100%** effective miss rate (1s timeout, 1.5s hb) to **~50–80%** miss rate without reordering/seal-align.

---

## 6. Requirements fit

| Criterion | Result |
|---|---|
| Taxonomy of suppression reasons | PASS — §4 |
| R1 vs landed comparison | PASS — §2 |
| Steady-state rate guidance | PASS — §5 (static; owner scan recommended) |
| Rank P0/P1/P2 + genesis-only fixes | PASS — §3, §7 |
| Next coding ticket proposed | PASS — §7 |
| Startup vs steady verdict | PASS — §3 P1 vs P0 |

---

## 7. Verdict

**REQUEST_CHANGES** — steady-state suppressions after timing-align are **expected** until propose is **seal-aligned** or moved **before** sync work in `steady_session.rs`. Startup-only bursts remain acceptable.

**Not a regression of genesis seal cadence** — 1s seal for BPH=3600 is correct. The residual issue is **transport/scheduling**, not wrong `blocks_per_hour`.

---

## 8. Recommended coding slice (single ticket)

**ID:** `20260529-v5-cy-cluster-propose-seal-align-coding`

**Minimal scope (genesis/code only, no new cadence CLI):**

1. **`lifecycle.rs` / `spawn_seal_loop`:** for `ClusterRole::Proposer` + cluster enabled, before `run_cluster_gate`, call `mk_cluster_prop` + `record_cluster_propose_originated` so round state exists on every seal tick (fail-closed gate unchanged for ACK quorum).
2. **`steady_session.rs`:** move `send_cluster_prop` to immediately after successful heartbeat (before `send_cross_shard_facts` / `send_sync_tx_batch`) so wire propose is not blocked by sync outbound work.
3. **Tests:** unit test — proposer with mocked tip, seal tick records round state before gate; peer_session test that steady ordering sends propose before sync batch (or integration with existing production cluster tests).
4. **Runbook:** note expected suppression count drops to startup-only after align.

**Optional P1 follow-up (separate ticket if scope grows):** inbound cluster-first read budget or catch-up throttle when `quorum_pending` (20260513 items 2–3).

**Explicit non-goals:** new `--seal-interval-ms`; reverting 1s seal; CY launcher CLI timing knobs.

---

## 9. Owner validation recipe

After rebuild with propose-seal-align coding:

```powershell
# Redirect stderr for 5–10 min soak, then:
.\scripts\scan_pwmd_log_counters.ps1 -LogDir .\tmp\cy-soak-<ts> -PerFile
```

Confirm startup lines:

- `seal_cadence genesis_blocks_per_hour=3600 seal_interval_ms=1000`
- `cluster_attest ... attest_timeout_ms=2000 heartbeat_interval_ms=1000`

**Pass:** sealed blocks grow; per-file suppressions ≪ sealed count after height ≥ 2.

---

## 10. Participation

```yaml
agent: orchestrator (review artifact)
result: FAIL
artifacts:
  - docs/reviews/20260529-v5-cy-cluster-suppression-r2-review.md
  - tasks/done/20260529-v5-cy-cluster-suppression-r2-review.json
follow_up_ticket: 20260529-v5-cy-cluster-propose-seal-align-coding
```

---

## 11. Git handoff

```powershell
# git-handoff
Set-Location 'P:\opt\docker\PWM-cryptocurrency'
git add 'docs/reviews/20260529-v5-cy-cluster-suppression-r2-review.md'
git add 'tasks/done/20260529-v5-cy-cluster-suppression-r2-review.json'
git commit -m 'docs(v5-cy): R2 review residual cluster suppressions'
```
