# ADR 0010: Slashing evidence stubs (V6)

## Status

**Accepted as V6 normative contract.** Record-only evidence in shard state; **no** balance seizure, **no** validator ejection from evidence alone in V6.

## Context

Roadmap V6 and [CONCEPT_ROADMAP.md](../CONCEPT_ROADMAP.md) require a forward-compatible path toward production slashing without implementing fund seizure or complex BFT fault proofs in the incremental PoS track. Operators need deterministic, auditable evidence attachment for future enforcement and peer-quality analytics.

V6 keeps `Chain::seal` and RFC16 attestation; slashing in V6 is a **stub** that appends evidence records when valid evidence transactions or internal seal hooks fire.

## Decision

### Evidence record (consensus state, snapshot v4)

```text
EvidenceType =
  DuplicateVote           // same height/round, conflicting vote_object binding
  InvalidAttestation      // attestation on candidate failing §6 checks (profile-gated)
  UnavailableProposer     // proposer miss recorded for rotation analytics
  CustomStub(u16)         // reserved lab codes; MUST NOT seize funds

EvidenceRecord {
  record_id: Hash32,       // deterministic id over (height, offender_idx, evidence_type, payload_hash)
  height: u64,
  offender_validator_idx: u16,  // index into GenCfg.vals / active set context at height
  evidence_type: EvidenceType,
  payload_hash: Hash32,     // hash of opaque evidence bytes
  reporter: Option<AccountId>,  // None for system-generated seal hooks
}
```

Records are **append-only** per shard. Duplicates with the same `record_id` MUST be rejected with `E_EVIDENCE_DUPLICATE`.

### Tx surface (V6 minimal)

Optional `EvidenceTx` (or `SystemTx::SubmitEvidence`) MAY be accepted when:

- submitter is active validator or profile-defined reporter;
- `payload_hash` references opaque bytes stored off-chain or in tx attachment per implementation;
- **no** balance fields.

If no public tx is enabled in V6-9, seal-internal hooks MAY append `UnavailableProposer` only. Public `EvidenceTx` wire is frozen but MAY ship disabled behind profile.

### Explicit non-effects (V6)

Evidence records MUST NOT:

- reduce `staked_pwm` or spendable balance;
- remove validators from `active_validator_indices` (stake gating remains separate, V6-3);
- alter peer connection set by itself.

Future ADR MAY define enforcement mapping from `EvidenceRecord` to stake penalties.

### Interaction with peer sync scoring

Peer sync scoring ([RFC 15 addendum](../rfc/addenda/v6-rfc15-peer-sync-scoring.md)) is **operator-local first**. Evidence records MAY be displayed alongside peer score in ops tooling; they are not a reputation economy.

## Wire (frozen)

```text
EvidenceTxBody {
  offender_validator_idx: u16,
  evidence_type: EvidenceType,
  payload_hash: Hash32,
}
```

Fee: normal fee model unless profile sets `fee = 0` for system reporters (default: normal fee).

## Non-Goals (V6)

- Automatic slashing percentages.
- Cross-shard evidence aggregation.
- Cryptoeconomic stake redistribution.
- CometBFT `Evidence` module parity.

## Consequences

- Snapshot v4 includes `evidence_log: Vec<EvidenceRecord>` or paged equivalent.
- CY soak (V6-10) MAY assert evidence append on induced proposer miss (analytics only).
- Production seizure requires a post-V6 Accepted ADR.

## References

- [RFC 4: Validators](../rfc/4-validators-and-finality.md)
- [RFC 16: Validator clone attestation](../rfc/16-validator-clone-attestation.md)
- [RFC addendum: V6 RFC16 rotation](../rfc/addenda/v6-rfc16-multi-proposer-rotation.md)
- [MVP v6 plan](../plans/mvp_v6.md)
