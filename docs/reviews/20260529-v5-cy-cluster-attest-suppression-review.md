# Review: CY cluster attest suppressions after faster seal cadence

**Date:** 2026-05-29  
**Agent:** pwm-review  
**Ticket:** `20260529-v5-cy-cluster-attest-suppression-review`  
**Scope:** review-only RCA; no product edits  
**Primary context:** `20260529-v5-genesis-seal-cadence-align-coding` changed seal cadence from hard-coded 2s to genesis-derived `3_600_000 / blocks_per_hour`.

---

## 1. Scope recap

Owner reports that the genesis-derived cadence fix improved block rate (~1s/block for `blocks_per_hour=3600`) but increased `seal_suppressed_by_cluster` warnings on the CY proposer.

This review checks whether the new suppressions are expected after the cadence change, where the proposer/attester timing mismatch comes from, and what follow-up coding scope should fix it without adding new operator CLI consensus knobs.

Reviewed:

- `tasks/done/20260529-v5-genesis-seal-cadence-align-coding.json`
- `docs/reviews/20260529-v5-cy-cluster-seal-cadence-review.md`
- `docs/reviews/20260513-cy-lab-sync-vs-cluster-priority-review.md`
- `crates/pwmd/src/lifecycle.rs`
- `crates/pwmd/src/config.rs`
- `crates/pwmd/src/main.rs`
- `crates/pwmd/src/transport/peer_session/mod.rs`
- `crates/pwmd/src/transport/peer_session/seed/session/steady_session.rs`
- `cy-cluster-proposer.ps1`, `cy-cluster-attester.ps1`, `cy-cluster-common.ps1`
- available historical logs: `tmp/lab2-prop.log`, `tmp/lab-prop.log`

---

## 2. Requirements fit

| Requirement | Evidence | Result |
|---|---|---|
| Correlate seal interval with cluster timeouts | `seal_interval_ms(3600)=1000` in `crates/pwmd/src/lifecycle.rs:41`; cluster defaults remain `tx_catchup_ms=500`, `attest_timeout_ms=1000` in `crates/pwmd/src/config.rs:40` and CLI defaults in `crates/pwmd/src/main.rs:316` | PASS — current timing is at the edge: timeout equals one seal tick |
| Explain proposer/attester desync | `run_cluster_gate` expects round state for `tip+1` at each seal tick; `send_cluster_prop` is emitted by peer steady loop, not by the seal tick; steady loop sleeps on heartbeat interval default 1500ms | PASS — proposal cadence can be slower than new 1000ms seal cadence |
| Quantify from logs or recipe | Available logs are pre-cadence-fix historical, but show the same signatures: `quorum_pending`, `missing_round_state`, `quorum_timeout`, scanner counts included below | PARTIAL — no fresh owner logs under `tmp/`, but static timing is sufficient to explain symptom |
| Recommend genesis/code-scoped fix, no new CLI tuning | Recommendations below derive cluster timing from seal cadence and decouple proposal from heartbeat; no `--seal-interval-ms` style knob | PASS |
| No product edits | This review writes only `docs/reviews/*` and ticket state | PASS |

---

## 3. Style and module shape

No production code was changed by this review.

Naming check on reviewed production files passed:

```text
python scripts/check_entity_name_segments.py crates/pwmd/src/lifecycle.rs crates/pwmd/src/transport/peer_session/mod.rs crates/pwmd/src/config.rs crates/pwmd/src/main.rs
```

Result: no naming-policy violations.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this review ticket).

---

## 4. Root-cause analysis

### 4.1 Timing after genesis-derived seal cadence

The coding ticket changed the seal loop from a fixed 2s tick to genesis-derived cadence:

```rust
seal_interval_ms = 3_600_000 / blocks_per_hour
```

For the current CY genesis expectation (`blocks_per_hour=3600`), the proposer attempts a seal every ~1000ms.

Cluster defaults did **not** change with that cadence:

| Parameter | Current source | Default | Relationship to 1s seal |
|---|---|---:|---|
| `seal_interval_ms` | genesis `blocks_per_hour` via `lifecycle::seal_interval_ms` | 1000ms for BPH=3600 | baseline block tick |
| `cluster_tx_catchup_ms` | CLI/config default | 500ms | half of one seal tick; appears only validated/logged in current grep scope |
| `cluster_attest_timeout_ms` | CLI/config default | 1000ms | exactly one seal tick; little/no slack |
| `transport_heartbeat_interval_ms` | CLI/config default | 1500ms | slower than one seal tick |
| `transport_heartbeat_timeout_ms` | CLI/config default | 4500ms | write/read timeout only |

This makes the cluster quorum path edge-triggered: the proposer can ask the cluster gate for `height=tip+1` every 1s, while the transport peer loop that emits `ClusterPropose` runs at a default 1.5s heartbeat cadence.

### 4.2 Proposal cadence is tied to peer heartbeat, not the seal tick

`spawn_seal_loop` checks `run_cluster_gate` every seal tick:

```rust
if !run_cluster_gate(&app).await {
    continue;
}
```

`run_cluster_gate` then requires already-existing round state for `tip+1`:

```rust
let Some(state) = hs.cluster_attest.rounds.get(&(next_h, round)) else {
    warn!("seal_suppressed_by_cluster reason=quorum_pending detail=missing_round_state ...");
    return false;
};
```

But round state is opened by `send_cluster_prop`, which is called inside the seed steady peer session loop after:

1. `sleep(heartbeat_interval_ms)`
2. heartbeat write
3. cross-shard facts
4. account views
5. sync tx batch
6. then `send_cluster_prop`
7. later sync tip
8. then inbound read/drain

So with default settings, a healthy proposer may try to seal every 1000ms while cluster proposal sending happens every ~1500ms plus preceding work. That mismatch was mostly hidden when seal cadence was 2000ms.

### 4.3 Timeout equals one tick and can be reported before useful slack exists

`cluster_attest_timeout_ms=1000` is used in `run_cluster_gate` to classify missing attestations:

```rust
if now_ms.saturating_sub(t0) > app.cluster_cfg.attest_timeout_ms {
    warn!("... reason=quorum_timeout ... elapsed_ms={} limit_ms={}");
    return false;
}
```

At 1s seal cadence, the next gate check can happen around the timeout boundary. Any transport scheduling delay, sync work, Windows process scheduling, or duplicate session churn can push elapsed time over 1000ms. This explains elevated `quorum_pending` / `quorum_timeout` even when the attester is not fundamentally broken.

### 4.4 `tx_catchup_ms` is currently not an effective runtime lever

A grep for `tx_catchup_ms` in `crates/pwmd/src` shows it is configured, validated (`<= attest_timeout_ms`), and logged, but not used in the cluster proposal/attestation path reviewed here. Therefore it cannot currently protect the attester round trip from a faster seal tick.

This is important because the operator-facing config suggests two timing windows, but only `attest_timeout_ms` appears to affect `run_cluster_gate` behavior.

### 4.5 Historical log correlation

No fresh post-cadence-fix owner logs were found under `tmp/` or `logs/` during this review. Available historical proposer logs still show the exact failure modes:

```text
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/scan_pwmd_log_counters.ps1 tmp\lab2-prop.log tmp\lab-prop.log -PerFile
```

Results:

| File | sealed height | suppressions | pending | timeout | Interpretation |
|---|---:|---:|---:|---:|---|
| `tmp/lab2-prop.log` | 1 | 2 | 1 | 1 | startup round-state/attestation delay, then steady progress |
| `tmp/lab-prop.log` | 0 | 52 | 52 | 0 | no usable round state at height 1; cluster readiness failure |

The newer symptom is plausible even without fresh logs because the static timing ratio changed from:

- old: `seal_interval=2000ms`, heartbeat/propose default `1500ms`, attest timeout `1000ms`
- new: `seal_interval=1000ms`, heartbeat/propose default `1500ms`, attest timeout `1000ms`

The old ratio gave proposal generation more room between seal ticks; the new ratio allows the seal gate to outrun proposal/attest round-state creation.

---

## 5. Safety

- The suppressions are fail-closed: `run_cluster_gate` returns `false`, so the proposer skips sealing rather than sealing without quorum.
- The safety risk is availability/liveness and operator noise, not consensus safety.
- Existing CLI knobs `--cluster-tx-catchup-ms` and `--cluster-attest-timeout-ms` remain runtime-tunable. That is acceptable as existing debug/ops surface, but it conflicts with the owner direction that consensus timing should be genesis/code-locked. Follow-up should either derive these values from genesis seal cadence or constrain CLI overrides to non-consensus lab profiles.
- The transport loop still multiplexes sync and cluster frames on the same steady session. As noted in the 2026-05-13 review, catch-up can delay cluster frames and make `attest_timeout_ms=1000` too strict under load.

---

## 6. Tests / validation

Executed:

```text
python scripts/check_entity_name_segments.py crates/pwmd/src/lifecycle.rs crates/pwmd/src/transport/peer_session/mod.rs crates/pwmd/src/config.rs crates/pwmd/src/main.rs
cargo test -p pwmd --lib lifecycle
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/scan_pwmd_log_counters.ps1 tmp\lab2-prop.log tmp\lab-prop.log -PerFile
```

Results:

- Naming policy: PASS, no violations.
- `cargo test -p pwmd --lib lifecycle`: PASS, 14 tests.
- Log scan: historical logs show `quorum_pending` / `quorum_timeout` signatures; no fresh post-fix logs were available in the workspace.

Missing validation:

- No live CY cluster was started in this review.
- No fresh owner log with `seal_cadence genesis_blocks_per_hour=3600 seal_interval_ms=1000` was present under `tmp/` or `logs/` for precise suppression-rate measurement.

---

## 7. Verdict

**REQUEST_CHANGES for cluster timing alignment before V5 CY closeout.**

The genesis-derived 1s seal cadence is correct per owner direction, but the RFC16 cluster timing model was not aligned with it. The current design can produce frequent suppressions because the seal gate checks every 1s, while proposals are emitted by a heartbeat-driven peer loop at 1.5s default and `attest_timeout_ms` is only 1s.

Recommended coding ticket:

```text
20260529-v5-cy-cluster-attest-timing-align-coding
```

Minimal scope:

- `crates/pwmd/src/lifecycle.rs`
- `crates/pwmd/src/config.rs`
- `crates/pwmd/src/main.rs`
- `crates/pwmd/src/transport/peer_session/seed/session/steady_session.rs`
- `crates/pwmd/src/transport/peer_session/mod.rs`
- `docs/runbooks/v5-cy-cluster-precloseout-soak.md`

Fix direction:

1. Derive cluster timing from genesis seal interval/code, not from new operator CLI knobs.
2. Ensure proposal cadence is not slower than seal cadence: either decouple `ClusterPropose` from heartbeat or make cluster-enabled proposal sending run at a cadence derived from `seal_interval_ms`.
3. Make `attest_timeout_ms` comfortably larger than one seal tick under cluster mode, for example a code-derived minimum like `max(2 * seal_interval_ms, transport_heartbeat_interval_ms + transport jitter budget)`.
4. Make `tx_catchup_ms` meaningful or remove it from the operator mental model; currently it is configured/logged but not used in the reviewed cluster path.
5. Add tests or an integration harness assertion for BPH=3600: proposer should not emit persistent `missing_round_state`/`attestations_missing` while head advances with a healthy attester.

No new `--seal-interval-ms` or equivalent runtime cadence override should be added.

---

## 8. Participation / token estimate

```yaml
agent: pwm-review
result: FAIL
artifacts: docs/reviews/20260529-v5-cy-cluster-attest-suppression-review.md
token_usage:
  source: estimate
  input: 26000
  output: 4200
  total: 30200
  confidence: medium
```

---

## 9. Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'P:\opt\docker\PWM-cryptocurrency'
git add 'docs/reviews/20260529-v5-cy-cluster-attest-suppression-review.md'
git add 'tasks/done/20260529-v5-cy-cluster-attest-suppression-review.json'
git commit -m 'docs(v5-cy): review cluster attest suppression'
```
