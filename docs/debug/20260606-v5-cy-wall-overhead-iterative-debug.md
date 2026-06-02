# V5 debug: iterative CY smoke (200s) — wall overhead vs chain depth

Date: 2026-05-30  
Ticket: `20260606-v5-cy-wall-overhead-iterative-debug`

## Scope and constraints

- Goal: validate owner hypothesis that wall overhead scales with chain depth/state (fresh vs resumed), not just with surface suppression metrics.
- Method used in this slice: controlled CY 2-node smokes (`SmokeSeconds=220`) and post-run log replay with steady-cut (`cluster_attest_ready + 20s`).
- No product behavior edits were made.

## Variants executed

### V0 — control resumed

- Script: `scripts/cy_cluster_two_node_smoke.ps1 -SmokeSeconds 220 -ProposerLeadSeconds 8 -StatusWaitSeconds 180`
- Logs:
  - `logs/2026-05-30/pwmd-cy-proposer-162908.log`
  - `logs/2026-05-30/pwmd-cy-attester-162916.log`
  - `logs/2026-05-30/pwmd-peer-cy-proposer-162908.log`
- Snapshot startup (proposer): `total_ms=7805`

### V1-attempt-1 — fresh (invalid/incomplete)

- Cleaned state and rebuilt genesis, but run did not produce analyzable proposer markers in expected file pair.
- Logs retained for audit only:
  - `logs/2026-05-30/pwmd-cy-proposer-163224.log`
  - `logs/2026-05-30/pwmd-cy-attester-163232.log`
  - `logs/2026-05-30/pwmd-peer-cy-proposer-163224.log`

### V1 — fresh chain (valid retry)

- Full clean-state archive+wipe + `demo-devnet-start.ps1`, then same smoke parameters.
- `initial head_height=0` confirmed.
- Logs:
  - `logs/2026-05-30/pwmd-cy-proposer-163526.log`
  - `logs/2026-05-30/pwmd-cy-attester-163534.log`
  - `logs/2026-05-30/pwmd-peer-cy-proposer-163526.log`
- Proposer startup line: `pwmd startup phase: ready (no snapshot row / file for current backend)`.

## Result matrix (steady-cut scoring)

| Variant | tip band (steady) | T100_est (s) | drift% (latest 100-band) | pending p50/p95 (per 10 seals) | suppression struck% | RTT p50/p95 (ms) | snapshot startup | verdict |
|---|---|---:|---:|---:|---:|---:|---|---|
| V0 resumed | 34710→34780 (70 blocks) | **157.95** | n/a (no full 100-band in window) | **329.5 / 578.2** | **56.6%** (1 window) | 361.5 / 1563.1 | 7805 ms | severe overhead |
| V1 fresh | 30→110 (80 blocks) | **107.66** | **+11.70%** (`actual=111702`) | **45 / 301.6** | **1.14%** (1 window) | 569 / 876 | no snapshot row/file | near-target, much better |
| V1-attempt-1 | n/a | n/a | n/a | n/a | n/a | n/a | n/a | aborted/incomplete evidence |

## Key comparative findings

1. **Depth/state correlation confirmed (partial but strong).**  
   Resumed state (`~34k` tip) shows very high `T100_est` and pending pressure; fresh chain starts near nominal and remains far better under same harness.

2. **Primary delay source in resumed run = gate wait/poll pressure**, not timeout path.
   - V0 pending p50 `329.5` (vs V1 `45`) indicates massive extra poll cycles between seals.

3. **Suppression strike% is not a sufficient predictor alone.**  
   In V0 it is high in sampled window (56.6%), but prior runs already showed low strike windows with high wall cost. Pending distribution remains more diagnostic for residual wall overhead.

4. **Snapshot startup cost exists on resumed runs**, but cannot explain full steady overhead by itself.  
   V0 startup snapshot ~7.8s; steady per-100 overhead in resumed runs is much larger than one-time startup tax.

5. **Fresh run still has residual >0** (`+11.7%` in sampled band), so this is not purely “depth only”; it is **depth-amplified poll/gate behavior**.

## Hypothesis verdicts (for this iterative slice)

- `H(state-depth)` — “overhead scales with accumulated blocks/state”: **CONFIRMED (PARTIAL)**
  - Strong delta V0 vs V1 under same host/session and same smoke harness.

- `H(snapshot-only)` — “snapshot startup is dominant cause”: **REJECTED**
  - Contributes on resumed startup, but does not account for sustained high `T100` and pending pressure.

- `H(pre-timeout/pending dominates resumed overhead)`: **CONFIRMED**
  - Pending explosion in V0 vs V1 is the clearest separator.

- `H(struck_pct alone explains wall)`: **REJECTED**
  - Need combined interpretation with pending and gate timeline.

## Ranked delay sources for resumed ~34k chain (this slice)

1. **Gate/poll waiting (cluster quorum path)** — dominant
2. **Propose/attest loop churn under deep resumed state** — secondary but linked
3. **Startup snapshot load/recovery** — tertiary (startup tax)

## Recommendations (handoff candidates)

### pwm-coding (evidence-backed)

1. **Propose coalescing / duplicate propose reduction** (high priority)  
   Reduce repeated propose attempts per eventual sealed block under heavy resumed state.

2. **Attest wake-to-seal fast path** (high priority)  
   Minimize poll-gap from gate-ready to seal commit (especially under large pending tails).

3. **Observability split for gate wait vs strike** (high priority)  
   Keep `slots_struck` but add explicit per-window wait-ms estimate and pending-driven counters to avoid false “healthy” conclusions when strike is low.

### Not in scope here

- RFC16 cadence formula changes.
- Direct product fixes in this ticket.

## Artifacts

- Report: `docs/debug/20260606-v5-cy-wall-overhead-iterative-debug.md`
- Evidence bundle: `tmp/cy-iter-debug-20260530_193825/`
  - includes V0/V1/V1-attempt-1 proposer/attester/peer logs.

## Acceptance criteria coverage (current pass)

- [x] Labeled variants run and documented (`V0`, `V1`, plus one invalid attempt with reason).
- [x] Matrix includes `T100_est`, drift, pending, snapshot metadata, verdict.
- [x] Explicit answer on depth correlation provided with evidence.
- [x] Top delay sources ranked for resumed chain.
- [x] Debug doc and evidence directory delivered.
- [ ] Full `<=10` variant sweep not exhausted (stopped at decisive contrast for this iteration slice).
