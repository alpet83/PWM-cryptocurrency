# RFC 16 Addendum (V6): Multi-proposer rotation over active set

**Parent:** [16-validator-clone-attestation.md](../16-validator-clone-attestation.md)  
**Status:** Normative for MVP v6 (V6-1 freeze)  
**Depends on:** [RFC 4 addendum V6](v6-rfc4-validators-stake-admission.md)

## 1. Summary

Extends RFC16 Variant A cluster attestation with deterministic **proposer rotation** over the **active** validator index list. Target: if the scheduled proposer misses its slot, the next proposer produces within **≤ 1** additional block height (failover, not multi-round BFT).

## 2. Proposer index function (frozen)

```text
active_len = active_validator_indices.len()
epoch = epoch_counter
height = current_block_height

proposer_slot = height % active_len
primary_proposer_idx = active_validator_indices[proposer_slot]

// Failover within same height epoch (single-block skip):
// If primary did not seal within profile window, next slot at height+1:
failover_slot = (height + 1) % active_len
failover_proposer_idx = active_validator_indices[failover_slot]
```

Implementations MUST map `proposer_idx` to cluster leader identity (pubkey / clone role) per existing `pwmd` cluster path.

## 3. Miss detection

A **miss** occurs when:

- `height` advances without a sealed block from `primary_proposer_idx` within the profile tick window, **or**
- quorum timeout (RFC16 §9.2) expires without seal from primary.

On miss, seal at `height+1` MUST attempt `failover_proposer_idx` before rotating to `(height+2) % active_len`.

**Acceptance target:** induced primary miss in harness → valid block from failover at `height+1` (≤ 1 skipped block).

## 4. Attestation unchanged

- Attesters still validate the **leader's candidate** (RFC16 §2).
- No competing multi-proposer rounds in V6.
- Attester set remains clone membership profile; active set gates **who may be leader**, not attester eligibility.

## 5. Evidence hook

Proposer miss MAY append `EvidenceRecord` with `UnavailableProposer` ([ADR 0010](../../adr/0010-slashing-evidence-stubs.md)); no stake seizure.

## 6. Non-goals (V6)

- Competing proposals at same height.
- `2f+1` BFT among distinct validators.
- Lease/fencing changes (RFC 8 unchanged).
