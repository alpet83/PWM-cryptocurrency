# Review: V2-9 Slice A leg 1 — cluster attestation foundation

**Ticket:** `tasks/20260513-v2-sprint9-rfc16-cluster-attestation.json`  
**Agent:** pwm-review  
**Date:** 2026-05-10  
**Scope claimed:** default-off `ClusterCfg` / CLI, `NodeHello` capability fields, `ClusterPropose` / `ClusterAttest` wire + `route_cluster_stub`, `run_cluster_gate` before seal, S2 vs quorum orthogonality (intent).

**Normative refs reviewed:** `docs/rfc/16-validator-clone-attestation.md` §5–§8.1, §10; `docs/reviews/20260509-v2-9-rfc16-sprint-checklist.md` §2–3.

---

## 1. Scope recap

The pwm-coding delegation lists: `config.rs`, `main.rs`, `handshake.rs`, `lifecycle.rs`, `transport/peer_session/wire.rs`, `transport/peer_session/mod.rs` (plus harness/wire_decode touchpoints visible in tree). This matches a **foundation** slice: configuration, additive handshake fields, new peer JSON wire variants, in-memory round state, and a seal-loop gate wired **after** `run_lease_gate`.

Checklist §2 (**LOCKED**): peer wire + capability; cluster only inside peer sessions; S2 vs quorum orthogonality — addressed at transport/capability and ordering/logging level. Slice A table rows on **full** §6.1 behavior, membership enforcement on wire, and cryptographically sound attestations are only **partially** covered (see gaps below).

---

## 2. Requirements fit

**Aligned**

- **Default-off:** `ClusterCfg::default().enabled == false`, CLI `--cluster-enabled` is opt-in (`SetTrue`), startup distinguishes enabled/disabled logging.
- **Wire + hello:** `NodeHelloCapabilities` gains additive `serde(default)` RFC16 fields (`cluster_attest_enabled`, role, members, optional k/n); dial path mirrors `app.cluster_cfg` when enabled. Matches RFC §10 preference for **negotiated capability** rather than a blind wire break for hello.
- **New messages:** `ClusterProposeWire` / `ClusterAttestWire` carry `(height, round, vote_object, candidate_hash, …)` consistent with RFC §5 vote-object framing for early slices.
- **Dispatch:** Inbound/steady peer paths route `ClusterPropose` / `ClusterAttest` to `route_cluster_stub` (when cluster disabled, frames are ignored — safe for legacy peers sending nothing).
- **Seal ordering:** `run_cluster_gate` runs **after** `run_lease_gate` in `spawn_seal_loop`, matching RFC §8 layering (lease first, then cluster quorum gate).
- **§6.1 config shape:** `tx_catchup_ms <= attest_timeout_ms` enforced in `validate_cluster_cfg`, consistent with bounded catch-up vs attest window.
- **n ≤ 3, k bounds:** validation enforces `n` in [2,3], `k` in [2, `n`], and `members.len() == n` — matches sprint checklist cluster size row.

**Gaps / mismatches (material)**

- **RFC §7 quorum semantics vs gate:** `run_cluster_gate` counts only entries in `ClusterRoundState.attesters` whose keys are in configured `members`. For the documented MVP pattern “two clones — leader + one standby attester”, only **one** `ClusterAttest` is expected from the peer; the leader issues `ClusterPropose`, not an attest counted here. With default CLI/config **`quorum_k = 2`** and **`quorum_n = 2`**, `ack_n` can reach **at most 1**, so the gate **never** opens unless every node also emits `ClusterAttest` (not specified in this slice) or **k** is reinterpreted. RFC §7 requires **explicit** wording whether **k** includes or excludes the leader; the code and config comments do not define this, and the current arithmetic does not match the usual reading of “2-of-2 = proposer binding + one attester ack”. **This blocks Slice A acceptance** until the counting model is fixed or documented and implemented coherently.
- **RFC §3 / §4 — signed attestations:** `route_cluster_stub` stores `signature` as a string but performs **no** cryptographic verification and does not bind the signature to `(H, R, vote_object)` under the deployment’s key policy. Acceptable only as **lab scaffolding**; must be explicit follow-up before any “conforming MVP” claim (RFC §11 item 2).
- **RFC §4.1 membership on ingress:** Counting filters by `members` at seal time, but **propose** is accepted from any `node_id` on the session; there is no check that the sender is the designated proposer or in `members`. Mis-routing or a malicious trusted peer could prime round state incorrectly (severity increases when verification is absent).
- **RFC §6.1 observability:** Multiple log lines attach `attest_tx_lag=true` for **binding mismatch**, **missing propose**, or **quorum_pending** — not for “deferred tx material” only. This **dilutes** the intended analytics key from §6.1 and the sprint checklist §3 row on structured logging.

**Protocol version (review rubric):** `PWM_PROTOCOL_VERSION` remains **`0.1.0`**; new hello fields use defaults (additive). `ClusterPropose` / `ClusterAttest` are **new tagged JSON variants** — peers on older builds will fail decode if they receive these frames (expected only when both sides opt into cluster traffic). Recommend a **short explicit rationale** in module/docs: “no major bump because mandatory negotiation is via hello defaults + capability bit; new frame types require code deploy” (per AGENT_PROMPT_review transport note).

---

## 3. Style and module shape

- English `//!` / comments on new types are present in `handshake.rs` and wire structs; overall consistent with surrounding code.
- **`scripts/check_rust_fn_name_segments.py`** on the claimed paths: **no violations** (prod max 4 segments).
- `route_cluster_stub` remains in a large `peer_session/mod.rs`; acceptable for leg 1, but later extraction may help readability (not a blocker).

---

## 4. Safety

- **Default-off** behavior is sound; cluster frames ignored when disabled.
- **Seal suppression** when cluster enabled but round/quorum missing avoids silent bypass — good direction for RFC §9 “no seal”.
- **Absence of signature verification** is the main trust-boundary gap for attestation acceptance.
- Round state in memory only (no persistence) — OK for foundation; watch for stale `(H,R)` growth in long runs (follow-up: bounds/TTL).

---

## 5. Tests

- Config tests: default off, bad bounds rejection, 2-of-2 config acceptance.
- `wire_decode` roundtrip for cluster payloads.
- **Missing for this slice:** unit/integration tests for `run_cluster_gate` quorum arithmetic (including leader-inclusive vs exclusive **k**), and negative tests for unsigned / wrong signer attest (once crypto is wired).

---

## 6. Verdict

**FAIL** — foundation wiring is in place and default-off safety is respected, but **quorum counting does not match RFC §7 / checklist Slice A** for the nominal 2-node topology at default **k=2**, and attestations are **not** cryptographically validated yet. Recommend **`pwm-coding`** address **k semantics + gate arithmetic** (and document leader-inclusive vs exclusive) before **`pwm-testing`** wave gates; treat signature verification and membership checks on propose/attest as **next-leg** hardening.

---

## 7. Suggested follow-ups (next coding leg)

1. Define and implement **explicit k-of-n semantics** (leader included or not per RFC §7); **fix `run_cluster_gate`** and defaults so 2-of-2 and 2-of-3 scenarios can pass by design.
2. **Verify** attest signatures against configured identity / peer keys; reject non-members and wrong binders before counting toward quorum.
3. **Reserve `attest_tx_lag` (or §6.1 key)** strictly for deferred tx material; use distinct reason codes for mismatch / missing propose / quorum timeout (RFC §9 vocabulary).
4. Optionally **require** matching `cluster_attest_enabled` (and role hints) on peer hello before honoring cluster frames between operators who enable the flag.
5. Extend tests covering gate + wire stub behavior.

---

## 8. Participation / token estimate

```yaml
agent: pwm-review
result: FAIL
artifacts:
  - docs/reviews/20260510-v2-9-slice-a-leg1-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 12000
  confidence: medium
```

---

## 9. Git handoff (orchestrator)

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260510-v2-9-slice-a-leg1-review.md'
git add 'tasks/20260513-v2-sprint9-rfc16-cluster-attestation.json'
git commit -m 'docs(v2-9): Slice A leg1 cluster attestation review + ticket traceability'
```
