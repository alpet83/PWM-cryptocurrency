# Review: V2-9 Slice A leg 2 — cluster attestation foundation (re-review)

**Ticket:** `tasks/20260513-v2-sprint9-rfc16-cluster-attestation.json`  
**Agent:** pwm-review  
**Date:** 2026-05-10  
**Scope claimed:** follow-up to `docs/reviews/20260510-v2-9-slice-a-leg1-review.md` §7 — attester-only **k** semantics, cryptographic attest verification, seal-time observability / reason vocabulary, hello capability gate for cluster frames, tests.

**Normative refs reviewed:** `docs/rfc/16-validator-clone-attestation.md` §4.1–§7, §9; sprint checklist `docs/reviews/20260509-v2-9-rfc16-sprint-checklist.md` (Slice A intent).

---

## 1. Scope recap

Leg 2 touches the pwm-coding artifact list in the ticket: `config.rs`, `lifecycle.rs`, `transport/peer_session/mod.rs`, `transport/peer_types.rs`, `transport/incoming_hello.rs`, `tests/transport_peer.rs` (inheritance from leg 1 for hello trust record fields). The slice remains **Slice A foundation**: static membership, in-memory round state, peer JSON wire, seal loop gate after lease, default-off cluster mode.

---

## 2. Requirements fit

**Resolved vs leg 1 §7 (material)**

- **RFC §7 semantics:** `quorum_k` is documented and validated as **attester-only** (`[1..=quorum_n-1]`), with defaults **`k=1`, `n=2`** matching the nominal “leader + one attester ACK” reading. `run_cluster_gate` excludes the recorded **proposer** from the attest count and requires `proposer_id` ∈ configured `members`. This removes the leg 1 arithmetic deadlock at default config.
- **RFC §3 / §4 — signed attestations:** `route_cluster_stub` verifies attest signatures with `pwm_core::crypto::verify` over a deterministic `height|round|vote_object|candidate_hash` message and the peer’s hello **Ed25519** public key; invalid hex, wrong length, or bad signatures drop the frame with `reason=invalid_signature`.
- **RFC §4.1 membership on ingress:** Frames require `TrustedPeer` presence, **non-empty** `instance_id` matching a configured member for attest/propose handling, and drop with structured reasons (`non_member`, `peer_instance_missing`, `untrusted_peer`). Attest from a non-member instance id is covered by a unit test.
- **Hello / capability gate:** Inbound cluster handling requires `peer.cluster_attest_enabled` from hello; otherwise `peer_hello_cluster_disabled`. Outbound dial mirrors local `cluster_cfg.enabled` into hello (`dial.rs` pattern), so capability negotiation remains consistent with RFC §10 “negotiated capability” direction.
- **§6.1 / observability conflation:** The prior misuse of **`attest_tx_lag`** for non-catch-up paths is **gone** from the reviewed tree; seal suppression uses `seal_suppressed_by_cluster` with **`reason=`** and **`detail=`** (for example `quorum_pending`, `invalid_proposal`, `binding_incomplete`, `proposer_not_member`, `attestations_missing`). This aligns much better with RFC §9 style, though wall-clock **`quorum_timeout`** is not yet tied to `T_attest` (expected later).

**Remaining gaps / partials (non-blocking for “foundation”)**

- **Signed payload vs wire:** `candidate_ref` is compared for attest binding but is **not** included in `cluster_sig_msg`. If operators rely on **VO2**-style split references, the attestation does not commit to `candidate_ref` today — document or extend the message format before calling attestations normatively complete for that profile.
- **Leader proposal authenticity:** Proposals are still accepted on **transport-trusted** session and role pairing, not a separately verified leader signature over the vote object. Acceptable for this lab slice if called out in operator assumptions (RFC allows MVP tranche with shared validator material; explicit proposer signatures are a later hardening).
- **`transport_peer.rs`:** Cluster-focused regressions are **not** obvious in the sections reviewed; the substantive new tests live under `peer_session` and `lifecycle` test modules. Consider consolidating HTTP hello tests for cluster capability if REST is part of the operator path.

---

## 3. Style and module shape

- English comments on cluster defaults and gate logic are clear; `//!` module discipline unchanged where already present.
- **`scripts/check_rust_fn_name_segments.py`** on the claimed leg 2 paths: **no violations** (production ≤4 segments; test ≤5).
- `route_cluster_stub` remains embedded in a large `peer_session/mod.rs` — same debt as leg 1, not escalated for this slice.

**Protocol version note (rubric):** `PWM_PROTOCOL_VERSION` unchanged; additive hello fields and new JSON variants remain consistent with leg 1 rationale — still worth a one-line operator note that both sides must deploy cluster wire code when enabled.

---

## 4. Safety

- **Default-off** path unchanged; untrusted / capability-mismatched peers cannot populate quorum tables via cluster frames under reviewed guards.
- **Signature verification** closes the leg 1 “string storage only” trust hole for attests on the hot path.
- **DoS / state:** Round map growth and `T_attest` enforcement remain future work; not regressions vs leg 1.

---

## 5. Tests

- **Config:** `cluster_cfg_accepts_2of2`, `cluster_cfg_accepts_2of3`, bad bounds rejection — aligned with attester-only **k** bounds.
- **Lifecycle:** `cluster_gate_2of2_ok`, `cluster_gate_2of3_ok` exercise **proposer exclusion** and member filtering (still using placeholder sig strings in state — acceptable for gate arithmetic).
- **Peer session:** `cluster_attest_unsigned_drop`, `cluster_attest_non_member_drop` cover verification and membership discard paths.

**Missing / nice-to-have:** explicit **`quorum_k=2`** with **`n=3`** gate test; positive-path attest acceptance test with a valid signature hex in `route_cluster_stub`; binding mismatch / missing propose already logged but could be asserted in tests.

---

## 6. Verdict

**PASS_WITH_NITS** — Leg 2 addresses the **leg 1 FAIL** drivers: **k** is **explicitly attester-only** with coherent defaults, **attests are cryptographically verified**, **membership and hello capability gates** apply before state updates, and **logging** separates reasons without abusing `attest_tx_lag`. Remaining items are **signature coverage for `candidate_ref`**, optional **k=2** gate coverage, and **timers** mapping to RFC §9 `quorum_timeout` when the slice moves beyond scaffolding.

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts:
  - docs/reviews/20260510-v2-9-slice-a-leg2-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 14000
  confidence: medium
```

---

## 8. Addendum — закрытие нит (код, 2026-05-10)

Ниты §2 / §5 (материальные gaps из первоначального leg2) закрыты в коде:

- **Подпись и `candidate_ref`:** сообщение для verify — пять строк (`height`, `round`, `vote_object`, `candidate_hash`, `candidate_ref` или пустая пятая строка). Тесты: `cluster_attest_cref_sig_ok`, `cluster_attest_cref_sig_mismatch`.
- **`k=2`, `n=3`:** `lifecycle::cluster_gate_2of3_k2_ok`.
- **`quorum_timeout` vs `T_attest`:** при `ack_n < quorum_k` и `propose_opened_at_ms` заданном: если `now - t0 > cluster.attest_timeout_ms` → `reason=quorum_timeout`; иначе `quorum_pending`. Время открытия раунда выставляется при приёме `ClusterPropose`.

Повторный полный прогон: `cargo test -p pwmd` — зелёный.

---

## 9. Git handoff (orchestrator)

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260510-v2-9-slice-a-leg2-review.md'
git add 'tasks/20260513-v2-sprint9-rfc16-cluster-attestation.json'
git commit -m 'docs(v2-9): Slice A leg2 cluster attestation re-review + ticket traceability'
```
