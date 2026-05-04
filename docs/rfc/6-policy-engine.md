# RFC 0006: Policy Engine & Transaction Authorization

**Status:** Draft
**Version:** 0.1
**Depends on:**

* RFC 0001 (Address Format)
* RFC 0002 (Subject Model)
* RFC 0003 (Roaming)
* RFC 0004 (Validator Model)
* RFC 0005 (Genesis & Bootstrap)

---

## 1. Abstract

This document defines the **Policy Engine** in PWM.

The Policy Engine determines whether a transaction is **allowed**, **restricted**, or **rejected**, based on:

* subject class (local entity, organization, witness)
* domain semantics
* multisignature requirements
* membership relationships
* cross-domain rules

PWM transactions are **not permissionless by default**.
They are **policy-constrained actions between classified subjects**.

---

## 2. Design Principles

1. **Policy is first-class**
2. **Separation from consensus**
3. **Deterministic evaluation**
4. **Composable primitives**
5. **Human-readable logic**

---

## 3. Validation Pipeline

Transaction validation MUST follow:

```text
validate(tx):
  assert consensus_valid(tx)
  assert policy_valid(tx)
```

Where:

* `consensus_valid` → RFC 0004
* `policy_valid` → this RFC

---

## 4. Policy Scope

Policy is evaluated at:

* transaction level
* sender subject level
* receiver subject level
* domain relationship level

---

## 5. Policy Inputs

```text
PolicyContext {
  sender: Address
  receiver: Address
  sender_class: SubjectClass
  receiver_class: SubjectClass
  sender_domain: u16
  receiver_domain: u16
  tx_type: TxType
  signatures: Vec<PubKey>
}
```

---

## 6. Policy Primitives

The Policy Engine is built from primitives.

---

### 6.1 Require Co-Sign

```text
require_cosign(class = organization)
```

Transaction MUST include signature from subject of given class.

---

### 6.2 Restrict Domain

```text
require_same_domain()
```

or

```text
require_roaming()
```

---

### 6.3 Restrict Recipient

```text
recipient_must_be_member_of(org)
```

---

### 6.4 Burn-Only Mode

```text
allow_mark_burn = true
allow_value_transfer = false
```

---

### 6.5 Disallow Receiving

```text
can_receive_value = false
```

---

### 6.6 Signature Role Matching

```text
require_signature_from(address)
```

---

## 7. Policy Rules Split (v1 baseline vs extensions)

---

### 7.0 Shard Semantics Clarification (normative)

For v1 baseline terminology:

- `spec-level geo-shard` means a domain-cluster with fixed `domain_hi` value.
- Runtime-level launch identity and peering prioritization model for this shard semantics is specified in `docs/rfc/8-shard-runtime-identity-and-peering.md`.
- Dev/test labels like `Shard A` and `Shard B` are allowed for process partitioning, but they are not protocol geo-shard semantics.
- "Islandization" is allowed at the domain-cluster level (operational/policy isolation of specific `domain_hi` clusters) without redefining shard identity.
- Range heuristics such as `domain_hi < 0x80` vs `>= 0x80` MUST NOT be used as a routing or policy source of truth.

---

### 7.1 MVP v1 Minimal Recipient/Domain Rules

```text
if receiver.class == witness:
    reject(tx)

if receiver.domain in {reserve, unknown}:
    reject(tx)
```

For cross-domain movement in v1:

```text
if sender.domain_hi != receiver.domain_hi:
    require_roaming()
```

`TRANSFER` remains same-shard by default; explicit cross-shard flow is `EXPORT/IMPORT`.
Route selection is protocol-derived from fixed-`domain_hi` comparison and MUST NOT be forced by API/CLI route mode parameter or by `0x80`-style range partitioning.

---

### 7.2 Burn Exception (MVP)

```text
if tx_type == BURN_MARK:
    assert sender.marks_quota >= mark_amount
    allow fee == 0 in baseline profile
    apply burn-specific recipient rules
```

Cross-domain burn context does not require target-shard state mutation; proof is handled in source shard.

---

### 7.3 Advanced Policy Extensions (post-v1)

The following rules are extension hooks and are not mandatory for v1 baseline:

```text
if sender.class == local_entity AND tx_type == TRANSFER:
    require_cosign(organization)
if sender.class == organization:
    recipient_must_be_member_of(sender)
```

```text
policy_requires_membership(sender, receiver):
    recipient_must_be_member_of(sender)
```

---

### 7.4 Extension Burn-Only Mode

```text
allow_burn_mark = true
allow_value_transfer = false
```

---

## 8. Multisignature Semantics

Multisig is interpreted semantically.

---

### 8.1 Signature Combinations

| Signatures  | Meaning                         |
| ----------- | ------------------------------- |
| local only  | individual action               |
| org only    | institutional action            |
| local + org | authorized institutional action |

---

### 8.2 Evaluation

```text
has_cosign(class):
    return any(signature belongs to class)
```

---

## 9. Membership Model

---

### 9.1 Structure

```text
MemberBinding {
  org_address
  member_address
  role
  status
}
```

---

### 9.2 Validation

```text
is_member(org, addr):
    return binding exists AND status == active
```

---

### 9.3 Usage

```text
recipient_must_be_member_of(org)
```

---

## 10. Policy Evaluation

```text
policy_valid(tx):

  if cross_domain(tx):
      assert roaming_provided(tx)

  if sender.class == local_entity:
      if tx_type == TRANSFER and extension_cosign_enabled:
          assert has_cosign(organization)

  if sender.class == organization and extension_membership_enabled:
      assert is_member(sender, receiver)

  if receiver.class == witness or receiver.domain in {reserve, unknown}:
      reject

  return true
```

---

## 11. Policy Overrides

Future versions MAY allow:

```text
PolicyOverrideTx
```

Examples:

* allow cross-org transfers
* allow external recipients
* relax cosign requirements

---

## 12. Policy Storage

Policy may be stored in:

* INIT transaction
* domain-level config
* organization-level config

MVP MAY use static rules.

---

## 13. Error Model

Transactions MUST fail with explicit reasons:

| Error                 | Meaning                            |
| --------------------- | ---------------------------------- |
| ERR_ROAMING_REQUIRED  | cross-domain without export/import |
| ERR_MISSING_COSIGN    | missing required signature         |
| ERR_NOT_MEMBER        | recipient not member               |
| ERR_INVALID_RECIPIENT | witness or forbidden               |
| ERR_POLICY_DENIED     | generic rejection                  |

---

## 14. Security Considerations

### 14.1 Policy ≠ Consensus

* validators do not enforce policy globally
* each shard enforces locally

---

### 14.2 Bypass Prevention

* all nodes MUST apply policy rules
* invalid tx MUST NOT be included in blocks

---

### 14.3 Misconfiguration Risk

Incorrect policy can:

* block legitimate flows
* allow unintended transfers

---

## 15. MVP Scope

MVP v1 MUST include:

* minimal recipient/domain restrictions
* cross-domain roaming requirement for cross-shard flow
* witness restriction
* burn exception for `BURN_MARK`

MVP v1 MUST NOT require:

* mandatory org cosign for all transfers
* mandatory membership routing for baseline operation
* dynamic policy updates / complex scripting

---

## 16. Future Extensions

* policy DSL
* programmable constraints
* zk-policy proofs
* dynamic organization governance
* compliance plugins

---

## 17. Conclusion

The PWM Policy Engine transforms transactions from:

> “value transfer between addresses”

into:

> **authorized actions between classified subjects under domain-aware constraints**

This enables:

* anti-abuse guarantees
* institutional correctness
* domain-level governance
* separation of intent and value