# Review R3: steady-state CY cluster suppressions after propose-align + drift

**Date:** 2026-05-30  
**Agent:** pwm-review  
**Ticket:** `20260530-v5-cy-cluster-suppression-r3-review`  
**Scope:** review-only RCA; no product code edits  
**Prior reviews:** `docs/reviews/20260529-v5-cy-cluster-suppression-r2-review.md`, `docs/reviews/20260530-v5-param-derivation-coupling-audit-review.md`  

---

## 1. Scope recap

Owner reports that after propose-seal-align and drift/orphan-param cleanup, startup `missing_round_state` is improved but steady-state `seal_suppressed_by_cluster` remains too noisy during CY soak.

This pass checks the post-fix tree and separates three things that were previously conflated:

1. **Liveness/safety:** whether the proposer can seal safely once quorum ACK is present.
2. **Wire latency:** whether attester ACK can arrive before every seal tick.
3. **Log severity/volume:** whether every pending tick should be a WARN.

Reviewed files:

- `crates/pwmd/src/lifecycle.rs`
- `crates/pwmd/src/transport/peer_session/mod.rs`
- `crates/pwmd/src/transport/peer_session/seed/session/steady_session.rs`
- `scripts/scan_pwmd_log_counters.ps1`
- `docs/runbooks/v5-cy-cluster-precloseout-soak.md`

---

## 2. Current post-fix model

Current tree has the R2 fixes in place:

- Seal loop opens local round state before the gate:

```rust
if app.cluster_cfg.enabled && matches!(app.cluster_cfg.role, ClusterRole::Proposer) {
    record_cluster_prop_tick(&app).await;
}
if !run_cluster_gate(&app).await { ... }
```

- Wire propose is now early in the steady peer loop:

```rust
write heartbeat
send_cluster_prop
send_cross_shard_facts
send_account_views
send_sync_tx_batch
send_sync_tip
read/drain inbound
```

- `attest_timeout_ms` is derived from genesis seal cadence; for `blocks_per_hour=3600`, the model is typically:

```text
seal_interval_ms = 1000
attest_timeout_ms = 2000
heartbeat_interval_ms on proposer <= 1000
```

So the previous P0 causes from R2 are largely fixed. The remaining problem is a **pending-state polling/logging problem** plus possible **ACK latency** under sync/catch-up.

---

## 3. Suppression taxonomy R3

| Suppression | Current likely meaning | Severity | RCA |
|---|---|---|---|
| `missing_round_state` at startup | Peer loop/handshake not ready yet, or local tick failed to record because local member not in `cluster_members` | Acceptable only as startup burst | Misconfig if persistent after `record_cluster_prop_tick` should be possible |
| `attestations_missing` before timeout | Local round exists, but attester ACK not yet processed | Expected transient | Seal tick polls every `effective_ms`; wire propose/ACK/read path is asynchronous |
| `quorum_timeout` | ACK still absent after `attest_timeout_ms` | Real warning | Attester slow/offline, sync/catch-up pressure, wire/session issue |
| `binding_mismatch` / `invalid_signature` / `proposer_not_member` | Protocol/config error | Real warning | Not cadence noise |

Primary steady-state cause after R2 fixes:

**P1 — `run_cluster_gate` logs every pending tick as WARN, even when the condition is a normal transient before `attest_timeout_ms`.**

The key code path is `crates/pwmd/src/lifecycle.rs:465`:

```rust
if ack_n < app.cluster_cfg.quorum_k {
    if elapsed > attest_timeout_ms {
        warn!("... reason=quorum_timeout ...");
        return false;
    }
    warn!("... reason=quorum_pending detail=attestations_missing ...");
    return false;
}
```

This means a healthy height can produce one or more WARN lines while the attester ACK is still in flight. With `seal_interval_ms=1000` and `attest_timeout_ms=2000`, up to two pending gate checks per height are normal before timeout. If drift correction reduces `effective_ms`, the number of polls before the same timeout can increase.

---

## 4. Ratio guidance

The existing scanner counts total `sealed height=` log lines, not true block count. In current `pwmd`, `sealed height` is logged for height 1 and every 10 blocks, so the raw ratio is only approximate unless logs include every height from another sink.

Practical CY soak guideline:

| Metric | Healthy | Watch | Request changes |
|---|---:|---:|---:|
| `missing_round_state` after height >= 2 | 0 or startup-only | rare reconnect bursts | sustained while head advances |
| `quorum_timeout / sealed-log-count` | 0 | < 0.1 | repeated timeouts |
| `attestations_missing / sealed-log-count` | can be > 1 with current WARN-every-poll design | noisy but head advances | sustained high + slow/no head growth |
| `binding_mismatch`, `invalid_signature`, `proposer_not_member` | 0 | any occurrence needs inspection | repeated |

A better scanner metric is needed: count `head_delta` from first/last `sealed height=N` and compute suppressions per actual block delta, not per log line.

---

## 5. Logs checked

No fresh post-fix owner CY logs were found under recursive `tmp/**/*.log` or `logs/**/*.log`. Available logs are historical and mostly pre-fix:

```text
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/scan_pwmd_log_counters.ps1 tmp\lab2-prop.log tmp\lab-prop.log tmp\s3-12-8-node1.log tmp\s3-12-8-node2.log -PerFile
```

Results:

| File | Sealed log lines | Suppressions | Pending | Timeout | Notes |
|---|---:|---:|---:|---:|---|
| `tmp/lab2-prop.log` | 1 | 2 | 1 | 1 | old startup example |
| `tmp/lab-prop.log` | 0 | 52 | 52 | 0 | old no-round-state storm at height 1 |
| `tmp/s3-12-8-node1.log` | 8 | 0 | 0 | 0 | non-CY/old cluster-irrelevant sample |
| `tmp/s3-12-8-node2.log` | 34 | 0 | 0 | 0 | non-CY/old cluster-irrelevant sample |

Conclusion: workspace logs do not quantify the owner’s latest post-fix symptom. Static code path is enough to explain why steady pending WARNs can remain without proving a liveness bug.

---

## 6. Drift interaction

Drift correction measures wall time over `SEAL_DRIFT_WINDOW_BLOCKS=100` **successful local seals** and adjusts `effective_ms` by at most 1% per window.

Impact:

- It does not directly change quorum semantics.
- If actual sealing is slow because cluster gating often waits/skips, drift correction can gradually decrease `effective_ms` after 100 successful seals.
- Lower `effective_ms` means more gate polls per wall second, which can increase the number of transient `attestations_missing` WARNs before the same `attest_timeout_ms` expires.
- The cap is small, so this is an amplifier of log volume, not the root cause.

Recommendation: keep drift correction, but pending quorum logs should be rate-limited or demoted so faster polling does not look like a worsening cluster fault.

---

## 7. Assessment

### What is fixed

- Startup `missing_round_state` should be greatly reduced because local proposer now records round state on each seal tick.
- Wire `ClusterPropose` is no longer blocked behind sync batch in the outbound steady loop.
- Removed orphan cluster timing knobs reduce operator confusion.

### What remains

- `record_cluster_prop_tick` is local only. Attester ACK still requires wire send, remote processing, response write, and local read/drain.
- `run_cluster_gate` treats normal pre-timeout pending as WARN on every tick.
- Scanner cannot distinguish “pending WARN spam while head advances” from actual liveness degradation without better ratios.

### Verdict on symptom

If the latest owner logs are mostly `reason=quorum_pending detail=attestations_missing` while head advances and `quorum_timeout` is rare/zero, this is **acceptable cluster noise with bad severity/rate**, not a consensus/liveness blocker.

If `quorum_timeout` repeats or head stalls, then it remains a real transport/attester latency bug and should not be closed as noise.

---

## 8. Recommended minimal coding slice

**ID:** `20260530-v5-cy-cluster-pending-log-throttle-coding`

Minimal scope:

- `crates/pwmd/src/lifecycle.rs`
- `crates/pwmd/src/state.rs` or a small lifecycle-local state holder if needed
- `scripts/scan_pwmd_log_counters.ps1`
- `docs/runbooks/v5-cy-cluster-precloseout-soak.md`

Fix direction:

1. Keep fail-closed gate semantics unchanged.
2. Log `quorum_timeout`, invalid proposal, binding/signature/member errors as WARN.
3. Demote pre-timeout `quorum_pending detail=attestations_missing` to DEBUG or INFO, or log it once per `(height, round, reason)` with a repeat counter.
4. Keep `missing_round_state` as WARN only after startup/grace or once per height; otherwise INFO/DEBUG.
5. Extend scanner to report:
   - `missing_round_state`
   - `attestations_missing`
   - `quorum_timeout`
   - first/last sealed height and `head_delta`
   - suppressions per `head_delta`
6. Add lifecycle tests for log-debounce state if stateful; otherwise add a scanner test/sample if the change is script-only.

Explicit non-goals:

- no new consensus CLI knobs;
- no revert of genesis-derived cadence;
- no drift correction removal;
- no wire-format/API marker change.

---

## 9. Verification performed

Commands run:

```text
cargo test -p pwmd --lib lifecycle
cargo test -p pwmd --lib peer_session
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/scan_pwmd_log_counters.ps1 tmp\lab2-prop.log tmp\lab-prop.log tmp\s3-12-8-node1.log tmp\s3-12-8-node2.log -PerFile
```

Results:

- `cargo test -p pwmd --lib lifecycle`: PASS, 19 tests.
- `cargo test -p pwmd --lib peer_session`: PASS, 38 tests.
- Historical log scan: old suppressions reproduce known signatures; no fresh post-fix logs found.

---

## 10. Verdict

**PASS_WITH_REQUESTED_FOLLOWUP** — no new consensus bug proven from current workspace evidence, but the steady-state operator symptom is real enough to require one small logging/measurement follow-up before CY closeout confidence is clean.

The recommended coding work is not “make cluster less strict”; it is “stop treating every normal pre-timeout quorum-pending poll as a WARN storm, and measure suppressions against actual head growth.”

**Verdict line:** `PASS_WITH_REQUESTED_FOLLOWUP — quorum gate is fail-closed and aligned; pending WARN spam needs debounce/metrics follow-up.`

---

## 11. Participation / token estimate

```yaml
agent: pwm-review
result: PASS_WITH_REQUESTED_FOLLOWUP
artifacts: docs/reviews/20260530-v5-cy-cluster-suppression-r3-review.md
token_usage:
  source: estimate
  input: 26000
  output: 4300
  total: 30300
  confidence: medium
```

---

## 12. Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'P:\opt\docker\pwm-protocol'
git add 'docs/reviews/20260530-v5-cy-cluster-suppression-r3-review.md'
git add 'tasks/done/20260530-v5-cy-cluster-suppression-r3-review.json'
git commit -m 'docs(v5-cy): R3 review cluster suppression noise'
```
