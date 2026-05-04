# RFC: Subject Model, Domain Semantics, and Policy-Constrained Transactions in PWM

**Status:** Draft
**Version:** 0.1
**Scope:** Phase 1–2 (MVP → Early Network)
**Applies to:** Addressing, Transaction Validation, Policy Layer

---

## 1. Abstract

This document defines the **subject model**, **domain semantics**, and **policy-constrained transaction framework** for the PayWall Mark (PWM) protocol.

PWM introduces a domain-aware addressing system where addresses encode jurisdictional and organizational context. Transactions are not treated as unrestricted value transfers, but as **policy-bound actions between classified subjects**.

This RFC formalizes:

* Subject classes (local entities, organizations, witnesses)
* Domain structure (`domain_hi + domain_lo`)
* Authorization via co-signature (multisig as institutional validation)
* Policy-driven constraints on value movement
* Separation between **intent (marks)** and **value transfer (coins)**

---

## 2. Design Principles

PWM is designed around the following principles:

1. **Domain Awareness**
   Every address belongs to a domain with semantic meaning.

2. **Subject Classification**
   Addresses represent different types of actors with distinct capabilities.

3. **Policy over Permissionlessness**
   Not all transactions are allowed by default; validity depends on policy.

4. **Separation of Intent and Value**
   Marks express intent; coins represent constrained value transfer.

5. **Institutional Authorization**
   Multisignature is used to model real-world authority relationships.

---

## 3. Domain Model

### 3.1 Domain Encoding

Each address contains a 16-bit domain field:

```text
domain = [domain_hi: u8][domain_lo: u8]
```

* `domain_hi` defines the **domain class**
* `domain_lo` defines the **subdomain or entity selector**

### 3.2 Domain Classes (`domain_hi`)

| Class     | Description                         |
| --------- | ----------------------------------- |
| Country   | Jurisdiction-bound entities         |
| Corporate | Organizations and institutions      |
| Reserve   | Reserved for future use             |
| Witness   | Non-spendable verification entities |

### 3.3 Domain Semantics

* `domain_hi` determines **routing and policy requirements**
* `domain_lo` specifies:

  * company identifier (corporate domains)
  * future regional segmentation (country domains)
  * policy-specific meaning (witness/reserve)

---

## 4. Subject Classes

PWM defines three primary subject types.

---

### 4.1 Local Entity (Country-bound Address)

Represents a subject operating within a jurisdiction.

Examples:

* Individual user
* Branch office
* Regional operator

Capabilities (baseline):

```text
can_receive_marks = true
can_burn_marks = true
can_receive_value = true
can_send_value_local = true
can_send_value_crossdomain = restricted
```

---

### 4.2 Organization (Corporate Address)

Represents an institution as a unified entity.

Examples:

* Company
* Bank
* Exchange
* Government body

Capabilities:

```text
can_receive_value = true
can_send_value = restricted_to_members
can_cosign_member_transactions = true
can_define_membership = true
```

---

### 4.3 Witness / Compliance Entity

Non-spendable address used for authorization and validation.

Capabilities:

```text
can_store_value = false
can_receive_value = false
can_cosign = true
```

Use cases:

* Multisig security
* AML/compliance approval
* Roaming admission
* Recovery mechanisms

---

## 5. Address Semantics

An address encodes:

* Domain (routing + classification)
* Flags (policy hints)
* Subaccount identifier (identity)

However:

> Address alone MUST NOT fully determine transaction permissions.

Final behavior is determined by:

* Address class
* INIT metadata
* Policy layer

---

## 6. Transaction Model

### 6.1 Transaction Types

| Type      | Description              |
| --------- | ------------------------ |
| TRANSFER  | Local value transfer     |
| EXPORT    | Cross-domain value exit  |
| IMPORT    | Cross-domain value entry |
| BURN_MARK | Intent signaling         |
| POLICY_TX | Policy configuration     |

---

### 6.2 Domain Routing Rule

```text
if domain_hi(sender) != domain_hi(receiver):
    roaming_required = true
```

This is the roaming baseline for v1 testnet extension; legacy local-only v0 profile may operate without cross-domain routing.

---

## 7. Authorization Model

### 7.1 Multisignature as Institutional Authorization

Multisig is used not only for security, but to express **authority relationships**.

Examples:

| Signatures        | Meaning                         |
| ----------------- | ------------------------------- |
| local only        | individual action               |
| corporate only    | institutional action            |
| local + corporate | authorized institutional action |

---

### 7.2 Co-Sign Requirements

Example policy:

```text
if sender.class == local_entity AND action == cross_domain_transfer:
    require_cosign(organization)
```

This ensures:

* individuals cannot move institutional value independently
* organization must explicitly authorize

---

## 8. Membership Model

Organizations define their member set explicitly.

### 8.1 Membership Binding

```text
MemberBinding {
  org_address
  member_address
  role
  status
  activation_height
  expiry_height?
}
```

### 8.2 Policy Example

```text
organization.can_send_value_only_to_members = true
```

This enforces:

* organization → external transfer = forbidden
* organization → member transfer = allowed

---

## 9. Policy Layer

### 9.1 Policy Primitives

The system uses composable primitives:

#### Require Co-Sign

```text
require_cosign(class = organization)
```

#### Restrict Recipient

```text
recipient_must_be_member_of(org)
```

#### Burn-Only Mode

```text
allow_value_transfer = false
allow_mark_burn = true
```

#### Domain Restriction

```text
cross_domain_requires_roaming = true
```

---

### 9.2 Policy Enforcement

Transaction validity depends on:

```text
valid(tx) =
    consensus_valid(tx)
    AND policy_valid(tx)
```

---

## 10. Intent vs Value

PWM distinguishes between:

### 10.1 Marks (Intent Layer)

* Low-friction
* Always allowed unless explicitly restricted
* Used for:

  * contact initiation
  * prioritization
  * AI interaction

### 10.2 Coins (Value Layer)

* High-friction
* Subject to:

  * domain rules
  * multisig
  * policy constraints

---

## 11. Cross-Domain Transactions

### 11.1 Roaming Requirement

Cross-domain transfers MUST use:

```text
EXPORT → (certificate) → IMPORT
```

### 11.2 Admission Layer

Target domain MAY require:

* compliance approval
* multisig
* delay (quarantine)

---

## 12. Security Model

Security does NOT rely on:

* address brute-force difficulty
* domain scarcity

Security relies on:

* validator finality
* policy enforcement
* anti-replay mechanisms
* explicit authorization

---

## 13. Non-Goals

PWM is NOT:

* a general-purpose payment network
* a high-throughput value transfer system
* a permissionless financial layer

PWM is:

> a domain-aware, policy-routed trust and interaction protocol

---

## 14. Future Extensions

* Dynamic validator sets per domain
* Hierarchical organizations
* DAO-compatible subject classes
* Zero-knowledge compliance proofs
* Bitcoin anchoring for settlement

---

## 15. Conclusion

PWM introduces a model where:

* addresses represent **classified subjects**
* transactions represent **authorized actions**
* value movement is **policy-constrained**
* intent signaling remains **accessible and low-friction**

This enables a new category of network:

> **domain-aware, institutionally coherent, and abuse-resistant digital interaction infrastructure**

