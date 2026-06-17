# V6-1 Spec / RFC / ADR Freeze Review

**Ticket:** `tasks/20260605-v6-sprint1-spec-adr-freeze.json`  
**Plan anchor:** [docs/plans/mvp_v6.md](../plans/mvp_v6.md) § Sprint V6-1  
**Slice type:** doc-only (no `crates/` changes)

## 1. Scope recap

Reviewed the claimed V6-1 normative freeze before coding sprints V6-2…V6-11.

**Ticket / plan scope:**

- RFC addenda: 4 (stake admission), 16 (multi-proposer rotation), 9 §A.5 (Mode B escrow), 6 (`ActivatePolicy` + evacuation), 10 (prepared wallet activation), 15 (peer sync scoring).
- New ADRs: 0009 (address flags runtime), 0010 (slashing evidence stubs), 0011 (policy activation target + fee-free activation).
- Updates: ADR 0006 (enforcement = V6), `docs/adr/README.md`, `docs/CONCEPT_ROADMAP.md` § MVP V6, `docs/rfc/9-crossdomain-roaming.md` §A.5 pointer, `docs/plans/mvp_v6.md` (V6-1 todo `in_progress`), `docs/ORCHESTRATOR-NOTES.md` (bootstrap template + V6-1 stub entry).

**Acceptance criteria checked:**

- No open `TBD` on V6 wire fields (addenda + new ADRs).
- ADR 0006 marked enforcement = V6.
- Consistency with `mvp_v6.md` accepted decisions (Option A incremental PoS, no CometBFT in V6, fee=0 `ActivatePolicy`, `activation_target` emergency evac, Mode B escrow, slashing stubs record-only, peer score non-consensus default).
- Internal cross-references coherent.

## 2. Requirements fit

**Met (substantive):**

| Decision (mvp_v6.md) | Where frozen | Assessment |
|---|---|---|
| Stake-gated active set, epoch boundary | `v6-rfc4-validators-stake-admission.md` | Aligned: registered vs active indices, `min_validator_stake`, mid-epoch stake changes deferred to next epoch. |
| RFC16 rotation over **active** set, failover ≤1 block | `v6-rfc16-multi-proposer-rotation.md` | Aligned: deterministic slot function, miss detection, evidence hook to ADR 0010. |
| No CometBFT / BFT replacement in V6 | RFC4 addendum, CONCEPT_ROADMAP §V6/V7 | Explicit and consistent. |
| Mode B escrow (lock, IMPORT release, timeout refund) | `v6-rfc9-mode-b-escrow.md`, RFC9 §A.5 pointer | Normative state machine and wire types present; supersedes pre-V6 «not implemented» posture. |
| Slashing stubs, no seizure | ADR 0010 | Append-only `EvidenceRecord`, explicit non-effects on balances/active set. |
| Peer score non-consensus default | `v6-rfc15-peer-sync-scoring.md` | Operator-local `pwmd` cache; consensus table explicitly deferred. |
| ADR 0006 runtime (bits 0–1) | ADR 0009 + ADR 0006 status update | Cosign non-disableable, conservation queue, interaction matrix with emergency routing. |
| `ActivatePolicy` + `activation_target`, fee=0, emergency evac | ADR 0011, `v6-rfc6-activate-policy-activation-target.md` | Wire extension, stable rejects, same-shard evacuation semantics, future uses scoped out of V6 runtime. |
| Prepared activation in wallet | `v6-rfc10-prepared-policy-activation.md` | Schema, invariants (`fee_pwm = 0`), CLI flags deferred to V6-7 as planned. |

**Gaps / partial coverage (non-blocking for V6-1 doc gate, see nits):**

1. **Mode B refund application timing** (`v6-rfc9-mode-b-escrow.md` §5): allows lazy refund on next account-touching tx *or* proactive seal tick, while requiring «outcome MUST be deterministic». At a fixed block height, lazy vs proactive paths can yield **different consensus state roots** (lock still `Locked` vs already `Refunded`). This should be pinned before V6-5 coding (recommend: **MUST** apply refund on seal tick when `current_height >= unlock_height`, with lazy path forbidden for consensus state or limited to non-state RPC preview only).

2. **`u128` JSON encoding** for new economic fields (`min_validator_stake`, `CrossShardLock.amount_pwm`, `PendingConservationTransfer.amount_pwm`): types are frozen, but addenda do not cross-reference the established V5 rule (RFC 0012 / RFC 0007: public JSON/snapshot `u128` as **decimal strings**). Precedent: V5-1 passed freeze with similar follow-up in V5-2; acceptable as V6-2 serde obligation, not V6-1 blocker.

3. **ADR 0006 §CONSERVATION** still says cancel/redirect is «subject to a future ADR» while ADR 0009 now normatively covers emergency interaction — stale cross-era wording, not a contract contradiction.

4. **CONCEPT_ROADMAP § MVP V6** links ADR 0010/0011/0009 and RFC 9/15 addenda; items 1–2 (stake admission, RFC16 rotation) and RFC 6/10 addenda are named in prose but lack direct hyperlinks to `v6-rfc4-*`, `v6-rfc16-*`, `v6-rfc6-*`, `v6-rfc10-*`.

5. **`docs/adr/README.md`** row for ADR 0006 still reads «Accepted (spec-only)» without the V6 enforcement pointer visible in the index column (body text elsewhere is fine).

**TBD scan:** No `TBD` / `TODO` / `FIXME` in `docs/adr/0009–0011` or `docs/rfc/addenda/v6-*`. Parent `docs/rfc/9-crossdomain-roaming.md` §A.5 retains historical «finalization signal — TBD» in pre-V6 narrative; V6 normative pointer to the addendum satisfies the ticket criterion for **V6 wire fields**.

## 3. Style and module shape

Doc-only slice — production Rust naming, `check_entity_name_segments.py`, and module banners are **not applicable**.

Documents follow established ADR/RFC addendum shape: English normative prose, explicit Status, frozen wire blocks, Non-Goals, and References. Terminology is consistent with V4/V5 policy and validator vocabulary (`active_validator_indices`, `PolicyTx`, `rescue_address`, stable `E_*` rejects).

Minor style nits:

- ADR 0010 `EvidenceType` / `EvidenceTxBody` wire enums lack explicit JSON/tag representation (acceptable defer to V6-2/V6-9 if tagged like existing tx enums).
- RFC10 mentions `wallet account prepare-activation` as a overwrite path not enumerated in `mvp_v6.md` V6-7 CLI list — harmless optional alias if documented in V6-7.

### Wire JSON / u128

**Scope:** Yes — normative V6 wire/state fields include `u128` quantities on snapshot v4 and genesis (`min_validator_stake`, `amount_pwm` in `CrossShardLock` and `PendingConservationTransfer`). `ActivatePolicy.activation_target` is `AccountId` (existing encoded type). Wallet `PreparedPolicyActivation` uses hex/pretty strings for targets and `signed_tx_b64` (not raw `u128` on wire).

**Findings:**

- New `u128` fields are named and typed but addenda do not state decimal-string JSON encoding.
- Repo precedent (RFC 0012 §public JSON, RFC 0007 fee rule, V5-2 implementation) already governs snapshot/API surfaces.
- **Severity: medium doc gap**, not a derive-only `u128` on peer JSON hazard in this slice (coding must attach `ser_json_u128` / snapshot decimal helpers in V6-2).
- **Recommendation:** one sentence per addendum or a single cross-ref in `mvp_v6.md` §V6-2 pointing at RFC 0012 / RFC 0007 for all new `u128` snapshot/genesis fields.

## 4. Safety

Spec-only; risks are protocol/operator semantics:

- **Emergency fee=0 + evacuation:** ADR 0011 anti-abuse bounds (emergency-only binding, same-shard, target MUST match rescue, one-shot evac) are adequate for V6 freeze.
- **Conservation delay:** height-only delay avoids wall-clock nondeterminism; ADR 0009 mempool line allows reject *or* pending-only admission — acceptable if seal path is canonical (stated).
- **Mode B griefing:** duplicate IMPORT, post-refund IMPORT, and locked-balance invariants are specified; refund **timing** ambiguity (§2.1 above) is the main safety/consensus concern.
- **Slashing stubs:** explicit prohibition on balance seizure and validator ejection from evidence alone — matches ADR gate in plan.
- **Peer score:** non-consensus default avoids emission/governance coupling; score table deferred.

No crypto implementation, panic paths, or RPC trust-boundary code in this slice.

## 5. Tests

**N/A** for executable verification on this doc-only slice.

**Future spec-test obligations (by sprint):**

| Sprint | Obligation implied by freeze |
|---|---|
| V6-2 | Serde round-trip v3→v4 for all frozen structs; `u128` decimal JSON; `ActivatePolicy` optional `activation_target`; reject code stubs. |
| V6-3 | Below-threshold excluded at epoch boundary; `min_validator_stake = 0` recovery path. |
| V6-4 | Harness: primary miss → failover block at `height+1`. |
| V6-5 | EXPORT lock/refund/IMPORT replay; deterministic refund at `unlock_height`. |
| V6-6 / V6-8 | `E_POLICY_FLAG_NON_DISABLEABLE`, conservation queue + emergency supersede ordering. |
| V6-7 | `emergency_activation_*`, prepared activation wallet round-trip, `fee=0` / target mismatch rejects. |
| V6-9 | `E_EVIDENCE_DUPLICATE`; peer score deterministic deltas; optional `EvidenceTx` shape. |

`pwm-testing` not required before V6-2 per plan.

## 6. Verdict

**approve with nits**

**Prioritized nits (orchestrator may auto-close mechanical items per PASS_WITH_NITS policy):**

1. **P1 (before V6-5):** Normatively fix Mode B refund application — seal-tick MUST at `unlock_height`, or document why differing per-height state is allowed without breaking state-root consensus.
2. **P2 (before V6-2 serde):** Add cross-ref to RFC 0012/RFC 0007 decimal-string rule for new `u128` fields in V6 addenda or V6-2 plan bullet.
3. **P3 (doc hygiene):** Update ADR 0006 conservation bullet to reference ADR 0009 instead of «future ADR»; add CONCEPT_ROADMAP hyperlinks to RFC4/16/6/10 addenda; clarify ADR 0006 index row («spec V5, enforcement V6»).

No blocking contradictions comparable to V5-1 RFC7/RFC12 split were found. V6-1 acceptance criteria are **substantively satisfied** for proceeding to V6-2 core model.

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260605-v6-sprint1-spec-adr-freeze-review.md
token_usage:
  source: estimate
  input: 28000
  output: 4500
  total: 32500
  confidence: medium
```

**Verdict:** approve with nits
