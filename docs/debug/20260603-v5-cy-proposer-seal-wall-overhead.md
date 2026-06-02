# V5 debug: CY proposer seal wall overhead — attest path vs suppression taxonomy

Date: 2026-05-30  
Ticket: `20260603-v5-cy-proposer-seal-wall-overhead-debug`

## Executive summary

- Observation `~34/100` suppression is confirmed directionally; in sampled steady window we measure **~30/100 ticks suppressed** (`30.21%`) with same class of wall slowdown.
- Throughput in selected window is slow: **100 blocks in 228.904s** (`2.289 s/block`) for height pair `31110 -> 31210`.
- Taxonomy split shows wall dominated by **A (pre-timeout quorum wait)**, not by **B (quorum_timeout)**:
  - steady counts: `pre_timeout=409`, `quorum_timeout=0` (after startup cut)
- Transport RTT (D) is real but not enough alone:
  - propose→attest accepted median **488ms**, p95 **~1079ms**.
- Main remaining overhead is the gap between ACK and actual seal scheduling/gate close, plus repeated gate polling under RFC16 cadence.

Conclusion: wall overhead is primarily attest/quorum wait path (A) and interval strike accounting (C), while explicit hard timeouts (B) are startup-only in this slice.

---

## Data scope and inputs

Primary analyzed window (paired CY logs):

- `logs/2026-05-30/pwmd-cy-proposer-125516.log`
- `logs/2026-05-30/pwmd-cy-attester-125554.log`
- `logs/2026-05-30/pwmd-peer-cy-proposer-125516.log`
- `logs/2026-05-30/pwmd-peer-cy-attester-125554.log`

Prior context read:

- `docs/debug/20260531-v5-cy-proposer-quorum-wait-root-cause.md`
- `docs/reviews/20260531-v5-cy-proposer-seal-throughput-degradation-review.md`
- `docs/runbooks/v5-cy-cluster-precloseout-soak.md`

---

## Taxonomy A–E with measured counts

### C) Interval strike summary windows (`seal_suppression_summary`)

From proposer log (`125516`):

- total summary windows: `20`
- all windows totals: `ticks=1515`, `suppressed=467`, `sealed_in_window=1048`, suppression `30.83%`
- steady subset (drop first 2 windows):
  - `ticks=1397`, `suppressed=422`, `sealed_in_window=975`
  - suppression `30.21%` ⇒ **~30 suppressed / 100 ticks**

This is consistent with owner correction that suppression is in the “tens per 100” regime, not single digits.

### A) Gate wait pre-timeout (`reason=quorum_pending phase=pre_timeout`)

Steady slice (after 3rd summary starts):

- `pre_timeout` lines: **409**

Interpretation: most denied seal attempts are waiting for attest inside timeout envelope, not hard failures.

### B) Hard timeout (`reason=quorum_timeout`)

Steady slice:

- `quorum_timeout` lines: **0**

Startup of this same file contains repeated timeout on `height=31101` before attester readiness, but it does not persist in steady mode.

### D) Transport attest RTT (`cluster propose sent` → `cluster attest accepted`)

From `pwmd-peer-cy-proposer-125516.log` pairing by `(height, round=0)`:

- matched rows: `107` (steady subset `h>=31200`: `98`)
- median RTT: **488ms**
- p95 RTT: **~1079ms**
- max RTT: **1141ms**

### E) Sealed success (`sealed height=`)

Per sampled 100-block pair:

- `31110 -> 31210`: **228.904s** wall, `2.289 s/block`
- checkpoint-normalized per-block deltas in that band:
  - median `2.199s`
  - p95 `3.146s`
  - max `3.138s`

---

## Sampled per-height timeline (propose → ACK → seal)

From paired proposer + peer logs (12 exemplars):

| Height | Propose | ACK | Seal | Propose→ACK | ACK→Seal | Total |
|---:|---|---|---|---:|---:|---:|
| 31110 | 12:56:32.782 | 12:56:33.225 | 12:56:34.422 | 443 ms | 1197 ms | 1640 ms |
| 31190 | 12:59:26.595 | 12:59:26.900 | 12:59:27.068 | 305 ms | 168 ms | 473 ms |
| 31270 | 13:01:58.445 | 13:01:59.451 | 13:01:59.459 | 1006 ms | 8 ms | 1014 ms |
| 31350 | 13:04:29.065 | 13:04:29.522 | 13:04:29.537 | 457 ms | 15 ms | 472 ms |
| 31430 | 13:06:56.892 | 13:06:57.271 | 13:06:57.347 | 379 ms | 76 ms | 455 ms |
| 31510 | 13:09:30.615 | 13:09:31.474 | 13:09:31.861 | 859 ms | 387 ms | 1246 ms |
| 31590 | 13:11:51.168 | 13:11:51.709 | 13:11:52.478 | 541 ms | 769 ms | 1310 ms |
| 31670 | 13:14:09.899 | 13:14:09.907 | 13:14:10.902 | 8 ms | 995 ms | 1003 ms |
| 31750 | 13:16:55.543 | 13:16:55.848 | 13:16:56.007 | 305 ms | 159 ms | 464 ms |
| 31830 | 13:19:37.627 | 13:19:37.807 | 13:19:38.628 | 180 ms | 821 ms | 1001 ms |
| 31910 | 13:21:59.636 | 13:22:00.213 | 13:22:01.109 | 577 ms | 896 ms | 1473 ms |
| 31990 | 13:24:07.291 | 13:24:07.299 | 13:24:07.788 | 8 ms | 489 ms | 497 ms |

Key point: even when RTT is small, `ACK→Seal` can still consume ~0.5–1.2s. This supports gate/scheduler polling overhead as a separate component from pure wire RTT.

---

## Hypothesis matrix verdicts (H1–H8)

- **H1** (`~34/100` strikes map to over-nominal slots): **PARTIAL CONFIRM**
  - We observe stable `~30/100` in this slice; same qualitative regime. Need longer multi-window cross-date replay for exact 34.

- **H2** (majority overhead is pre-timeout wait): **CONFIRMED**
  - `pre_timeout=409` vs `quorum_timeout=0` in steady slice.

- **H3** (`quorum_timeout` rare in steady mode): **CONFIRMED**
  - Timeouts concentrated in startup (`height=31101`), disappear afterward.

- **H4** (RTT moderate, but loop stretches slot > nominal): **CONFIRMED**
  - RTT median 488ms, p95 ~1079ms; total per sampled block often >1.0s and up to 1.6s, with sizable ACK→Seal lag.

- **H5** (structural polling effects under RFC16): **LIKELY**
  - `cluster_gate_pending_summary` present per sealed progression with pending ticks; median pending ticks is non-trivial in this run.

- **H6** (grid align jitter secondary): **PARTIAL / NOT ISOLATED**
  - Jitter visible in per-block totals, but current evidence attributes dominant share to gate wait path.

- **H7** (autosnapshot/CPU not dominant): **LIKELY**
  - No data here showing autosnapshot as primary contributor for this specific window; prior RCA also points to gate-dominated wall.

- **H8** (metrics must split wait vs strike): **CONFIRMED**
  - Current operator-facing summary provides C (strike), but A (wait inside timeout) is only inferable from DEBUG logs and not surfaced as first-class metric.

---

## Attest-wait vs strike vs timeout contribution

For this sampled window:

- **Timeout path (B)** contribution to extra wall in steady operation: near zero.
- **Strike path (C)** remains high (`~30%` of ticks suppressed).
- **Wait path (A)** is the dominant residual source and explains most of wall overhead beyond nominal.

Given nominal 1.0s/block and observed ~2.2–2.3s/block in sample pair, excess is ~1.2–1.3s/block. RTT median (~0.49s) plus ACK→Seal lag (often ~0.5–1.0s) accounts for majority of excess; this is consistent with “attest/quorum wait dominated” interpretation.

---

## Comparison to 20260531 RCA

What remains consistent:

- Gate path dominates over CPU-bound sealing.
- Pre-timeout waits are far more common than true timeouts in steady state.
- Suppression summary remains in “tens per 100 ticks” territory.

What this slice adds:

- Explicit A/B/C/D/E split in one paired window.
- Concrete propose→ACK→seal exemplars showing ACK→Seal lag can rival RTT.

---

## Recommendations and follow-ups

### 1) V5 coding follow-up (metrics split) — `pwm-coding`

Add explicit operator counters per 100s window (INFO/ERROR summary block):

- `slots_waited_attest` (A): ticks where gate failed with `pre_timeout`
- `slots_timeout` (B): ticks or slot outcomes with `quorum_timeout`
- `slots_struck` (C): existing suppression strike count
- optional: `ack_rtt_ms_p50/p95`, `ack_to_seal_ms_p50/p95`

This prevents conflating “waiting but healthy” with “true timeout/failure”.

### 2) RFC16 / V6 track

Evaluate design options to reduce structural wait:

- pipelined attest / decoupled seal-commit acknowledgment,
- reduced polling latency coupling between propose and seal close,
- explicit per-round timing model in operator docs.

No protocol behavior change proposed in this debug slice.

---

## Acceptance check

- [x] Documents suppression in `~30/100` range with direct log evidence (same order as owner `~34/100`).
- [x] Separates attest wait (A) from timeout (B) and strike (C).
- [x] Provides sampled timeline propose→ACK→seal (12 exemplars).
- [x] Gives metric-split recommendation (H8) for coding follow-up.
- [x] No product code edits.

---

## Participation

Agent: `pwm-debug`  
Mode: debug evidence + RCA reporting  
Files edited: docs only
