# RFC 0016: Validator Clone Cluster Attestation (Variant A)

**Status:** Draft  
**Version:** 0.4.8 — §8.2: seal-role derivation vs cluster attester; MVP attest path vs §6 (informative)  
**Depends on:**

- `docs/WHITE_SPEC_v0.md`
- `docs/adr/0001-consensus-and-node-stack.md`
- `docs/rfc/8-shard-runtime-identity-and-peering.md`
- `docs/rfc/4-validators-and-finality.md`
- `docs/reviews/20260509-single-sealer-failover-design.md` (deployment profiles, lease/fencing context)
- `docs/reviews/20260511-single-sealer-S3-cluster-consensus-design.md` (S3 option matrix; Variant A selection)

**Related:** Slice **S3** in `tasks/20260509-single-sealer-failover-profiles.json` (*optional cluster consensus*).

---

## 1. Abstract

This RFC specifies **Variant A**: a **leader–attester** protocol among **clone processes** that share the **same validator identity** (same signing key in the validator set). One designated node **proposes** a **candidate block** (or a deterministic binding to it); **other clones** in the configured cluster **attest** that the candidate satisfies local validity checks **before** the candidate may be **sealed** into the local chain view.

This document is **normative for implementation** only after **explicit adoption** (protocol version bump and operator opt-in profile). Until then it is a **design contract** for MVP-sized slices.

**Variant A does not replace** failover semantics ([RFC 8], lease/S2): it addresses **agreement on candidate content** among clones, not **HA exclusivity** of who may run the seal loop.

---

## 2. Terminology

| Term | Definition |
|------|------------|
| **Validator clone** | A `pwmd` runtime instance configured to act for the **same** validator identity (same key material / `validator_identity_hash`) as other clones. |
| **Cluster membership** | The **explicit** set of clone identities (`node_instance_id` and/or long-term peer keys) allowed to participate in attest rounds for this validator. **Attesters MUST NOT be “any peer on the shard”.** |
| **Leader (round)** | The clone that **constructs** the candidate block for height `H` (and round `R` if used). |
| **Candidate** | A block (or a binding commitment to it — see §5) proposed for height `H`. |
| **Attestation** | A **cryptographically signed** statement by a clone that it accepts the candidate under §6 rules. Unsigned “OK” messages are **out of scope** and MUST NOT be treated as attestations. |
| **Quorum** | Predicate over attestations: e.g. **k-of-n** accepting clones from membership (see §7). |
| **Commit / seal** | Application of `Chain::seal` (or equivalent) to produce the next sealed block **after** Variant A rules succeed. |
| **Round identity** | Pair `(H, R)` uniquely identifying a consensus attempt for height `H`. |
| **Relay pool** | The larger set of nodes that may participate in **gossip / relay** toward the cluster (scaling, visibility). **Not** identical to the attest quorum set. |
| **Active quorum set** | The **k** (or **k-of-n**) clones that count toward Variant A attest for a round — a **subset** of membership and typically a **subset** of the relay pool when the pool is larger than **n**. |

### 2.1 Single-leader candidate source (normative)

Variant A **does not** define a protocol where each clone **independently assembles** its own candidate block from its **local mempool** and peers **compare or merge** competing blocks.

**Normative model:**

1. Exactly **one leader-chosen candidate** exists per `(H, R)` attempt (constructed by the leader from the leader’s tx selection / mempool view).
2. **Attesters** obtain **that** candidate (full block or header+body per profile) and perform §6 **validity checks on the leader’s artifact**.
3. Attesters **sign acceptance** of the **leader’s** `vote_object`, not a hash of a **self-built** alternative block at the same height.

**Consequence:** divergent mempools between clones affect **whether** an attester accepts the leader’s candidate (e.g. unknown tx in body → may reject as invalid or fetch deps — profile-defined), **not** a competing “my assembled block vs yours” collation step within baseline Variant A.

A future extension (**out of scope** for this RFC’s baseline) could define **multi-proposer** rounds or leader rotation with competing proposals; that is **not** the MVP Variant A path described here.

---

## 3. Goals and Non-Goals

### 3.1 Goals

1. Prevent a single buggy or compromised **leader** clone from sealing an invalid candidate **without** cross-check by **other trusted clones** when the profile is enabled.
2. Define a **minimal** message and state machine: propose → attest → (optional prepare) → seal.
3. Define **disagreement and timeout** behavior sufficient for operators and tests (§9).
4. Keep **fork-choice** for the rest of the network unchanged unless a future RFC explicitly extends header semantics.

### 3.2 Non-Goals

1. **Byzantine fault tolerance** among clones with formal `2f+1` proofs — **deferred** (see Variant F in the S3 design review).
2. Replacing **PoA / expected proposer** rules of the chain with a new global consensus algorithm.
3. Using attest quorum as a substitute for **lease / fencing** (S2): exclusivity and candidate agreement remain **orthogonal** (see §8).
4. Attestation by **foreign** shard peers or unauthenticated endpoints.
5. **Parallel candidate assembly** by non-leader clones with subsequent **block-vs-block** reconciliation as the **baseline** Variant A workflow — **excluded** (see §2.1).

---

## 4. Trust Model

### 4.1 Membership authority

All attesters MUST be members of a **closed membership set** configured **out-of-band** for MVP:

- Operator-supplied list of `node_instance_id` values **and** authenticated transport identity (e.g. Ed25519 **peer** keys bound in hello), **or**
- A shared operator secret used only for lab setups (**not** recommended for production).

**Sybil rule:** If membership is underspecified, implementors MUST default to **reject attestations** from non-members.

### 4.2 Signing keys

Each attestation MUST be signed by a key that proves **clone identity** as agreed in deployment docs (typically the same **validator signing key** or a **dedicated attestation sub-key** derived deterministically and documented). Using **distinct attestation keys** per clone is allowed if their mapping to validator identity is **one-to-one** and configured.

**MVP slice (owner agreement):** it is **sufficient** that **proposer and attesters share the same validator-identity key material** (aligned public keys / same logical validator) for the first implementation tranche; dedicated attestation sub-keys remain optional (see **Appendix B.3**).

---

## 5. Vote Object (what is attested)

Implementations MUST fix one of the following **vote objects** per deployment (documented in operator guide):

| ID | Vote object | Notes |
|----|----------------|-------|
| **VO1** | `candidate_block_hash = Hash(canonical_serialized_candidate)` | Recommended default: compact, unambiguous. |
| **VO2** | Hash of **header fields** only (`prev_hash`, height, `tx_root`, `state_root`, timestamp bounds, …) | Use when block body is transferred separately. |

The leader MUST advertise `(H, R, vote_object)` in the **proposal** phase. Attesters MUST verify the candidate matches `vote_object` before signing.

**Replay protection:** attestations MUST include `(H, R, vote_object)` **inside** the signed payload (or a collision-resistant commitment chain).

---

## 6. Validity predicate (local checks before attest)

Checks apply to **the leader’s candidate** (§2.1), not to a separately assembled local candidate.

Before signing an attestation, each clone MUST run **at least**:

1. **Structural / consensus rules** applicable to a proposed block at height `H` (same checks the leader would run before seal today).
2. **Tip consistency**: parent hash matches expected head **for this clone’s view**, OR documented reconciliation rule if lagging (§9.4).
3. **Policy / tx rules** unchanged from normal block production.

**Explicit non-requirement:** Clones are **not** required to share **identical mempool state** with the leader. A standby may still **accept** the leader’s candidate if every tx in the body passes local validation (and dependencies are available or pulled per profile). If the attester **cannot** validate the leader’s body (e.g. missing tx graph), it MUST **reject** attest (reason documented) — **not** substitute its own assembled block as the vote object. **MVP default for ordering gaps** — **§6.1**.

### 6.1 MVP default: tx material missing at first sight (informative + SHOULD)

**Context:** The attester must validate the **leader’s** candidate body. A tx might be **logically** valid but **not yet present** in the attester’s local buffer when the proposal arrives (e.g. relay ordering). In a **tight local cluster** with a single fan-out / broadcast path, the pattern “**candidate first, required tx much later**” is expected to be **rare**; the **preferred** behavior is still to **ingest and validate** the tx when it appears, then attest if all checks pass within the attest window.

**MVP policy (SHOULD for default profile):**

1. On proposal, if validation is blocked only by **missing tx bytes or dependencies**, the attester SHOULD **wait** a **bounded** interval `T_tx_catchup ≤ T_attest` (config or fraction of `T_attest`) while **accepting** incoming relay/gossip for those ids from cluster peers and the leader.
2. If the material arrives in time, run full validation; **MUST** emit a **structured log** (analytics) that the attester **completed attest path after deferred tx material** (suggested field: `attest_tx_lag` or `candidate_before_tx` = true) — for capacity and incident review, not an error by itself.
3. If after `T_tx_catchup` the body still cannot be validated, **reject** attest with a documented reason (e.g. `missing_tx_after_catchup`) per the baseline rule above.

**Rationale for logging:** even when the scenario is **unlikely** locally, the event is **useful** for analytics (mempool sync, hop counts, future broadcast design).

---

## 7. Quorum policy

Parameters (operator-configured, bounds enforced):

- `n` = membership size (≥ 2 for Variant A to make sense).
- `k` = required attestations **including** leader or excluding leader — **must be explicit**.

**Default MVP recommendation:** **2-of-2** when exactly two clones exist (leader + one standby attest); OR **2-of-3** when three clones exist (leader + two distinct attest paths). Implementations SHOULD refuse ambiguous configs (`k > n`, `k < 2`).

### 7.1 Quorum vs relay pool (normative)

The **active quorum set** (who must attest for a round) is an **incoming subset** of a potentially larger **relay pool**: extra nodes may exist for relay scaling and future broadcast layers without every node being an attester every round. Policies that select **which** attesters are active for height `H` (rotation, epochs) are **profile-defined** and MUST NOT be conflated with **S2 seal lease** (see §8.1).

### 7.2 MVP stabilization scope

**Implementation priority:** stabilize the protocol on a **maximum three-node** consensus deployment (2-of-3 or equivalent) before expanding **n** or relay-pool-only scaling. Larger **k-of-n** (e.g. 7–9 attesters) remains a **later** profile once the three-node path is proven in test and ops.

---

## 8. Interaction with failover and lease (S2)

Variant A **layers on** active/standby and lease:

| Question | Normative MVP stance |
|----------|----------------------|
| Who may **propose**? | Typically the clone holding **active seal role** and valid lease when S2 is enabled; **MUST** be documented. |
| Who may **attest**? | Any **standby** clone in membership with sync’d tip sufficient to validate §6; **or** only peers that passed handshake — **profile-defined**. |
| Who performs **final seal**? | **One** designated **committer** (often = leader) after quorum; others MUST NOT seal the same `(H,R)` candidate. |

**Conflict:** If lease says “only A may seal” but quorum requires attest from B and B is down, result is **no seal** until timeout/policy — **not** silent bypass of lease.

### 8.1 Orthogonality: S2 seal lease vs quorum membership (normative)

These MUST be kept distinct in specs and operator docs:

| Axis | Role |
|------|------|
| **S2 (and external) seal lease** | **Who may execute seal** for this validator identity — exclusivity / fencing for **production** HA paths. |
| **Quorum membership / rotation** | **Who must attest** before a candidate is committed — **Variant A** agreement among clones. |
| **Relay pool** | Broader relay/gossip participation — **not** a substitute for attest quorum and **not** the same as lease holder. |

Lifecycle of quorum slots (rotation as nodes join/leave) is **not** the same abstraction as renewing a **seal lease**; implementations SHOULD use different metric/log prefixes to avoid operator confusion.

### 8.2 Implementation alignment notes (informative, pwmd / lab)

These notes capture **deployment reality vs** normative §6–§8 and guide **convergent refactors** (2026-05 review: `docs/reviews/20260513-cluster-role-rfc-alignment-review.md`).

**Seal loop vs cluster role (Variant A lab / production shape):** Non-proposer cluster clones (**`ClusterRole::Attester`**) **MUST NOT** execute the local periodic seal path that competes with the designated committer (**§8**, final row). Relying on a separate debug-only flag for that invariant is **error-prone**: historically `--debug-disable-seal-loop` was introduced for **out-of-cluster / harness** replay modes and remained in lab scripts after fixes. **Target product behavior:** when **`cluster.enabled`** and local role is **`Attester`**, the implementation **derives** standby / no competing local seal **from role** (after any explicit `seal-role` validation), so lab launchers need not pass `--debug-disable-seal-loop` solely for attesters. The flag remains valid for **non-cluster** followers / replay-only nodes where **`ClusterRole`** is unset. **Proposer** nodes are unaffected; a later profile may narrow or forbid the flag on proposer starts.

**Attestation vs §6 completeness (current MVP wire path):** Normative **§6** requires local checks on the leader’s **candidate** (structure, tip consistency, tx rules) before signing. The present **`pwmd`** attester path signs **`ClusterPropose`** fields (`vote_object`, `candidate_hash`, height, round) after **handshake / membership / role** gates — it does **not** yet re-run full block-body validation against the attester’s synced state inside `mk_cluster_attest`. Operators **SHOULD** keep attester tip within **sync of the leader** (§9.4) so that future §6-hard gates match reality; until block-level checks land, treat quorum attestations as **cryptographic assent to the proposal binding**, not a guarantee of full §6 replay on the attester process. **Roadmap:** wire candidate block reference (§5 VO1/VO2), deferred validation + `stale_tip` / `missing_tx_after_catchup` per §6.1.

---

## 9. Timeouts and disagreement reactions

### 9.1 Timer set (informative names)

All durations SHOULD be **operator-configurable** with safe defaults documented in the runbook.

- `T_block_assembly` (**config**, e.g. `block_assembly_timeout_ms`): leader gathers transactions / builds candidate before proposing — **not** hard-coded wall-clock in implementations.
- `T_propose`: wait for proposal after round start.
- `T_attest`: collect attestations after valid proposal.
- `T_seal`: optional bound on seal after quorum.

### 9.2 Deterministic reactions

| Condition | Required outcome |
|-----------|------------------|
| Proposal missing or invalid by §6 | No seal; emit **round_failed** reason `invalid_proposal`; optionally increment `R` (same `H`) per profile. |
| `< k` attestations by `T_attest` | No seal; reason `quorum_timeout`; operator-visible alarm. |
| Conflicting valid attest sets for **same** `(H,R)` | No seal; reason `equivocation_suspected`; **freeze** further seal attempts for `H` until operator ack (**lab**: optional automated round bump). |
| Attestation signature invalid or signer ∉ membership | Discard; count toward fault metrics; never seal. |
| Leader sealed without quorum when profile requires quorum | **Violation**; peers MUST treat as **misbehavior** (log + metric); fork-choice interaction **out of scope** unless header proves quorum (future RFC). |

### 9.3 Partition

If clones partition, **do not** seal on minority quorum unless profile explicitly allows **unsafe_min_quorum** (default **false**).

### 9.4 Lagging clone

If attester’s tip is behind `H-1`, it MUST **reject** attest with reason `stale_tip` **or** run catch-up to parent of candidate first — profile chooses; default **reject**.

### 9.5 Attester responsiveness and future quorum-slot demotion (informative)

**Problem framing:** `T_attest` bounds how long the leader (committer) waits for attestations after a valid proposal (§9.1–§9.2). An attester that is **persistently overloaded**—for example spending most of its wall time serving **catch-up, blocks, or RPC to external clients**—may often fail to validate and sign inside `T_attest`. The protocol outcome is deterministic given clocks and scheduling: **no seal** for that round with reason **`quorum_timeout`** once the deadline passes without enough ACKs. Operational **frequency** of that outcome, however, depends on load, queueing, and operator-chosen timeouts—so a busy clone can **de facto** behave like an unreliable quorum participant even though membership has not changed.

**Design direction (non-normative for baseline MVP):** for deployments with **`n > k`** or with a **relay pool** larger than the **active quorum** subset (§7.1), a **future profile** MAY introduce **liveness-aware quorum participation**:

1. **Observe** per-member signals: sustained late or missing attestations, repeated `quorum_timeout` episodes where a specific member is the likely bottleneck, optional **capacity hints** (implementation-defined metrics).
2. **Demote** chronically unresponsive members from the **active quorum slot** for subsequent rounds or epochs—**preferring more available clones** that remain in the closed **membership** set (informally: “freer bees” in the swarm), without inviting arbitrary Sybils (§4.1 still applies).
3. **Orthogonality:** adjusting **who must attest** (quorum slot rotation, demotion, promotion) MUST remain distinct from **S2 seal lease** (§8.1): revoking or rotating **attest obligation** does not by itself redefine **who may execute seal**; both axes need explicit operator policy and audit trails.

**MVP stance:** baseline Variant A does **not** require automated demotion. Operators SHOULD first widen **`T_attest`**, **`T_tx_catchup`**, and **`k-of-n` margin** (§7) before relying on automation. Normative algorithms (thresholds, fairness, cooldowns, re-promotion) are **deferred** with §12 item **4** (selecting active **k** attesters from a large pool).

---

## 10. Wire and protocol versioning

- Any new **mandatory** wire fields for attest/propose MUST bump **`PWM_PROTOCOL_VERSION`** or use a **negotiated capability bit** (`NodeHelloCapabilities`).
- Preferred hot path: extend the **peer/wire** session so a connection may be marked after handshake as **cluster participant** / **active quorum attester** (reduces reliance on REST for steady-state attest traffic).
- Until bump lands, implementations MAY use **out-of-band** channels (e.g. authenticated REST between clones) **only** under `multi_sealer_experimental` / lab flags documented in runbook.

### 10.1 Cluster gossip transport (phased)

**Current-phase MVP (especially ≤3 consensus nodes):** forwarding candidate-related gossip **only across established peer connections** to immediate neighbors is sufficient; with three nodes, classic broadcast **storms are not applicable** in the same way as in large fan-out meshes.

**Longer-term:** a dedicated **UDP** (or UDP-like) cluster broadcast plane MAY be introduced with **explicit anti-storm rules** (e.g. **no internal echo retransmission** loops — echo only at originating edge per policy). That mechanism is **out of scope** for baseline Variant A code slices until separately specified.

**Connectionless trust boundary:** Datagram delivery **does not** inherit the authenticity assumptions of an established **TCP peer session** (no session keying, easier spoofing of source addresses in some deployments). Therefore, any cluster-affecting frames on a **UDP / broadcast listen** plane SHOULD carry **mandatory per-message cryptographic authentication** — e.g. validator (or attestation) signatures over a deterministic payload domain compatible with §5 / VO binding, plus normative **replay / sequencing** rules. **MVP slices that use TCP peer wire only** may continue to rely on hello-trusted sessions **until** a UDP slice is specified; the UDP slice RFC/checklist MUST lock signing + replay policy **before** production opt-in.

---

## 11. MVP acceptance checklist (implementation slice)

A conforming MVP implementation **when enabled**:

1. Config validates `(membership, k, n, VO choice, timeouts, committer role)`.
2. Proposal carries `(H, R, vote_object)`; attest carries signatures over agreed payload.
3. Seal path refuses seal without quorum when profile demands it.
4. Metrics/logs expose quorum outcomes per §9.
5. Integration tests use **injected faults** (partition, mute attest, invalid candidate) — full testnet optional.

---

## 12. Open questions (target revision 0.5)

1. ~~On-chain **quorum proof** in header~~ — **not required for MVP** (informative: **Appendix B.1**); external third-party assurance is similarly out of scope (analogy: RAID/backups are operations, not chain proofs).
2. ~~**Unknown / late tx** at attest time~~ — **MVP default** in **§6.1** (bounded catch-up, ingest tx, analytics log; reject if still missing after `T_tx_catchup`). Alternative strict **reject-without-wait** profiles allowed as non-default.
3. ~~**Attestation vs block signing key** mapping~~ — **MVP:** same identity keys acceptable (**Appendix B.3**); derived attestation keys — later hardening.
4. Deterministic **algorithm for selecting active k attesters** from a relay pool larger than **n** (epochs, VRF, operator slice) — **deferred** until post–three-node MVP; **§9.5** seeds **liveness-weighted** demotion / promotion (“busy attester” mitigation) as **profile-defined** input to that selection, not a separate consensus layer.
5. **Divergence reactions** beyond §9 — refine into runbooks/tests **when** operational incident history exists; informal seed ideas only in **Appendix B.2** (non-prescriptive).
6. **Signed dynamic cluster join** (handshake self-identification block) — **separate slice** (**Appendix B.5**); design brainstorming + review before normative wire.

---

## 13. Appendix A — Owner-agreed decisions (informative, revision 0.3)

Captured so preliminary consensus does not get lost; normative clauses above take precedence.

### A.1 Leader determination (tie-break)

Implementations SHOULD **not** use raw **IP address alone** as the sole leader tie-break (NAT, DHCP, multi-homing, IPv4/IPv6 mixing). Prefer a **stable total order** on operator-assigned **`node_instance_id`** and/or **registered node name**, with deployment docs stating the exact predicate.

**Deployment guidance (mixed VPN / NAT / LAN):** operators SHOULD name and route nodes so the **designated leader-eligible** clone is **not** stuck behind the slowest path solely due to naming order — e.g. align **`node_instance_id`** lexicographic order with **expected lowest-latency** path to peers, or fix leader by explicit config in constrained topologies. This remains **operational** guidance; the RFC does not mandate a global latency oracle.

### A.2 Iterative rollout

Feature activation SHOULD be **incremental** (basic paths tested before wider **n** or UDP broadcast). CLI-style roles (e.g. `--allowed-cluster-roles proposer,attester` — exact spelling TBD) align with phased adoption.

### A.3 Relation to §7–§8

- **§7.1–7.2:** quorum as subset of relay pool; MVP **≤3** nodes first.
- **§8.1:** S2 seal lease vs quorum membership explicitly separated.
- **§9.1:** `T_block_assembly` configurable.
- **§10.1:** neighbors-first gossip now; UDP broadcast later with anti-echo discipline.

---

## 14. Appendix B — Owner decisions / next plast (informative; B.2 clarified v0.4.1)

Further agreements so later slices do not lose context. Normative sections above prevail; conflicts require RFC revision.

### B.1 Quorum proof visibility (MVP scope)

For **MVP**, **cryptographic proof of attest quorum inside the block header** for **external** observers is **not** a goal: the cluster consensus path targets **operator resilience** (fewer mistaken seals), not third-party auditability of intra-cluster votes. Demanding chain-visible proof would be comparable to expecting chain evidence that infra uses **RAID or backups** — **operational**, not protocol-core MVP.

**Deferred:** optional header fields proving quorum — future RFC if/when external attestability is required.

### B.2 Divergence taxonomy (leader vs attesters) — informal seed only

There is **no** complete catalogue of real-world failure modes for this internal-consensus path yet. **Normative** seal / no-seal behavior remains **§7** (quorum) and **§9** (timeouts and disagreement table).

The following table is **non-normative**: a rough vocabulary for future runbooks when operators accumulate incidents — **not** a commitment to specific reactions until reviewed again.

| Class | Informative examples | Notes |
|-------|----------------------|--------|
| **Structural binding to chain tip** | Parent hash, height linkage | Disagreement often implies **inconsistent views** of head — treat as **high severity** in incident response; exact operator actions **TBD** per deployment. |
| **Payload / execution layer** | Tx selection, fee-related fields in candidate | May sometimes be **retried** without equating to permanent fork — **illustrative** only; concrete recovery steps **TBD**. |
| **Chronic attester overload / starvation** | Hot path dominated by external sync or RPC; attest path starved | Contributes to **`quorum_timeout`** (§9.2); future **quorum-slot demotion** toward more responsive members — **§9.5** (non-normative MVP). |

Ambiguous or partial attest patterns (counts near thresholds, timeouts mid-round, asymmetric silence) SHOULD be handled by **logging, metrics, and operator escalation** until enough field evidence exists to justify stricter automation.

### B.3 Validator key alignment (MVP)

For the **first implementation slice**, it is **acceptable** that **proposer and attesters use the same validator public identity** (same key material / aligned keys) as already implied by clone semantics; dedicated **attestation sub-keys** remain optional follow-up.

### B.4 Attestation transport channels (extensibility)

Two logical communication patterns SHOULD remain distinguishable in future wire design:

1. **Attester → leader:** votes / attest payloads needed to decide whether the leader may seal (`commit` path).
2. **Attester → all cluster participants** (incl. broadcast listen plane): **observability** of quorum fill, epoch rotation, and optional **inclusion requests** (“request promotion into active quorum”) for large relay pools.

Large deployments may use a **broadcast listen** channel so every node tracks quorum progress; **unicast to leader** remains the locus for committing attestations unless a profile specifies otherwise. Protocol evolution SHOULD reserve extension points so both facets coexist **without** duplicating conflicting attest semantics.

### B.5 Dynamic cluster membership (deferred slice)

**Registration / join** with a **signed self-identification block** attached to an extended **handshake** phase is **not** part of baseline MVP wire in this RFC. Plan a **separate slice**: pwm-review input, brainstorming, normative fields — before enabling hot join under production profiles.

---

## 15. References

- `docs/reviews/20260511-single-sealer-S3-cluster-consensus-design.md` — Variant A positioning.
- `docs/reviews/20260509-s2-lease-fencing-failover-final-review.md` — process-local lease limits.
- `docs/rfc/8-shard-runtime-identity-and-peering.md` — identity and peering baseline.
- `docs/plans/mvp_v2.md` — **Sprint V2-9** (planned) first implementation tranche for this RFC.

---

## 16. Implementation readiness (informative)

**Inputs for this Draft are sufficient** to plan a **first coding sprint** for Variant A **without** waiting for every deferred topic:

| Closed / scoped | Reference |
|-----------------|-----------|
| Single-leader candidate, attest semantics, quorum vs relay pool, S2 orthogonality | §2.1, §7–§8 |
| Timeouts, disagreement table | §9 |
| MVP ≤3 nodes stabilization | §7.2 |
| Late tx / candidate ordering default | **§6.1** (closes former §12 gap on unknown txs) |
| No header quorum proof for MVP | §12.1, Appendix B.1 |
| Keys MVP | §4.2, Appendix B.3 |
| Transport phases (neighbors first; UDP deferred) | §10.1 |

**Explicitly deferred** (do not block slice 0): §12 items **4–6** — large-pool attester selection, divergence runbook automation, **dynamic join** wire (Appendix B.5).

**Suggested milestone:** [docs/plans/mvp_v2.md](../plans/mvp_v2.md) **Sprint V2-9** — **multiple slices** behind **feature flags**: core cluster path (§6.1, ≤3 nodes), then **2-node** and **3-node** waves, plus **same-shard followers that are not cluster members** — they MUST still **converge to the shard tip/state** produced by the cluster (typically via **same-shard sync v1**, Sprint V2-8; otherwise an explicit lab baseline in the sprint checklist). **Transport (locked for implementation planning):** attest/propose traffic uses **extended peer wire + capability negotiation** per §10; cluster handshake / membership for MVP slices runs **only over established peering connections** — no parallel out-of-band REST channel as the normative cluster bootstrap path.

**Planning note (repo alignment):** automated wave acceptance that remained open under **V2-8 Slice 6** on a **legacy multi-sealer** path is **not** treated as a blocker for this RFC: multi-node scenarios **transfer** to **V2-9** tests under **single proposer + attesters**, because competing seals for the **same validator identity** were not reliably convergent without this architectural pivot (see plan narrative under Sprint V2-8 *Статус и перенос приёмки*).

**Status promotion:** moving this document from **Draft** toward **Frozen** for a slice SHOULD follow a **`pwm-review`** gate after wire sketches and flag defaults are agreed.
