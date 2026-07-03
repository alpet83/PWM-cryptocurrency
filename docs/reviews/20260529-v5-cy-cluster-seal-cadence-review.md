# Review: CY cluster seal cadence (~2s/block)

**Date:** 2026-05-29  
**Agent:** pwm-review  
**Ticket:** `20260529-v5-cy-cluster-seal-cadence-review`  
**Scope:** review-only diagnosis; no product Rust edits  

---

## 1. Scope recap

Owner observed about 20 seconds for 10 sealed blocks on the CY proposer (`~2.0 s/block`) during the V5 CY cluster soak. This review classifies whether that cadence is a regression, an RFC16 cluster overhead, or expected current node behavior.

Reviewed artifacts:

- `docs/runbooks/v5-cy-cluster-precloseout-soak.md`
- `crates/pwmd/src/lifecycle.rs`
- `crates/pwmd/src/config.rs`
- `crates/pwmd/src/main.rs`
- `crates/pwmd/src/transport/peer_session/mod.rs`
- `cy-cluster-proposer.ps1`, `cy-cluster-attester.ps1`, `cy-cluster-common.ps1`
- available CY lab logs: `tmp/lab2-prop.log`, `tmp/lab-prop.log`
- scanner: `scripts/scan_pwmd_log_counters.ps1`

---

## 2. Requirements fit

| Question | Evidence | Conclusion |
|---|---|---|
| Expected seal cadence? | `crates/pwmd/src/lifecycle.rs:432` uses `tokio::time::interval(Duration::from_secs(2))`; `docs/MVP-checklist.md:127` documents background seal every 2s | Current theoretical max is one local seal attempt per ~2 seconds, both single-sealer and CY proposer |
| Is `~20s / 10 blocks` a regression? | `sealed height` logs only height 1 and every 10 blocks at `crates/pwmd/src/lifecycle.rs:504`; `seal_lease_renewed` logs show per-block cadence every ~2s | No: `~20s / 10 blocks` matches the hard-coded tick |
| Does RFC16 cluster add mandatory extra waits? | `run_cluster_gate` only reads existing round state and returns; no sleep in `crates/pwmd/src/lifecycle.rs:349`; attestation is produced/accepted in peer session code | In steady state no additional per-block wait beyond the next 2s tick; startup/missing quorum can skip ticks |
| Lease gate contribution? | `run_lease_gate` at `crates/pwmd/src/lifecycle.rs:279`; CY launcher uses `--seal-lease-backend process-local` | Per-tick check/renew only; not a 1s/block cost |
| `maybe_align_mid` contribution? | `maybe_align_mid` at `crates/pwmd/src/lifecycle.rs:268`; CY launchers do not pass `--debug-align-seal-mid-second` | Not active in CY launchers; no contribution |
| Snapshot persist / V5 apply cost? | Autosnapshot only on interval hits; V5 empty-block apply path is not in observed logs as a dominant delay | Not the cause of steady 2s cadence |

Acceptance criteria satisfied: cadence established, contributors classified, fix locus mapped, and recommendation below.

---

## 3. Style and module shape

No production code was changed by this review. The inspected code is straightforward but has one operator-visibility gap: `spawn_seal_loop` hard-codes the 2s interval without a named constant or CLI/config surface, while CY operators are interpreting wall-clock block rate from logs.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this review ticket).

---

## 4. Safety and behavior analysis

### Theoretical cadence

`spawn_seal_loop` drives sealing from a Tokio interval:

```rust
let mut iv = tokio::time::interval(std::time::Duration::from_secs(2));
loop {
    iv.tick().await;
    ...
    match g.chain.seal(txs) { ... }
}
```

This gives a steady-state upper bound of:

- one seal attempt per ~2 seconds,
- ~0.5 block/s,
- ~30 blocks/min,
- ~10 blocks in ~20 seconds.

The same upper bound applies to the CY proposer because `cy-cluster-proposer.ps1` does not pass any flag that changes the interval. There is no existing CLI option for block interval tuning.

### Cluster gate contribution

The RFC16 cluster path is a gate in front of seal, not a separate blocking wait inside the seal tick:

- `run_cluster_gate` checks round state for `next_h` and attester count.
- Missing proposal/attestation returns false and skips that tick.
- `attest_timeout_ms` default is 1000ms, but it is used to classify/report stale pending quorum; it does not sleep inside the seal loop.
- Cluster proposal/attestation exchange happens in `peer_session` (`cluster propose sent`, `cluster attest accepted`) before a future tick reaches the gate.

So a healthy CY cluster still seals at the 2s tick. An unhealthy or cold-start cluster seals slower by skipped ticks (`quorum_pending`, `quorum_timeout`), never faster.

### Lease gate contribution

`run_lease_gate` is checked before the cluster gate. In this CY lab the launcher explicitly uses:

```powershell
'--seal-lease-backend', 'process-local'
```

The available logs show lease renewals every ~2s with matching `tip_h`, which is a symptom of the seal loop tick, not the cause of the delay. File-backed lease CAS could add small overhead in other runs, but not enough to explain a deterministic 2s/block steady state here.

### Log evidence

`tmp/lab2-prop.log` shows startup cluster gating followed by steady 2s renew/tip progression:

| Time | Event |
|---|---|
| 06:29:30.084 | `seal_lease_acquired ... tip_h=0` |
| 06:29:30.084 | `quorum_pending missing_round_state height=1` |
| 06:29:32.093 | `quorum_timeout ... elapsed_ms=1492 limit_ms=1000` |
| 06:29:34.109 | `sealed height=1` |
| 06:29:36.098 | `seal_lease_renewed ... tip_h=1` |
| 06:29:38.107 | `seal_lease_renewed ... tip_h=2` |
| 06:29:40.127 | `seal_lease_renewed ... tip_h=3` |
| 06:29:50.160 | `seal_lease_renewed ... tip_h=8` |

Scanner output on available logs:

```text
lab2-prop.log: sealed height=1, seal_suppressed_by_cluster=2, quorum_timeout=1, quorum_pending=1, seal_lease_renewed=10
lab-prop.log:  sealed height=0, seal_suppressed_by_cluster=52, quorum_timeout=0, quorum_pending=52, seal_lease_renewed=51
```

Interpretation:

- `lab2-prop.log`: after initial cluster readiness, cadence is exactly tick-bound (~2s/block).
- `lab-prop.log`: cluster never reached usable round state; all ticks were suppressed at height 1. That is a cluster readiness issue, not a slow-block issue.

---

## 5. Tests / validation

Commands run:

```text
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/scan_pwmd_log_counters.ps1 tmp\lab2-prop.log tmp\lab-prop.log -PerFile
```

Static validation:

- Read `spawn_seal_loop`, `run_cluster_gate`, `run_lease_gate`, `maybe_align_mid` in `crates/pwmd/src/lifecycle.rs`.
- Read cluster CLI defaults in `crates/pwmd/src/main.rs`: `cluster-tx-catchup-ms=500`, `cluster-attest-timeout-ms=1000`.
- Read `ClusterCfg::default` in `crates/pwmd/src/config.rs`: same defaults.
- Read cluster launchers: proposer/attester enable RFC16 with `k=1,n=2` and process-local lease; no cadence flag.
- Read runbook: it documents health signals but not expected 2s/block cadence.

No live cluster was started by this review.

---

## 6. Verdict

**APPROVE / NO PRODUCT BUG** for the reported `~2s/block` cadence.

The observed `~20s for 10 blocks` is expected under the current hard-coded seal loop interval. It is not caused by V5 marks/inflation apply cost and is not an RFC16 quorum regression when the cluster is healthy.

Recommended action:

1. **Immediate docs fix (recommended):** update `docs/runbooks/v5-cy-cluster-precloseout-soak.md` to state that current CY and single-sealer dev cadence is ~2s/block, so 10 blocks in ~20s is healthy. Also document that `quorum_pending`/`quorum_timeout` means skipped ticks and can make cadence slower.
2. **Optional tuning ticket:** if the owner wants ~1s/block in CY lab, add a configurable seal-loop interval (for example `--seal-interval-ms`, default 2000) in `PwmdConfig` / `main.rs` / `spawn_seal_loop`. Keep production/default at 2s until economics and soak assumptions are recalibrated.
3. **No immediate cluster-code fix:** do not tune `cluster-attest-timeout-ms` for this symptom; it does not determine steady healthy block rate.

Fix locus if tuning is requested:

- `crates/pwmd/src/lifecycle.rs:432` — hard-coded interval.
- `crates/pwmd/src/config.rs` — add config field/default.
- `crates/pwmd/src/main.rs` — add CLI/env flag.
- `docs/runbooks/v5-cy-cluster-precloseout-soak.md` — document operator expectation.

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260529-v5-cy-cluster-seal-cadence-review.md
token_usage:
  source: estimate
  input: 18000
  output: 3000
  total: 21000
  confidence: medium
```

---

## 8. Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'P:\opt\docker\pwm-protocol'
git add 'docs/reviews/20260529-v5-cy-cluster-seal-cadence-review.md'
git add 'tasks/done/20260529-v5-cy-cluster-seal-cadence-review.json'
git commit -m 'docs(v5-cy): review cluster seal cadence'
```
