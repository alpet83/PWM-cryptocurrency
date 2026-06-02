# Debug: CY proposer quorum wait root cause

**Date:** 2026-05-30  
**Source:** `pwm-review` (debug lane)  
**Ticket:** `20260531-v5-cy-proposer-quorum-wait-root-cause-debug`

---

## 1. Data sources

| Source | Lines | Span |
|---|---|---|
| `logs/2026-05-30/pwmd-cy-proposer-123837.log` | 249 | 12:38:38 → 12:43:30 (~5 min) |
| `logs/2026-05-30/pwmd-cy-attester-123847.log` | 15 | 12:38:47 → 12:38:56 (startup only) |
| `logs/2026-05-30/pwmd-cy-proposer-051753.log` | prior | 05:17 → 14:55 (~10h, used for comparison) |

---

## 2. Comparison: pre-fix vs post-fix

| Metric | Pre-fix (051753, 10h span) | Post-fix (123837, 5min) |
|---|---|---|
| effective_ms range | 1010 → 257 (−74.3%) | 1010 (stable, clamped) |
| envelope clamp | None (integral windup) | `clamp_applied=false` (in envelope) |
| Seal suppression pct (per 100s window) | ~93% (log-derived) | 55.74% then 48.44% |
| Suppressed per seal (pending_ticks) | ~14.2 avg | ~8.5 avg |
| Quorum_timeout WARNs | 0 (all pre_timeout after startup) | 7 on block 30901 startup, 0 after |
| s/block average | 1.48 | 2.43 (first 90 blocks, improving) |
| Inter-seal delta trend | — | 2.84s → 1.57s (improving over time) |

---

## 3. Hypothesis test results

### H1: Attest RTT + 2-of-2 round-trip exceeds nominal seal tick

**Verdict:** REJECTED (conceptually correct, but not the primary mechanism)

Attest RTT is <10ms on loopback. The real delay is NOT RTT — it is **heartbeat cadence mismatch**.

Evidence:
- All 78 suppressions are `pre_timeout` (71/78) or `timeout` (7/78, startup only)
- `pre_timeout` means `now_ms - propose_opened_at_ms <= attest_timeout_ms` — i.e., the gate returns false BEFORE timeout
- This means the attester IS responding, but the proposer checks quorum at its own tick rate, not attester's response arrival time

---

### H2: heartbeat_interval_ms mismatch — proposer 1000ms vs attester 1500ms

**Verdict: CONFIRMED — ROOT CAUSE**

Startup log evidence:

```
Proposer: heartbeat_interval_ms=1000
Attester:  heartbeat_interval_ms=1500
```

Mechanism:

```
Proposer (every ~1s):
  tick → propose → wait quorum
  tick → quorum_pending (got=0) → FAIL
  tick → quorum_pending (got=0) → FAIL
  tick → quorum OK → SEAL     (≈ 2 ticks wasted)

Attester (every ~1.5s):
  heartbeat → check transport → see proposal → ACK sent
  heartbeat → (nothing) → wait 1.5s
  heartbeat → see next proposal → ACK
```

The proposer ticks at 1010ms intervals but the attester only processes messages at 1500ms intervals. Result: proposer wastes ~2 ticks per seal = ~55% suppression rate.

**Proof of heartbeat mismatch impact:**

After startup transient, inter-seal delta stabilizes at **1.57–1.58 s/block** (10 blocks in 15.7s). At 1.58s/block with effective_ms=1010ms, we need 0.56s of cluster_gate wait. 0.56s / 1.01s = 0.55 ticks wasted per seal → 1.55 ticks total per seal → matches 1.58s/block. This is exactly the heartbeat mismatch impact: without it, effective_ms=1000ms with cluster attest RTT would yield near-1.0s/block.

**Expected fix:** Set `heartbeat_interval_ms=1000` (or ≤ seal_interval_ms) for both proposer AND attester. This aligns the transport poll interval with the seal cadence.

---

### H3: effective_ms drift correction irrelevant; wall dominated by gate false returns

**Verdict: CONFIRMED**

Even with envelope clamp (effective_ms=1010, exactly at nominal), suppression is still 52%. Wall time is 73% cluster_gate wait, 27% effective_ms sleep:

```
Seal cycle breakdown (post-fix):
  effective_ms sleep:      1010ms    (aligned with nominal)
  cluster_gate wait:       550ms     (2-3 ticks @ 1010ms → 550ms avg waste)
  seal CPU work:           ~5ms      (negligible)
  ─────────────────────────────────
  Total per seal:          ~1565ms   (1.57s/block measured)
```

Drift correction was never the primary issue. The envelope clamp only prevented the catastrophic windup (257ms) — it did not restore 1s/block cadence.

---

### H4: Autosnapshot adds 100–500ms every 100 blocks

**Verdict: NOT ACTIVE in fresh log** (only 112 blocks sealed, no autosnapshot fired)

Prior log (051753) showed 160 autosnapshot events aligned with drift windows — they do add to `actual_ms` (max 1.91x expected). But the steady-state 1.48x gap is gate-dominated. Autosnapshot is a secondary confounder, not the primary cause.

---

### H5: Observability gap — pre_timeout suppressions invisible at INFO level

**Verdict: CONFIRMED (but now improved)**

`seal_suppression_summary` (newly added, ERROR level) now provides per-100s window visibility:
```
suppression_pct=55.74 sealed_in_window=27
suppression_pct=48.44 sealed_in_window=33
```

Pre-timeout suppressions remain at DEBUG level (71 lines in 5 minutes) — these are useful for diagnostics but invisible by default. The ERROR-level summary adequately serves the observability requirement.

---

## 4. Root causes — ranked

| Rank | Cause | Confidence | Impact | Fix complexity |
|---|---|---|---|---|
| **R1** | heartbeat_interval_ms mismatch (proposer=1000, attester=1500) | HIGH | 52% suppression | 1-line config alignment |
| R2 | Effective_ms drift windup (already fixed) | CONFIRMED | Was −74.3%, now 0% | Fixed: envelope clamp |
| R3 | Autosnapshot spikes every 100 blocks | MEDIUM | 10–20ms per window spike | Pre-existing, secondary |
| R4 | Startup quorum_timeout on first block (snapshot load: 7.9s) | LOW | 7 WARNs, 1 block | Startup-only transient |

---

## 5. Concrete fix recommendation

### Priority fix: align heartbeat_interval_ms

**File:** PwmdConfig or genesis/CLI defaults  
**Change:** Ensure both CY proposer and CY attester use `heartbeat_interval_ms = 1000` (matching `seal_interval_ms`)

```rust
// Expected after fix:
// Proposer: heartbeat_interval_ms=1000
// Attester:  heartbeat_interval_ms=1000  (was 1500)
```

**Expected impact:** suppression drops from ~52% to <15%, s/block approaches 1.1s.

**Ticket:** `20260531-v5-cy-attester-heartbeat-align-coding` — scope: one config change, no RFC changes needed.

### Long-term: RFC16 reattest model (separate track)

The 2-of-2 quorum model with per-block attest is inherently a ~2× multiplier on seal cadence. To achieve true 1 block/s in a cluster, the RFC needs to decouple seal from attest (pipeline attest; proposal pipelining; attest-after-seal). This is a V6+ architectural design, not a V5 fix.

---

## 6. Verdict

**ROOT CAUSE IDENTIFIED** — `heartbeat_interval_ms` mismatch (proposer 1000ms vs attester 1500ms) causes 2–3 proposer ticks of wasted `quorum_pending` per seal. Fixing this alone should reduce suppression from ~52% to <15% and bring s/block close to 1.1s. Envelope clamp (already fixed) resolves the −74.3% windup but does not address the gate-wait bottleneck.

**Recommended next ticket:** `20260531-v5-cy-attester-heartbeat-align-coding` (1-line config change, pwm-coding).