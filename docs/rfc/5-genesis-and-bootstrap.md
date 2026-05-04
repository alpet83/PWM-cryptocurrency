# RFC 0005: Genesis Distribution & Validator Bootstrap

**Status:** Draft
**Version:** 0.1
**Depends on:**

* RFC 0002 (Subject Model)
* RFC 0004 (Validator Model)

---

## 1. Abstract

This document defines post-v1 economic/bootstrap tracks and optional compatibility hooks. For v1 testnet baseline, fixed genesis bootstrap from v0 remains valid.

This document defines:

* initial token issuance
* IPv4-based distribution
* legacy token allocation
* validator bootstrap model

Claim-seeded genesis is an extension track and is not mandatory for v1 testnet baseline.

---

## 2. Total Supply

```text
TotalSupply = 21,000,000,000 PWM
```

* fully minted at genesis
* not fully distributed immediately

---

## 3. Distribution Pools

### 3.1 IPv4 Claim Pool (post-v1 extension)

```text
~4 billion PWM
```

Allocated to:

* IPv4 address holders
* infrastructure operators
* organizations

---

### 3.2 Legacy PWM Pool

```text
<1 billion PWM
```

Allocated to:

* testers
* developers
* early contributors

---

### 3.3 Future Distribution Pool

Remaining supply distributed:

```text
annually via IPv4 claim
```

---

## 4. IPv4 Claim Mechanism (post-v1 extension)

### 4.1 Purpose

* bootstrap distribution
* seed validator base
* align with real infrastructure

---

### 4.2 Claim Input

```text
Claim {
  ipv4_range
  proof_of_control
  destination_address
}
```

---

### 4.3 Proof Types (implementation-defined)

* BGP announcement
* reverse DNS control
* signed challenge via hosted endpoint
* manual verification (bootstrap phase)

---

### 4.4 Claim Result

```text
allocated_pwm = f(ipv4_range_size)
```

---

## 5. Validator Bootstrap

For v1 baseline:

- static per-shard validator list from genesis is acceptable;
- dynamic claim/delegation onboarding may be deferred.

---

### 5.1 Bootstrap Validator Sources

Validators can emerge from:

1. IPv4 claim holders
2. Delegated operators
3. Bootstrap-authorized nodes

---

### 5.2 Bootstrap Phase

During initial network launch:

* validator set may be partially predefined
* controlled rollout allowed
* gradual decentralization expected

---

### 5.3 Validator Entry Paths

#### Direct Entry

Claimant runs validator

#### Delegated Entry

Claimant delegates stake

#### Governance Entry

Bootstrap authority assigns validator

---

## 6. Domain-Based Emergence

Validators may organize by:

```text
domain_hi → jurisdiction class
domain_lo → corporate / subdomain grouping
```

This enables:

* country-based validator clusters
* corporate domain validators
* hybrid structures

---

## 7. Annual Redistribution (post-v1 extension)

### 7.1 Mechanism

Every year:

* new claim window opens
* unclaimed pool redistributed
* stake base expands

---

### 7.2 Effects

* onboarding new participants
* gradual decentralization
* dynamic validator expansion

---

## 8. Governance Implications

Early network is:

```text
bootstrap-distributed
claim-seeded
progressively decentralized
```

NOT fully permissionless at genesis.

---

## 9. Risks

### 9.1 Centralization

* large IPv4 holders dominate

### 9.2 Legacy Bias

* early participants advantage

---

## 10. Mitigations

* annual redistribution
* delegation model
* domain-based validator separation

---

## 11. Interaction with Validator Model

RFC 0004 uses:

* claim-derived stake
* delegation
* bootstrap validators

to define validator sets.

---

## 12. Non-Goals

* perfect fairness
* equal distribution
* instant decentralization

---

## 13. Future Extensions

* IPv6 integration
* proof standardization
* reputation-based weighting
* hybrid identity models

---

## 14. Conclusion

PWM genesis uses:

* **immediate total issuance**
* **IPv4-based distribution**
* **progressive validator formation**

This provides:

* fast bootstrap
* infrastructure alignment
* realistic path to decentralization
