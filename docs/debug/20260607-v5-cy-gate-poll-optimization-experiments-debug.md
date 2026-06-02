# 2026-06-07 V5 CY Gate/Poll Optimization Experiments Debug

## Scope

Follow-up for ticket `20260607-v5-cy-gate-poll-optimization-experiments-debug` after owner approval for temporary logic patches in `pwmd` and launcher-level workarounds on Windows.

This pass focused on recovering a valid fresh-state baseline first, then rechecking patched resumed behavior.

## Temporary code and launcher changes used in this pass

- `cy-cluster-proposer.ps1` and `cy-cluster-attester.ps1` were already patched earlier in the session to prefer direct `rust-target-shared/debug/pwmd.exe` execution and avoid Windows `os error 5` binary-lock races from concurrent `cargo run`.
- `crates/pwmd/src/transport/lifecycle.rs`
  - clear `cluster_attest.sent_key_by_node[node_id]` on `record_peer_close()`.
- `crates/pwmd/src/transport/peer_session/mod.rs`
  - allow same `(height, round)` `cluster_propose` resend while the proposer still has zero attestations for that round.

Both Rust edits were validated with:

```text
cargo test -p pwmd cluster_prop_ -- --nocapture
```

## Root-cause findings confirmed in this pass

### F1. Fresh-state `height=1` stall was not only startup ordering

Serial attester-first startup alone did not fix the fresh-state failure.

Authoritative failed run evidence:

- proposer log: `logs/2026-05-30/pwmd-cy-proposer-174345.log`
- proposer peer log: `logs/2026-05-30/pwmd-peer-cy-proposer-174345.log`
- attester peer log: `logs/2026-05-30/pwmd-peer-cy-attester-174344.log`

Observed behavior:

- proposer sent `cluster propose sent ... height=1 round=0`
- attester dropped the early inbound cluster frame before trust-map population
- proposer never retransmitted the same `(height=1, round=0)` proposal on the surviving path
- result was endless `quorum_pending` / eventual timeout at genesis

### F2. Two resend-suppression defects were involved

#### F2a. Stale dedup after close

`send_cluster_prop()` deduped forever by `sent_key_by_node`; that entry was not cleared on peer close.

Effect:

- a proposal sent on a dead socket could suppress all future retransmits for the same remote node and same `(height, round)` after reconnect.

#### F2b. No resend while attestation count stayed zero

Fresh-state runs exposed a second race even without a reconnect loop:

- attester initially saw proposer as `untrusted_peer` on the inbound socket
- trust was populated only moments later through the separate outbound seed session
- proposer had already recorded the round as sent and would not resend while the round still had zero attestations

Effect:

- the first trusted window after attester trust-map population could still miss the proposal forever.

## Recovered valid fresh baseline

### E1 fresh, valid after both temporary fixes

Run artifacts:

- run dir: `tmp/cy-opt-E1-fresh-20260530_205111`
- proposer log: `logs/2026-05-30/pwmd-cy-proposer-175114.log`
- proposer peer log: `logs/2026-05-30/pwmd-peer-cy-proposer-175114.log`
- attester peer log: `logs/2026-05-30/pwmd-peer-cy-attester-175112.log`

Key evidence:

- proposer reached `cluster_attest_ready` at `17:51:16.025`
- proposer sealed `height=1` at `17:51:17.498`
- proposer had sealed through `height=10` by `17:51:26.277`
- proposer peer log shows repeated resends only until the first attestation lands, then normal progress:
  - multiple `cluster propose sent ... height=1 round=0`
  - `cluster attest accepted ... height=1 round=0 attesters=1`
  - then the same pattern for heights `2..12`

Normalized throughput slice collected from REST:

- start height: `5`
- target height: `55`
- elapsed for `+50` blocks: `42.24s`
- `T100_est ~= 84.48s`

Interpretation:

- fresh-state genesis stall is recoverable with the temporary resend fixes
- fresh-state performance on the patched debug binary is dramatically better than the earlier stuck-at-genesis behavior

## Patched resumed baseline: only partial

### E0 resumed on patched binary

Run artifacts:

- run dir: `tmp/cy-opt-E0-resumed-patched-20260530_205306`
- proposer log: `logs/2026-05-30/pwmd-cy-proposer-175308.log`
- proposer peer log: `logs/2026-05-30/pwmd-peer-cy-proposer-175308.log`
- attester peer log: `logs/2026-05-30/pwmd-peer-cy-attester-175306.log`

Observed behavior:

- same initial `untrusted_peer` drop still appears once on `height=1`
- resend logic recovers `height=1`
- proposer seals `height=1` at `17:53:11.584`
- attester later accepts repeated `height=2 round=0` proposals
- proposer peer log eventually records `cluster attest accepted ... height=2 round=0 attesters=1` around `17:54:14.354`
- but the proposer head and proposer main log did not progress cleanly into steady sealing beyond that point during this slice

Normalized throughput attempt:

- start height: `1`
- target height: `51`
- measured end height after `183.57s`: `1`
- `+50` block slice was not reached

Interpretation:

- the fresh-state deadlock is fixed for debug purposes
- resumed-state behavior is improved enough to recover `height=1`, but another post-attestation or seal-loop issue still remains around `height=2` / onward progress in this run shape

## Current status

- `E1 fresh`: PASS on temporary debug binary
- `E0 resumed` on patched binary: PARTIAL
- `X1 ahead_off`: not run in this pass because resumed baseline remained unstable after `height=1`

## Recommended next step

Before measuring `ahead_off`, debug one more level down on the resumed `height=2` path:

1. correlate `cluster attest accepted height=2` in `pwmd-peer-cy-proposer-175308.log`
2. inspect why `pwmd-cy-proposer-175308.log` does not continue sealing despite accepted quorum
3. only after resumed sealing is steady, rerun:
   - `E0 resumed` on current patched logic
   - `X1 ahead_off` on the same resumed state

## Continuation update (2026-05-31): ticket stays OPEN

Owner decision accepted for this pass: publish report now, keep the ticket open, continue root-cause work for unstable auto seal rhythm.

### Latest evidence snapshot

- run dir: `tmp/cy-opt-E0-yield2-20260531_210308`
- auto-mode benchmark (`120s`) from resumed head:
  - start head: `65708`
  - end head after `120.2s`: `65708`
  - result: `0 blocks / 120s`
- attester peer handshake counters (latest run slice):
  - `hello_completed=3`
  - `hello_read_failed wire_read_len_timeout=77`
- proposer tail pattern remains dominated by:
  - repeated `seal_suppressed_by_cluster ... got=0 need=1`
  - short `cluster_attest_ready` bursts followed by sync/gate loss

Interpretation:

- temporary fairness changes (poll interval 50ms + partial `yield_now`) improved some runs,
- but resumed auto-mode still has non-deterministic stalls,
- root cause is not fully closed for stable throughput.

### Closure status

- status: `PARTIAL`
- ticket state: `OPEN`
- reason to keep open: acceptance target for stable resumed auto cadence is not met (`100 blocks / 100s` still unproven).

### Next debug slice (active)

1. Complete fairness instrumentation in remaining `poll_pause` continuations inside `crates/pwmd/src/lifecycle.rs`.
2. Rebuild and rerun resumed auto benchmark under the same CY topology.
3. Collect per-run matrix:
   - head delta in fixed wall window,
   - handshake fail/completed counts,
   - suppression summaries and `quorum_pending` density.
4. Use LLDB path on live stalled proposer when needed to confirm scheduler/await starvation vs transport/session churn.
5. Promote to closure candidate only after at least two independent resumed runs show stable progression with no long zero-progress windows.

## Debug slice update (2026-05-31, next-step execution)

### Code delta applied

Completed fairness patching in remaining `poll_pause` continue branches in `crates/pwmd/src/lifecycle.rs`:

- `debug_disable_seal_loop` continue path: added `tokio::task::yield_now().await`
- attester-role early continue path: added `tokio::task::yield_now().await`
- cluster-gate fail continue path: added `tokio::task::yield_now().await`

Build validation:

- `cargo build -p pwmd --bin pwmd` succeeded after Windows lock workaround (`pwmd.exe` rename before relink).

### Runtime setup and benchmark

Topology used for this slice:

- only proposer + one attester were running (extra attesters not started)
- proposer launcher: `cy-cluster-proposer.ps1`
- attester launcher: `cy-cluster-attester.ps1`

Resumed auto benchmark (`120s`, proposer REST head):

- start head: `65795`
- end head: `65884`
- delta: `+89` blocks in `120.1s`
- `T100_est ~= 135.0s`

Primary run logs:

- proposer log: `logs/2026-05-31/pwmd-cy-proposer-181654.log`
- attester peer log: `logs/2026-05-31/pwmd-peer-cy-attester-181630.log`

### Observed evidence

- proposer suppression summaries in this slice:
  - `suppression_pct=0.00 sealed_in_window=70` (18:18:35)
  - `suppression_pct=0.00 sealed_in_window=77` (18:20:16)
- attester peer handshake health (latest attester-peer log):
  - `peer handshake completed=2`
  - `peer handshake failed=0`
- proposer still shows high variance in `pending_ticks_since_last_sealed` summaries (e.g. 75..94 in tail), which aligns with residual cadence overhead even without full stalls.

### Conclusion for this slice

- full-yield coverage removed the prior zero-progress resumed behavior in this run shape,
- resumed auto now advances continuously,
- but target `100 blocks / 100s` is still not met (`T100_est ~= 135s`).

Status after this slice:

- ticket status: `OPEN`
- result: `PARTIAL`
- residual gap: throughput/cadence variance on resumed auto path.

Operational cleanup:

- all `pwmd` processes were stopped after evidence capture (no stray background nodes kept by this slice).

---

## Debug slice update (2026-05-31, run-3 with JSONL block-timing)

### Setup

Root-cause continuation. `PWM_BLOCK_TIMING_ENABLED` was hardcoded `'false'` in `cy-cluster-common.ps1` (comment: "Disabled until 20260610 nonblocking merge") — preventing any JSONL collection. Changed to conditional guard with default `'true'` for this debug session. Both nodes restarted cleanly.

### Prогон 3 — результаты бенчмарка

- Snapshot resumed from `tip_h=65800`, `seal_interval_ms=1000`
- Benchmark window: 120.1s
- Blocks sealed: 80 (h65829 → h65909)
- **T100_est = 150.2s** — regression vs run-2 (135s), attributable to JSONL write overhead

### JSONL block-timing analysis (N=159 records)

**Field statistics:**

| Field | N | mean | p50 | p95 | max | min |
|---|---|---|---|---|---|---|
| `wall_total_ms` | 159 | 947.4 | 999 | 1512 | 1677 | 8 |
| `seal_slip_ms` | 159 | 1031.6 | 1009 | 1561 | 9669 | 13 |
| `pending_ticks_at_seal` | 159 | 33.5 | 33 | 73 | 93 | 0 |
| `attest_rtt_ms` (prop_rx_attest − att_rx_propose) | 153 | 601.9 | 483 | 1450 | 1607 | 0 |
| `prop_rx_attest_ms` | 159 | 891.9 | 940 | 1478 | 1614 | 0 |

- `attest_timeout=True`: 0 (yield coverage effective)
- `suppress_strike=True`: 0

**wall_total distribution (200ms buckets):**

```
    0ms: ######### (17)   ← burst-catchup multi-tick seals
  200ms: # (1)
  400ms: ###### (6)
  600ms: ######################### (25)
  800ms: ############################### (31)
 1000ms: ############################ (28)
 1200ms: ############################# (29)
 1400ms: ################### (19)
 1600ms: ### (3)
```

### Root-cause findings from JSONL

**F3. Attestation RTT is the primary rhythm instability source.**

- `prop_rx_attest` p50=940ms, p95=1478ms against a 1000ms seal nominal.
- p95 overhead leaves only ~520ms margin before the 2000ms `attest_timeout` fires.
- 5% of seal cycles take 1400–1677ms, causing the cadence to slip past the nominal grid by 400–677ms in that fraction.

**F4. High `pending_ticks` mean=33.5 on resumed state.**

- Resumed run from `tip_h=65800` starts with a large backlog of pending grid ticks.
- This drives the burst-seal pattern visible as 17 blocks at wall_total≈0ms.
- The burst behaviour compresses inter-seal timing in fast windows but creates variability.

**F5. JSONL write overhead adds measurable regression.**

- T100_est regressed from 135s (run-2, no JSONL) to 150.2s (run-3, JSONL enabled).
- Confirms the original "Disabled until 20260610 nonblocking merge" comment — write contention on Windows is real.

### No anomaly outliers

No records exceeded the 3×median threshold (2997ms). The distribution is wide but unimodal — no pathological stalls.

### Current diagnosis summary

| # | Root cause | Evidence | Status |
|---|---|---|---|
| F1 | fresh-state h=1 stall (resend + peer-key fix) | resolved in earlier slice | CLOSED |
| F2 | zero-progress resumed stall (yield coverage gap) | resolved in run-2 (this session) | CLOSED |
| F3 | attest RTT p95=1450ms dominates cadence variance | JSONL run-3 | **OPEN** |
| F4 | pending_ticks backlog on resume (mean=33.5) | JSONL run-3 | **OPEN** |
| F5 | JSONL write contention on Windows | regression T100 135→150s | INFO — revert timing after session |

### Next steps (proposed)

1. Investigate `attest RTT` p95: is it TCP loopback scheduling jitter (Windows) or proposer-side gate latency?
2. Check `seal_ahead_ms` tuning: increasing from 100ms might compensate for attest RTT variance.
3. Investigate pending_ticks backlog origin on resume — is the grid computation correct for resumed tip_h far behind wall-clock?
4. Revert `PWM_BLOCK_TIMING_ENABLED` to `'false'` in `cy-cluster-common.ps1` after this debug session.

### Debug slice update (2026-06-01) - Profiling improvements added

Based on the analysis showing that both `try_flush_once` (block timing writes) and `periodic_snap_save` (autosnapshot writes) can cause significant blocking on Windows, we've added profiling to these operations:

- Added timing for `try_flush_once` in `crates/pwmd/src/block_timing.rs` - logs if flush takes more than 1ms
- Added timing for `periodic_snap_save` in `crates/pwmd/src/lifecycle.rs` - logs if autosnapshot save takes more than 10ms
- These operations were identified as potential sources of the stalls seen in the JSONL analysis
- Future runs will show timing logs to quantify the impact of these I/O operations

### Ticket status

- ticket status: `OPEN`
- result: `PARTIAL`
- files touched: `cy-cluster-common.ps1`, `docs/debug/20260607-v5-cy-gate-poll-optimization-experiments-debug.md`, `crates/pwmd/src/block_timing.rs`, `crates/pwmd/src/lifecycle.rs`, `crates/pwmd/src/state.rs`
- commands run: 120s benchmark × 2, full JSONL analysis (N=159 records), code profiling additions
