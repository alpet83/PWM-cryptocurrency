# RFC 0004: Validator Model & Finality

**Status:** Draft
**Version:** 0.2
**Depends on:**

* RFC 0001 (Address Format)
* RFC 0002 (Subject Model)
* RFC 0003 (Cross-Domain Roaming)
* RFC 0005 (Genesis Distribution & Bootstrap)

---

## 1. Abstract

This document defines the **validator model**, **finality mechanism**, and **validator lifecycle** in PWM.

Validators are **domain-scoped actors** responsible for:

* block production
* transaction validation
* finality certification

For v1 testnet baseline, validator participation MAY start from static per-shard sets. Stake-backed and IPv4-derived admission is a post-v1 extension (see RFC 0005).

---

## 2. Design Goals

1. Deterministic finality
2. Domain independence (no global consensus)
3. Compatibility with cross-domain roaming
4. Simple MVP implementation
5. Evolution toward stake-based governance

---

## 3. Validator Roles

---

### 3.1 Shard Validators

Primary consensus actors within a domain.

```text
role: shard_validator
scope: single domain
responsibilities:
  - produce blocks
  - validate transactions
  - sign finality certificates
```

---

### 3.2 Border Validators (Optional)

Policy enforcement layer for roaming.

```text
role: border_validator
scope: target domain
responsibilities:
  - issue AdmissionCertificate (RFC 0003)
  - enforce compliance policies
```

---

### 3.3 Witness Validators (Optional)

Auxiliary co-signers without block production.

```text
role: witness_validator
responsibilities:
  - multisig authorization
  - recovery / arbitration
```

---

## 4. Validator Set

### 4.1 Definition

```text
ValidatorSet {
  validators: Vec<PubKey>
  threshold: u8  // e.g., 2/3
}
```

---

### 4.2 Domain Scope

Each domain maintains its own:

```text
domain_validator_set[domain_id]
```

No global validator set exists.

---

### 4.3 Source of Validator Stake (post-v1 extension)

Validator eligibility is derived from:

* IPv4 claim allocation (RFC 0005)
* delegated stake
* bootstrap authorization (early phase)

---

## 5. Block Model

```text
Block {
  height: u64
  parent_hash: H256
  txs: Vec<Tx>
  proposer: PubKey
}
```

---

## 6. Finality Model

### 6.1 Definition

A block is finalized when:

```text
signatures >= configured_threshold_for_shard
```

---

### 6.2 Finality Certificate

```text
FinalityCertificate {
  block_hash: H256
  height: u64
  validator_signature_agg: bytes
}
```

Used for:

* Export proof (RFC 0003)
* Cross-domain verification

---

### 6.3 Properties

* deterministic
* no probabilistic confirmations
* no reorg after finality

---

## 7. Signature Model

### 7.1 Individual Signature

```text
sig_i = Sign(privkey_i, block_hash)
```

---

### 7.2 Aggregation

```text
agg_sig = Aggregate(sig_1 ... sig_n)
```

MVP MAY use:

* raw signature list
* Schnorr batch
* BLS (future)

---

## 8. Validator Lifecycle

---

### 8.1 Activation

Validator becomes active if:

```text
stake >= minimum_threshold
AND registered in validator set
```

---

### 8.2 Deactivation

Occurs if:

* stake withdrawn
* validator removed by governance
* inactivity threshold exceeded

---

### 8.3 Delegation

```text
Delegation {
  delegator
  validator
  amount
}
```

Allows non-technical participants to support validators.

---

## 9. Interaction with Roaming

### 9.1 Export Requires Finality

```text
ExportTx valid only after finalized block
```

---

### 9.2 Import Requires Proof

```text
ImportTx must include FinalityCertificate
```

---

### 9.3 Trust Model

Target shard trusts:

* validator quorum of source shard
* admission validators (if applicable)

---

## 10. Security Model

### 10.1 Assumption

≥2/3 validators are honest.

---

### 10.2 Failure Modes

| Scenario               | Effect            |
| ---------------------- | ----------------- |
| validator offline      | delay             |
| < threshold signatures | no finality       |
| ≥2/3 collusion         | shard compromised |

---

### 10.3 Mitigation

* domain isolation
* admission layer
* future Bitcoin anchoring

---

## 11. Island Operation

Validators MUST support:

* fully local operation
* delayed roaming
* no cross-domain dependency

---

## 12. MVP v1 Constraints

MVP v1 MUST include:

* static validator set
* simple proposer model
* configurable finality threshold profile
* FinalityCertificate

MVP MUST NOT include:

* slashing
* complex BFT
* dynamic validator rotation

---

## 13. Future Extensions

* dynamic validator sets
* staking economics
* slashing
* BFT consensus (HotStuff / Tendermint)
* zk-finality proofs

---

## 14. Conclusion

PWM validators form a **domain-local finality layer** enabling:

* independent shard operation
* verifiable cross-domain transfers
* simple initial deployment
