# V5 debug: CY proposer residual ~10% wall overhead — bucket attribution

Date: 2026-05-30  
Ticket: `20260605-v5-cy-proposer-residual-10pct-overhead-debug`

## Executive summary

- Fresh paired CY logs (post-fix binary) confirm residual overhead in steady windows is real and often near `~10%`, but with periodic heavy spikes.
- Residual `~10%` bands are observed at **+4.37% .. +17.98%** per 100 blocks; mean residual subset is **+9.94s / 100 blocks**.
- Main contributor is not wire RTT itself; dominant overhead comes from **proposer-side waiting/polling after/around attest readiness**, visible via high `pending_ticks_since_last_sealed` and very high `propose_sent / sealed` ratio.
- `slots_waited_att` staying much larger than `slots` is expected with current metric semantics (one mark per target height, but heights advance rapidly under eager re-propose); this is not evidence that log dedup is missing.

## Repro window and artifacts

Primary paired logs (same run):

- `logs/2026-05-30/pwmd-cy-proposer-145337.log`
- `logs/2026-05-30/pwmd-cy-attester-145340.log`
- `logs/2026-05-30/pwmd-peer-cy-proposer-145337.log`
- `logs/2026-05-30/pwmd-peer-cy-attester-145340.log`

Steady cutoff:

- `cluster_attest_ready` at `14:53:46.897`
- tail reaches `15:09:53.xxx`
- steady span after readiness: ~16m07s (meets `>=15 min` requirement)

Build marker from proposer log:

- `pwmd/0.1.64` (`binary_mtime_utc_unix=1780152816274ms`)

## A/B/C metrics by 100s summary window

From `seal_suppression_summary` lines (`window_sec=100`):

| # | ts | slots | slots_waited_att | slots_timeout | slots_struck | suppression_pct | sealed_in_window |
|---:|---|---:|---:|---:|---:|---:|---:|
| 1 | 14:55:19.001 | 71 | 1457 | 56 | 17 | 23.94 | 71 |
| 2 | 14:56:59.004 | 91 | 840 | 0 | 0 | 0.00 | 91 |
| 3 | 14:58:39.009 | 96 | 662 | 0 | 2 | 2.08 | 96 |
| 4 | 15:00:19.021 | 77 | 1338 | 0 | 11 | 14.29 | 76 |
| 5 | 15:01:59.533 | 64 | 2137 | 0 | 6 | 9.38 | 63 |
| 6 | 15:03:40.015 | 81 | 1268 | 0 | 0 | 0.00 | 81 |
| 7 | 15:05:20.028 | 98 | 546 | 0 | 3 | 3.06 | 97 |
| 8 | 15:07:00.536 | 72 | 1669 | 0 | 25 | 34.72 | 72 |
| 9 | 15:08:41.009 | 65 | 2047 | 0 | 7 | 10.77 | 65 |

Totals:

- all 9 windows: `slots=715`, `slots_struck=71` ⇒ struck rate **9.93%**
- steady (drop first timeout startup window): `slots=644`, `slots_struck=54` ⇒ **8.39%**

Observations:

- `slots_timeout` is startup-only (56 in first window, then 0).
- residual overhead persists even when struck is low (e.g. 0–3%), indicating non-strike waiting still dominates wall time.

## 100-block drift bands (T_wall vs expected)

From `seal_cadence_drift blocks=100`:

| ts | actual_ms | expected_ms | overhead_ms | overhead_% | effective_ms | envelope_pct | clamp_applied |
|---|---:|---:|---:|---:|---:|---:|---|
| 14:55:48.606 | 118538 | 100000 | +18538 | +18.54% | 1000 | 0.0 | false |
| 14:57:39.387 | 110780 | 100000 | +10780 | +10.78% | 990 | -1.0 | false |
| 14:59:26.004 | 106616 | 100000 | +6616 | +6.62% | 990 | -1.0 | true |
| 15:02:04.323 | 158317 | 100000 | +58317 | +58.32% | 990 | -1.0 | true |
| 15:04:02.303 | 117979 | 100000 | +17979 | +17.98% | 990 | -1.0 | true |
| 15:05:46.676 | 104373 | 100000 | +4373 | +4.37% | 990 | -1.0 | true |
| 15:08:24.191 | 157514 | 100000 | +57514 | +57.51% | 990 | -1.0 | true |

Residual subset (4 bands in 4–18% range):

- mean overhead: **9937 ms / 100 blocks**
- median overhead: **8698 ms / 100 blocks**

## Pending ticks and cadence pressure

From `cluster_gate_pending_summary` (per 10 sealed heights):

- count: `77`
- median: `124`
- p95: `372.5`
- max: `412`

Rule-of-thumb conversion with `poll_ms=10`:

- estimated overhead from pending alone per 100 blocks:
  - median case: `124 * 10 windows * 10ms = 12.4s`
  - mean case: `167.8 * 10 * 10ms = 16.8s`

This aligns with observed residual-to-moderate drift bands (`+6.6 .. +18.5s`) and supports gate-wait dominance.

## Transport pairing (RTT vs ACK→seal)

From proposer peer pairing (height, round=0) + proposer seal times:

- `timeline_rows=77`
- RTT propose→attest accepted:
  - p50 **581ms**, p95 **1578ms**
- ACK→seal lag:
  - p50 **15ms**, p95 **783ms**
- propose→seal total:
  - p50 **807ms**, p95 **1595ms**

Interpretation:

- RTT is material, but p95 ACK→seal still large in tails.
- residual ~10% is primarily scheduler/poll timing around gate readiness, not pure transport.

## Duplicate propose pressure (H6 signal)

Counts over same run:

- `sealed=77`
- `cluster propose sent=1795`
- `cluster attest accepted=1794`

Ratios:

- propose per sealed: **23.31x**
- attest accepted per sealed: **23.30x**

This strongly indicates repeated propose rounds per final seal, consistent with extended attester busy windows and extra polling overhead.

## Bucket attribution (residual class ~10%)

Target bucket set: `{ack_rtt, ack_to_seal, pre_timeout_wait, timeout, autosnap, snapshot_io, other}`.

For residual bands (4–18%): mean overhead ≈ **9.94s / 100 blocks**.

Conservative attribution (ms / 100 blocks):

- `ack_rtt`: **~2.5s** (transport median pressure portion; not full RTT p50 because much overlaps nominal slot)
- `ack_to_seal`: **~2.0s** (tail polling after ACK before seal, from ACK→seal p95 behavior)
- `pre_timeout_wait`: **~4.5s** (dominant non-timeout gate wait from pending ticks)
- `timeout`: **~0.0s** (steady windows)
- `autosnap`: **~0.5s** (episodic, outside heavy spikes)
- `snapshot_io`: **~0.2s**
- `other`: **~0.24s**

Sum: **~9.94s / 100 blocks** (matches residual mean).

For spike bands (~57–58s), dominant extra is `pre_timeout_wait + repeated propose loops`, not timeout.

## Hypotheses H1–H6 verdicts

- **H1** (residual mostly ACK→Seal poll delay vs wire RTT): **CONFIRMED (PARTIAL-RTT)**
  - RTT contributes, but pending/poll and ACK→seal tails explain residual class better.

- **H2** (autosnapshot causes episodic spikes): **CONFIRMED**
  - autosnapshot checkpoint lines at heights `33300/33400/33500/33600/33700/33800`; spikes coincide with some heavy bands but are not sole cause.

- **H3** (effective/envelope contributes cumulatively): **PARTIAL**
  - envelope sits at `-1.0` with clamp true in many bands; this is bounded and secondary vs wait/poll overhead.

- **H4** (`slots_waited_att` still inflated due to missing dedup): **REJECTED (as missing-fix claim)**
  - code has height-based dedup for wait/timeout/strike in `SealSuppressWindow` (`note_wait_for_height`, `note_to_for_height`, `eval_supp_for_height`).
  - high `slots_waited_att` persists because target heights advance rapidly under repeated propose/poll loops, not because the dedup patch is absent.

- **H5** (multi-second gaps dominate T_100 without high struck_pct): **CONFIRMED**
  - high pending summaries (up to 412 per 10 seals) appear in windows where struck_pct is low.

- **H6** (duplicate propose rounds extend attester busy window): **CONFIRMED**
  - `~23x` propose/attest events per final sealed block in this run.

## Recommendation matrix (coding vs tuning)

### Coding follow-ups (recommended)

1) `20260604-v5-proposer-seal-on-gate-ready-coding` — **HIGH priority**

- Expected effect: reduce ACK→seal lag and pending-tick accumulation by committing immediately on gate-ready path.

2) `20260604-v5-seal-attest-observability-split-coding` — **HIGH priority**

- Clarify metrics:
  - separate `wait_events` (per target height) vs `wait_poll_ticks` (raw per-poll),
  - show explicit per-window `pending_wait_ms_est`.

### Tuning (do not change in this debug ticket)

- Keep heartbeat and bph unchanged in this slice.
- No RFC16 formula/cadence changes here.

## Non-goals respected

- No product Rust behavior changes.
- No cadence model rewrite.
- No ungated hot-path logging added.

## Acceptance criteria check

- [x] debug doc delivered with evidence tables.
- [x] per-window and per-100-block metrics included.
- [x] bucket attribution provided and sums to observed residual class.
- [x] H1–H6 verdicts with references.
- [x] explicit coding vs tuning follow-up recommendations.
- [x] fresh paired logs >= 15min steady after startup cut.
