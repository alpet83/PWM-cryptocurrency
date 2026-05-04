# RFC 0003: Cross-Domain Roaming (Export / Import / Admission)

**Status:** Draft
**Version:** 0.1
**Depends on:**

* RFC 0001 (Address Format / bech32DX)
* RFC 0002 (Subject Model & Domain Semantics)

---

## 1. Abstract

This document defines the **cross-domain roaming mechanism** in the PWM protocol.

Roaming enables value transfer between domain shards without custodial bridges by using:

* **Export** (source domain exit)
* **Finality Certificate** (proof of inclusion and finality)
* **Admission** (target-domain policy approval, optional)
* **Import** (target domain entry)

Roaming is **asynchronous** and **policy-constrained**.

Scope alignment for `MVP SPEC v1 testnet`:

- Roaming is an **additive extension** over the account-based model from `WHITE_SPEC_v0`.
- Local value flow (`INIT/TRANSFER/STAKE/UNSTAKE/BURN_MARK`) remains intact on same-shard paths.
- Cross-shard movement is explicit (`EXPORT` + `IMPORT`), not an implicit replacement of `TRANSFER`.

---

## 2. Terminology

* **Shard / Domain:** Independent ledger identified by `domain_hi + domain_lo`
* **Source Shard (Sₐ):** Domain where value originates
* **Target Shard (Sᵦ):** Domain receiving value
* **Export:** Transaction that removes or locks value in Sₐ
* **Import:** Transaction that creates value in Sᵦ
* **Finality Certificate:** Validator quorum proof that export is finalized
* **Admission Certificate:** Target-domain approval (policy/compliance)
* **Export ID:** Unique identifier of exported value
* **Roaming:** Cross-domain transfer protocol

---

## 3. Design Goals

1. **No custodial bridge**
2. **Shard independence (island resilience)**
3. **Asynchronous execution**
4. **Deterministic validation**
5. **Policy-aware admission**
6. **Replay protection**

---

## 4. High-Level Flow

```text
Sₐ (source)                      Sᵦ (target)

EXPORT TX
   ↓
Finality Certificate
   ↓
(optional) Admission Certificate
   ↓
IMPORT TX
```

---

## 5. Export Transaction

### 5.1 Structure

```text
ExportTx {
  from: Address
  target_domain: u16
  recipient: Address
  amount: u128
  fee: u128
  nonce: u64
}
```

### 5.2 Semantics

Upon inclusion:

* value is **debited from spendable account balance** in source shard
* export commitment is recorded for proof generation/finality
* route into roaming flow is derived by protocol domain comparison (`domain_hi(sender) != domain_hi(receiver)`), not by a manual route selector

### 5.3 Value Handling Modes

MVP MUST choose one:

#### Mode A — Burn

```text
value is destroyed in Sₐ
```

#### Mode B — Escrow (recommended)

```text
value is locked and cannot be spent locally
```

**As-implemented devnet MVP (see RFC 0009):** `EXPORT` debits spendable balance and records export commitment for the `IMPORT` path; **Mode B escrow is not implemented** as a distinct output/state machine. A **proposed** future protocol upgrade (conditional lock, finality proof, timeout/refund) is documented in [9-crossdomain-roaming.md](9-crossdomain-roaming.md) Appendix A.5; do not treat that appendix as current runtime behavior.

For strict-upgrade v1, the lock/commitment is tracked in account-ledger metadata (no mandatory UTXO output type requirement).

---

## 6. Export Identifier

```text
export_id = hash(
  source_shard ||
  tx_hash ||
  output_index ||
  nonce
)
```

MUST be globally unique.

---

## 7. Finality Certificate

### 7.1 Structure

```text
FinalityCertificate {
  source_shard: u16
  block_height: u64
  block_hash: H256
  export_id: H256
  merkle_proof: bytes
  validator_signature_agg: bytes
}
```

### 7.2 Requirements

* block MUST be finalized
* signatures MUST satisfy the source-shard finality profile configured for v1 testnet (minimal profile allowed, stronger quorum profiles allowed)
* proof MUST include ExportTx

---

## 8. Admission Certificate (Optional Layer)

### 8.1 Purpose

Enables:

* AML / compliance
* domain policy enforcement
* rate limiting / quarantine

### 8.2 Structure

```text
AdmissionCertificate {
  target_shard: u16
  source_shard: u16
  export_id: H256
  decision: enum { allow, deny, quarantine }
  policy_hash: H256
  signature_agg: bytes
}
```

### 8.3 Behavior

| Decision   | Effect                       |
| ---------- | ---------------------------- |
| allow      | Import permitted             |
| deny       | Import permanently rejected  |
| quarantine | Import delayed / conditional |

---

## 9. Import Transaction

### 9.1 Structure

```text
ImportTx {
  export_certificate: FinalityCertificate
  admission_certificate?: AdmissionCertificate
}
```

### 9.2 Validation Rules

```text
validate_import(tx):

  assert export_id not in ImportedSet

  verify_finality(export_certificate)

  if admission_required(target_shard):
      verify_admission(admission_certificate)

  assert export.target_domain == current_shard

  assert recipient matches export

  mark export_id as used

  credit recipient account balance
```

---

## 10. Replay Protection

Each shard MUST maintain:

```text
ImportedSet: Set<export_id>
```

If `export_id` already exists → reject.

---

## 11. Domain Routing Rule

Baseline:

```text
if domain_hi(sender) != domain_hi(receiver):
    roaming_required = true
```

---

## 12. Admission Requirement

```text
requires_admission =
    cross_domain
    OR policy_requires(target_shard)
```

For v1 baseline, `cross_domain` is protocol-derived from sender/receiver domain classes and MUST NOT depend on client-side forced route-mode flags.

---

## 13. Failure Modes

### 13.1 Invalid Certificate

* reject import

### 13.2 Duplicate Import

* reject

### 13.3 Admission Denied

* reject permanently

### 13.4 Quarantine

* hold until condition satisfied

### 13.5 Cross-Domain Burn Context

If burn references cross-domain context:

* burn proof is created/validated only in source shard;
* target shard is not required to mutate local burn state for external burn events.

---

## 14. Security Considerations

### 14.1 Trust Model

* trust source shard validator set
* trust admission validator set (if used)

### 14.2 Attack Vectors

| Attack                   | Mitigation             |
| ------------------------ | ---------------------- |
| replay import            | ImportedSet            |
| forged export            | validator quorum       |
| fake admission           | signature verification |
| cross-shard double spend | export locking         |

---

## 15. Island Resilience

Roaming MUST tolerate network partition:

* exports can occur independently
* imports processed when proofs arrive
* no synchronous dependency required

---

## 16. Non-Goals

* atomic cross-shard execution
* synchronous consensus across shards
* universal permissionless transfers

---

## 17. Minimal MVP Scope

MVP v1 testnet SHOULD implement:

* ExportTx
* FinalityCertificate (static validator set)
* ImportTx
* ImportedSet
* No Admission layer (optional)
* Compatibility with existing same-shard v0 transfer flow

---

## 18. Future Extensions

* dynamic validator sets
* Bitcoin anchoring for finality
* zk-proof admission
* multi-hop roaming
* rate-limited roaming quotas
* **Source-side conditional lock on export** until target-side finalization is proven (or timeout/refund): normative sketch and «not implemented in MVP» posture in [RFC 0009 Appendix A.5](9-crossdomain-roaming.md#appendix-a-mvp-stabilization-delta-2026-05) (pairs with optional settlement/import-export chain discussion there).

---

## 19. Conclusion

PWM roaming defines a **non-custodial, proof-based cross-domain transfer model** where:

* value exits via export
* is proven via validator finality
* is optionally approved via admission
* and enters target domain via import

This enables:

* domain sovereignty
* policy enforcement
* scalable multi-domain operation

without reliance on bridges or global consensus.

