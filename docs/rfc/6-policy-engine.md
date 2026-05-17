# RFC 0006: Policy Engine & Transaction Authorization

**Status:** Draft
**Version:** 0.2
**Depends on:**

* RFC 0001 (Address Format)
* RFC 0002 (Subject Model)
* RFC 0003 (Roaming)
* RFC 0004 (Validator Model)
* RFC 0005 (Genesis & Bootstrap)
* RFC 0007 (Transaction & State Model)
* RFC 0014 (Claim/Burn/Policy API error contract)

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

### 7.3 Advanced Policy Extensions (V4 baseline)

The following rules are extension hooks and are not mandatory for v1 baseline. MVP V4 promotes a bounded subset into the runtime baseline: policy registration, activation lifecycle, emergency routing, and explicit policy rejects.

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

#### 7.3.1 Corporate INIT registration profile (V4)

The v1/v3 baseline keeps `INIT` minimal. V4 policy runtime adds an extended corporate `INIT` profile without changing the meaning of legacy minimal `INIT`:

```text
CorporateInitExtension {
  owner_kind,
  owner_display_name,
  owner_country_hint,
  company_metadata_commitment,
  external_verification_ref,
  requested_domain_lo,
  rescue_address?,
  initial_policies[],
  cosign_policy?,
}
```

Semantics:

- `requested_domain_lo = 0` means root/generic company registration inside the corporate-sector base cluster; it is not a rented domain namespace.
- `requested_domain_lo > 0` means registration against a rented or requested corporate namespace and must follow lease/auction policy.
- Metadata storage is hybrid: short public fields (`owner_kind`, `owner_display_name`, `owner_country_hint`) are canonical on-chain text with strict byte limits; long, mutable, or private metadata is represented by `company_metadata_commitment` plus `external_verification_ref`.
- `rescue_address` is optional, but emergency routing activation is impossible without it.
- `initial_policies[]` may install policies with `activation = immediately` or `activation = dormant`; policies that are not present in `INIT` can be added later by `PolicyTx`.
- `cosign_policy` links corporate registration to multisig/membership rules and is also reused by emergency routing activation when applicable.

This subsection is an implementable V4 profile, but field byte limits, canonical text encoding, and exact serialization live in RFC 0007 and implementation tickets.

#### 7.3.2 Policy registration and activation lifecycle (V4)

Policy updates MUST use dedicated control-plane transactions, not zero-value self-transfers. Normal `TRANSFER` with `from == to` remains invalid, because it has historically been a source of accounting ambiguity and should not carry policy semantics.

```text
PolicyTx {
  target_account,
  action,
  fee,
  nonce,
  signatures[],
}

PolicyAction {
  SetPolicy { policy, activation }
  ActivatePolicy { policy_id }
  DeactivatePolicy { policy_id }
}

ActivationMode = Dormant | Immediately
```

> **Draft extension:** third mode **`Deferred`** and chain-height scheduling are specified in [ADR 0005](../adr/0005-policy-deferred-activation.md). Not evaluator-normative for shipped V4 until RFC 0006/0007 are updated accordingly.

Rules:

- `SetPolicy { activation = Immediately }` installs and activates the policy in the same state transition.
- `SetPolicy { activation = Dormant }` stores the policy without affecting ordinary transaction validation until an `ActivatePolicy` is accepted.
- `DeactivatePolicy` is allowed only for reversible policies. System policies may be explicitly irreversible.
- `PolicyTx` pays a normal fee and increments the target account nonce; it never transfers PWM value to the target.
- `PolicyTx` is the only V4 mechanism for dynamic policy updates. `INIT` may only install initial policies during account registration.

#### 7.3.3 System policies and emergency routing (V4)

V4 distinguishes user policies from protocol/system policies. System policies are still represented as enum variants, not scripts or callbacks.

V4 system policy set:

- `routing.same_domain_only`
- `routing.emergency_redirect`
- `sender_filter`
- `default_behavior`
- `cosign_required`

Emergency routing is a special system policy:

```text
EmergencyRoutingPolicy {
  rescue_address,
  activation = Dormant | Immediately,
  finalizes_account = true,
}
```

Activation rules:

- Emergency routing requires `rescue_address` from the extended `INIT` profile or from an accepted policy definition.
- Emergency activation requires the target account signature and a cosignature from `rescue_address`.
- Once active, emergency routing finalizes the target account. Finalization is irreversible in MVP V4.
- After finalization, possession of the old private key no longer authorizes ordinary spend/control actions from the finalized account.
- Incoming value transfers to the finalized account are deterministically redirected to `rescue_address` by the state transition, or rejected if routing cannot be applied under the current shard/domain rules.
- Policy evaluation remains pure: it returns a routing/finalization decision; only the apply path mutates balances or policy state.

Out of scope for V4: policy DSL, programmable constraints, domain lease auctions, full organization membership registries, and governance plugins.

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
evaluate_policy(tx, read_only_state):

  if cross_domain(tx):
      assert roaming_provided(tx)

  if target_account.finalized:
      allow only explicitly permitted system-policy actions
      route incoming value to rescue_address when emergency routing is active

  if sender.class == local_entity:
      if tx_type == TRANSFER and extension_cosign_enabled:
          assert has_cosign(organization)

  if sender.class == organization and extension_membership_enabled:
      assert is_member(sender, receiver)

  if receiver.class == witness or receiver.domain in {reserve, unknown}:
      reject

  return PolicyDecision::Allow | Redirect | Reject
```

`evaluate_policy` MUST be a pure function: no state writes, no callbacks, no IO, and no dependency on wall-clock state outside the block context already supplied to validation.

### 10.1 MVP V4-3 minimal semantics

V4-3 implements the evaluator before the richer policy parameter model. The following limitations are intentional and must not be presented as final corporate governance semantics:

- `sender_filter` is a conservative incoming-transfer deny placeholder while no on-chain allow-list is available. A future slice may add an explicit allow-list or membership binding contract.
- `default_behavior` is interpreted as default-deny for incoming `TRANSFER` when active; there is no separate `allow` parameter in V4-3.
- `cosign_required` validates that at least one embedded cosignature signs the same canonical transaction intent. It does not yet bind the signer to an organization/member registry. Emergency rescue-address cosign semantics are specified for V4-4 and must be stricter than this generic scaffold.
- `routing.emergency_redirect` in V4-4 applies to incoming `TRANSFER` only: a finalized account with active emergency routing redirects same-shard incoming transfers to `rescue_address`. Other value ingress classes, such as `IMPORT`, do not participate in emergency redirect until a later RFC explicitly extends them.

---

## 11. Policy Actions

MVP V4 policy actions are bounded control-plane state transitions:

```text
SetPolicy
ActivatePolicy
DeactivatePolicy
```

These actions may install or toggle enum policies. They MUST NOT carry scripts, dynamic predicates, external callbacks, or arbitrary bytecode.

---

## 12. Policy Storage

Policy may be stored in:

* INIT transaction
* account policy state
* domain-level config
* organization-level config

V4 stores per-account policies in account state. Domain-level and organization-level config remain extension points unless explicitly used by a V4 slice.

---

## 13. Error Model

Transactions MUST fail with explicit reasons. API-facing policy reject codes use
the additive `E_POLICY_*` wire contract defined in RFC 0014:

| Wire code | Meaning |
| --------- | ------- |
| `E_POLICY_SCHEMA_INVALID` | malformed policy payload |
| `E_POLICY_NOT_INSTALLED` | policy is not installed for the account |
| `E_POLICY_NOT_ACTIVE` | policy exists but is not active |
| `E_POLICY_DENIED` | generic deterministic policy rejection |
| `E_POLICY_SENDER_FILTERED` | sender/filter policy rejected the transaction |
| `E_POLICY_ROUTING_DENIED` | routing policy rejected the transaction or redirect |
| `E_POLICY_MISSING_COSIGN` | required generic cosignature is absent or invalid |
| `E_POLICY_RESCUE_REQUIRED` | emergency routing requires a rescue address |
| `E_POLICY_EMERGENCY_COSIGN_REQUIRED` | emergency activation lacks a valid rescue cosignature |
| `E_POLICY_ACCOUNT_FINALIZED` | finalized account rejects the requested old-key action |
| `E_POLICY_IRREVERSIBLE` | requested policy transition is irreversible or cannot be undone |

Older non-`E_POLICY_*` conceptual labels may appear in historical explanatory
material, but they are not the stable JSON/API wire codes for MVP V4.

---

## 14. Security Considerations

### 14.1 Policy evaluation boundary

* policy evaluation is not a VM or external service callback
* validators enforce the deterministic policy rules of their shard before including transactions
* domain/global governance policies remain explicit state/config inputs, not hidden runtime plugins

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

MVP V4 MUST include:

* dedicated `PolicyTx` / policy action transactions;
* per-account policy state with `Dormant` and `Active` lifecycle;
* hybrid corporate INIT metadata profile;
* emergency routing with rescue-address cosign and irreversible account finalization;
* pure enum-based policy evaluation and structured reject errors.

MVP V4 MUST NOT include:

* self-transfer as a policy carrier;
* policy DSL / VM / bytecode;
* external service callbacks during validation;
* production domain lease auctions or full organization governance.

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
