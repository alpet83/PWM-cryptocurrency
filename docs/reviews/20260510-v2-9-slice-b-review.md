# Review: Sprint V2-9 — Slice B (2-node cluster waves)

**Ticket:** `tasks/20260513-v2-sprint9-rfc16-cluster-attestation.json`  
**Scope:** checklist §3 Slice B; notes `docs/reviews/20260510-v2-9-slice-b-wave-notes.md`; code `crates/pwmd/src/transport/tests/production.rs`, `pub(crate) run_cluster_gate` in `crates/pwmd/src/lifecycle.rs`  
**References:** `docs/rfc/16-validator-clone-attestation.md` §9–§11, `docs/reviews/20260509-v2-9-rfc16-sprint-checklist.md` Slice B  
**Reviewer:** pwm-review  
**Date:** 2026-05-10  

---

## 1. Scope recap

Slice B in the sprint checklist requires: happy 2-of-2 with reproducible seal/publish; negative no-quorum with operator-visible reason (`quorum_timeout` or equivalent); at least one §11-style fault inject; and a reproducible artifact (auto-test or runbook).

The wave adds three async tests in `production.rs` and relies on existing `run_cluster_gate` plus peer routing for cluster frames.

---

## 2. Requirements fit

### RFC §9 (timeouts and disagreement)

`run_cluster_gate` matches the intended shape: missing round state → `quorum_pending` with log; incomplete binding → `invalid_proposal`; proposer not in membership → `invalid_proposal`; insufficient attester ACKs → `quorum_pending` while within `attest_timeout_ms`, then `quorum_timeout` when `propose_opened_at_ms` is set and elapsed time exceeds the configured limit. Attesters are counted only among members and exclude the configured proposer id, consistent with the comment citing RFC §7 (k counts non-proposer attesters).

### RFC §10 (wire over trusted peer session)

Negative and fault tests use `process_inbound_socket`, `Hello` / `HelloAck`, and `ClusterPropose` / `ClusterAttest` frames—aligned with the locked checklist row (peer wire + capability / trusted session). No UDP or out-of-band path is introduced in this slice.

### RFC §11 (MVP checklist)

- Config / seal refusal without quorum: exercised indirectly via `run_cluster_gate` returning false when attestations are missing or mismatched (profile demands quorum when cluster is enabled).
- Signed attest payload: peer handler verifies signature against the peer’s hello pubkey before recording an attestation; the binding-mismatch test exercises the “discard, no seal” path with a real wire frame.
- Logs for quorum outcomes: code paths emit `seal_suppressed_by_cluster` with reasons consistent with §9 naming (`quorum_pending`, `quorum_timeout`, `invalid_proposal`); peer path logs `binding_mismatch` and related drops.

**Gaps (material):**

1. **Happy-path test vs checklist wording:** Checklist Slice B asks for 2-of-2 where, with quorum, the **block is sealed and reproducibly published**. The new tests stop at **`run_cluster_gate(&app_a)`** returning true or false—no assertion on `Chain::seal`, height advance, or publication/sync checkpoints.

2. **Happy-path wire coverage:** `cluster_2of2_gate_ok` sends `ClusterPropose` over TCP, but with the chosen `handshake_ib_client(app_b, &app_a)` topology the propose is **accepted on the attester** (`app_b`), while `run_cluster_gate` is evaluated on the **proposer** (`app_a`). The test then **manually** fills `app_a`’s `cluster_attest` round state and inserts attester entries with the placeholder value `"sig-ok"`—it does **not** send a **valid** `ClusterAttest` over the wire to the proposer’s inbound handler. So the “TCP round-trip propose + attest unlocks gate” story in the wave notes is **not** what the happy test actually proves; it proves **gate behavior given arbitrary in-memory state** plus a side propose delivery to the peer role. End-to-end parity with **`cluster_bind_mismatch_no_seal`** (proposer as inbound server, frames from attester) is missing for the positive case.

3. **Negative timeout vs log reason:** `cluster_timeout_no_seal` asserts only that the gate stays closed. It does not assert that the log reason is **`quorum_timeout`** (vs early `quorum_pending` if timing were tighter), so operator-visible §9.2 alignment is not locked in the test.

---

## 3. Style and module shape

- `run_cluster_gate` identifier length and structure are fine; orchestration uses clear early returns and logging.
- `production.rs` cluster tests use existing harness patterns; added helper `cluster_sig_line` is readable.
- **`python scripts/check_rust_fn_name_segments.py`** on `lifecycle.rs` and `production.rs`: **no violations** (prod ≤4 segments, tests ≤5).

Minor: the doc comment on `cluster_2of2_gate_ok` overstates wire coverage given the manual handshake mutation on the proposer.

---

## 4. Safety

- Cluster frames are gated on `cluster_cfg.enabled`, trusted peer, hello capability, and membership—consistent with closed membership (RFC §4.1).
- Attest signatures are verified on the wire path; manual test injection of `"sig-ok"` does not weaken production code but **does** mean the happy test is not a trust-boundary test for forged attest acceptance.
- No new unchecked hot-path panics identified in the reviewed gate; failure modes defer seal and log.

---

## 5. Tests

**Covered well:**

- Timeout-style no quorum (short `attest_timeout_ms`, propose only).
- Binding / vote-object mismatch with real attest frame and signature over the **wrong** vote object (fault inject aligned with §9.2 / §11).

**Missing or weak:**

- Full happy 2-of-2 over **proposer** inbound: propose (from attester client) already lands on proposer in other tests; add **valid** `ClusterAttest` on the same session (or a second session to the same proposer if that matches product topology) so `route_cluster_stub` inserts attesters without manual `HashMap` edits.
- Optional hardening: capture/trap logs or expose a small test hook to assert `reason=quorum_timeout` on the timeout case.
- Checklist “seal и публикуется”: if Slice B is interpreted strictly, a follow-on test that runs far enough to **`seal` once** under cluster (harness flag) would close the checklist row; currently **not** in scope of the reviewed diff’s assertions.

---

## 6. Verdict

**PASS_WITH_NITS** — `run_cluster_gate` and the negative/fault wire tests are substantively aligned with RFC §9–§11 and Slice B’s no-quorum and fault-inject rows. The **happy 2-of-2** slice is **not** fully satisfied as written: the positive test does not demonstrate wire-delivered, signature-verified quorum on the **proposer**, and **seal/publish** is not covered by assertions.

**Prioritized nits for pwm-coding**

1. Rework `cluster_2of2_gate_ok` so quorum is achieved via **ClusterAttest** handled on the **proposer** (same role/topology as the mismatch test), without manual `attesters` insertion.
2. Optionally assert **`quorum_timeout`** (or stable reason string) in `cluster_timeout_no_seal`.
3. If orchestrator treats checklist literally, add or scope a **seal-height** assertion behind cluster flags, or narrow the checklist row to “gate open” for this wave.

---

## 7. Participation / token estimate (orchestrator)

```json
{
  "agent": "pwm-review",
  "result": "PASS_WITH_NITS",
  "artifacts": ["docs/reviews/20260510-v2-9-slice-b-review.md"],
  "token_usage": {
    "source": "estimate",
    "input": null,
    "output": null,
    "total": 9500,
    "confidence": "low"
  }
}
```

**Optional ticket JSON suggestion:** extend `artifacts` with something like `"slice_b_review": "docs/reviews/20260510-v2-9-slice-b-review.md"` (or merge into `review_md` if you keep a single canonical review pointer per wave).

---

## 8. Git handoff

```powershell
# git-handoff
Set-Location 'P:\opt\docker\PWM-cryptocurrency'
git add 'docs/reviews/20260510-v2-9-slice-b-review.md'
git add 'tasks/20260513-v2-sprint9-rfc16-cluster-attestation.json'
git commit -m 'docs(v2-9): Slice B cluster wave review and traceability'
```
