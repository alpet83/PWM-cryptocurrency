# Review: V2-9 cluster seal E2E (`tasks/20260510-v2-9-cluster-seal-e2e`)

Post-`pwm-testing` **PASS** verification for the cluster propose/attest wire path, transport loop integration, harness tests, CY lab scripts, and lease lab notes.

## 1. Scope recap

Ticket **`tasks/20260510-v2-9-cluster-seal-e2e.json`** (status **done**): close the gap where **`seal_suppressed_by_cluster`** reflected missing round state because **`ClusterPropose` / `ClusterAttest`** were not wired end-to-end on live peer TCP; stabilize lab **`seal_lease_cas_failed`** noise.

Claimed surfaces:

- **`crates/pwmd/src/transport/peer_session/mod.rs`**: `mk_cluster_prop`, `send_cluster_prop`, `mk_cluster_attest`, `route_cluster_stub` inbound propose handling, local mirror via `record_cluster_propose_originated`; unit tests **`cluster_prop_auto_ack`**, **`cluster_prop_mirror_send`**.
- **Transport loops**: **`inbound.rs`** (post-handshake + per-heartbeat fan-out), **`initial_exchange.rs`** and **`steady_session.rs`** (outbound seed path) calling **`send_cluster_prop`** in the same ordering bucket as cross-shard facts, account views, sync tx batch, sync tip.
- **Tests**: **`crates/pwmd/src/transport/tests/production.rs`** (wire gate scenarios).
- **Ops/docs**: **`cy-cluster-proposer.ps1`**, **`cy-cluster-attester.ps1`**, **`cy-cluster-common.ps1`**, **`issues-report.md`** (2026-05-11 lease lab entry).

Normative anchor: **`docs/rfc/16-validator-clone-attestation.md`** Variant A, attester-only quorum counting, and **раздел 8.1** — seal lease (S2) **orthogonal** to quorum membership (no conflation).

Linked sprint context: **`docs/plans/mvp_v2.md`** Sprint V2-9, **`docs/reviews/20260509-v2-9-rfc16-sprint-checklist.md`**.

## 2. Requirements fit

**Cluster wire E2E.** Proposer path builds a **`ClusterProposeWire`** at **`tip_h + 1`** with a deterministic **`vote_object`** derived from height and tip hash, gates on **`cluster_cfg`**, membership, and **`ClusterRole::Proposer`**, sends only to configured cluster peers whose hello exposes **`ClusterRole::Attester`**, and **mirrors** the round locally via **`record_cluster_propose_originated`** before write — so **`run_cluster_gate`** can see propose-side binding without relying on a loopback read of the same TCP write.

**Auto-attest.** On **`ClusterPropose`**, **`route_cluster_stub`** records the round under the remote proposer **`instance_id`**, then **`mk_cluster_attest`** signs the canonical attest payload for attesters; **`inbound.rs`** and **`steady_session.rs`** both **write `ClusterAttest` immediately** when **`route_cluster_stub`** returns **`Some`**. Symmetric handling on inbound acceptor and outbound seed steady loop matches the «extended peer wire» intent in RFC 16 planning notes.

**Trust and crypto.** Cluster frames require **`cluster_cfg.enabled`**, a **trusted peer** with **`cluster_attest_enabled`**, membership match, and **`cluster_role_ok`** for propose vs attest direction; **`ClusterAttest`** verifies Ed25519 over the bound **`cluster_sig_msg`**. This aligns with MVP expectations (same identity keys acceptable per RFC appendix notes).

**S2 vs quorum.** Heartbeat path still carries **lease** fields for observability; cluster quorum logic remains in **`HandshakeState.cluster_attest`** and **`run_cluster_gate`**. Lab switch to **`--seal-lease-backend process-local`** in CY scripts and the **`issues-report.md`** entry explicitly frame lease CAS contention as **HA/lab mechanics**, not a substitute for **`k-of-n`** — consistent with **RFC 16 раздел 8.1**.

**Residual gaps (explicit, not blockers for this slice):** automated tests still use **`record_cluster_propose_originated`** manually in several **`production.rs`** scenarios where the harness acts as the proposing TCP client; that remains a harness pattern, while real **`pwmd`** proposer sessions now mirror via **`send_cluster_prop`**. Full operator soak on physical hosts was not part of this review pass (relied on **`pwm-testing`** command transcript).

## 3. Style and module shape

- **Production / test naming:** Ran **`python scripts/check_rust_fn_name_segments.py`** on  
  `peer_session/mod.rs`, `inbound.rs`, `steady_session.rs`, `initial_exchange.rs`, `transport/tests/production.rs` — **`violations`** empty for all listed files (policy prod ≤4 segments, tests ≤5).
- **Module banners:** Touched modules retain minimal English **`//!`** where expected (`inbound.rs`, `steady_session.rs`, `initial_exchange.rs`, root **`peer_session/mod.rs`**).
- **Protocol semver:** This slice does not redefine **`PWM_PROTOCOL_VERSION`**; cluster payloads ride existing negotiated peer wire. No inconsistency flagged for this gate.

## 4. Safety

- **Trust boundary:** Cluster handling does not bypass **`trusted_peers`** / **`cluster_attest_enabled`** / membership checks.
- **Panics:** No new obvious **`unwrap`** hotspots in the reviewed cluster send/recv paths beyond established test harness patterns.
- **DoS / abuse:** **`send_cluster_prop`** runs on **every outbound heartbeat tick** (seed) and **every inbound heartbeat handling** cycle — repeat proposes are likely **idempotent** for round state but increase wire volume; treat as operational profile concern, not an immediate integrity bug.
- **Silent early exits:** **`send_cluster_prop`** returns **`Ok(())`** when remote lacks **`node_instance_id`**, is not a cluster member, or role mismatch — appropriate for «no work», but operators debugging mis-hello may see **no** «cluster propose sent» info log; observability nit only.

## 5. Tests

**Relied on orchestrator `pwm-testing` record:** `cargo fmt --check`, **`cargo check --workspace`**, **`cargo test -p pwmd`** (**PASS**), fn-segment lint (**PASS**, empty violations), **`snapshot_load`** bench compile-only (**PASS**).

**Code-reviewed additions:**

- **`cluster_prop_auto_ack`** (**`mod.rs`** tests): attester receives propose from trusted proposer peer → **`route_cluster_stub`** yields attest → signature verifies against local identity key.
- **`cluster_prop_mirror_send`**: proposer **`send_cluster_prop`** → round present at **`(tip_h+1, 0)`** with **`proposer_id`** **`node-a`**.
- **`production.rs`**: existing **`cluster_2of2_gate_ok`**, **`cluster_2of3_*`**, **`cluster_timeout_no_seal`**, **`cluster_bind_mismatch_no_seal`** continue to exercise **`run_cluster_gate`** with wire ingress.

**Missing / future:** dedicated integration asserting **automatic** mirror **without** manual **`record_cluster_propose_originated`** in harness (could reduce drift risk); optional soak proving bounded **`ClusterPropose`** resend volume under long heartbeat windows.

## 6. Verdict

**Approve with nits.**

**Nits (non-blocking; product/ops — no owner escalation unless you want wire-volume policy codified):**

1. Periodic **`send_cluster_prop`** on **every** heartbeat cycle may be noisy on WAN links; consider documenting expected cadence or a future «send only on bind change» optimization.
2. Harness vs prod: **`production.rs`** still calls **`record_cluster_propose_originated`** explicitly in multiple tests — acceptable for clarity, but a single «full prod path» TCP test would reduce conceptual drift.

No **request changes** items from naming lint, crypto verification, or RFC **8.1** separation.

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts:
  review_md: docs/reviews/20260510-v2-9-cluster-seal-e2e-review.md
token_usage:
  source: estimate
  input: 9500
  output: 4200
  total: 13700
  confidence: medium
```

## 8. Sprint-final glossary traceability

**Glossary:** обновлён **`docs/GLOSSARY.md`** — добавлены/уточнены операторские термины: автоматическая отправка **`ClusterAttest`** после входящего **`ClusterPropose`**, продовый путь **`send_cluster_prop`** и зеркало через **`record_cluster_propose_originated`**, лабораторный режим аренды **`process-local`** для скриптов CY; скорректирована формулировка «зеркала» propose под прод-путь.

---

**Verdict (one line):** Approve with nits — PASS gate.

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260510-v2-9-cluster-seal-e2e-review.md'
git add 'docs/GLOSSARY.md'
git add 'tasks/20260510-v2-9-cluster-seal-e2e.json'
git commit -m 'docs(v2-9): cluster seal e2e review, glossary, ticket delegation'
```
