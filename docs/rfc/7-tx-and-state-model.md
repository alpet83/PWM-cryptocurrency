# RFC 0007: Transaction & State Model

**Status:** Draft
**Version:** 0.1
**Depends on:**

* RFC 0001 (Address Format)
* RFC 0002 (Subject Model)
* RFC 0003 (Roaming)
* RFC 0004 (Validator Model)
* RFC 0005 (Genesis & Bootstrap)
* RFC 0006 (Policy Engine)

---

## 1. Abstract

This document defines:

* the **state model** of PWM
* the **transaction types**
* the **data structures for value storage**
* the **mechanics of export/import**

For `MVP SPEC v1 testnet`, PWM keeps the **account-based ledger core** from `WHITE_SPEC_v0` and adds cross-shard roaming primitives as an extension track.

Historical UTXO-oriented constructs in this RFC are treated as optional extension design notes, not as mandatory MVP core.

* transfer outputs
* export outputs
* policy-aware validation

---

## 2. Design Goals

1. Strict-upgrade from v0 (no core ledger rewrite)
2. Deterministic state transitions
3. Compatibility with roaming
4. Explicit value lifecycle
5. Replay-safe cross-domain transfers

---

## 3. State Model Overview

Each shard maintains:

```text id="qk2puy"
State {
  Accounts
  ImportedSet
  MemberBindings
}
```

---

### 3.1 Accounts (MVP core)

Stores spendable balances and account metadata:

```text id="6sz6yv"
Account {
  balance_pwm: u128
  staked: u128
  marks: u128
  marks_quota: u128
  initialized: bool
  index: u32
  flags: u32
}
```

---

### 3.2 ImportedSet

Used for replay protection (RFC 0003):

```text id="8jxtau"
ImportedSet = Set<export_id>
```

---

### 3.3 MemberBindings

Defined in RFC 0006:

```text id="g0kjm4"
MemberBinding {
  org_address
  member_address
  status
}
```

---

## 4. Roaming Records (extension over account core)

```text id="e8q0hn"
enum RoamingRecordType {
  EXPORT_COMMIT,
  IMPORT_APPLIED
}
```

---

### 4.1 Export commit

Represents value leaving source shard and being committed for proof/finality.

### 4.2 Import applied

Represents successful import application into target shard and replay-protected mark.

---

## 5. Transaction Types

---

### 5.1 TransferTx (same-shard)

```text id="wshzcc"
TransferTx {
  from
  to
  amount
  fee
  nonce
  signatures[]
}
```

Rules:

* debit sender account
* credit receiver account
* protocol routes this tx type only when `domain_hi(sender) == domain_hi(receiver)`

---

### 5.2 ExportTx (cross-shard start)

```text id="hq1u4u"
ExportTx {
  from
  target_domain: u16
  recipient: Address
  amount: u128
  fee: u128
  nonce: u64
  signatures[]
}
```

---

#### Effects:

* debits sender account
* creates export commitment record
* selected by protocol when `domain_hi(sender) != domain_hi(receiver)`

---

### 5.3 ImportTx

```text id="k6nyxk"
ImportTx {
  export_certificate
  admission_certificate?
}
```

---

#### Effects:

* verifies export proof
* adds export_id to ImportedSet
* credits recipient account in target shard

---

### 5.4 BurnMarkTx

```text id="rf2hlg"
MarkBurnTx {
  mark_amount
  fee
  cross_domain_context?
  target?
}
```

---

#### Effects:

* burns `marks_quota` (burn-only account resource for v1 testnet baseline)
* may run with `fee = 0` in baseline profile
* no target-shard burn state mutation is required for external burn context

---

### 5.5 PolicyTx (MVP V4)

`PolicyTx` is the dedicated control-plane transaction for account policy updates. It is intentionally separate from `TransferTx`: zero-PWM self-transfer MUST NOT be used as a policy carrier, and ordinary self-transfer remains invalid.

Used to update:

* per-account policies
* policy activation state
* reversible policy deactivation
* emergency-routing activation when the required rescue cosign is present

```text
PolicyTx {
  target_account: AccountId,
  action: PolicyAction,
  fee: u128,
  nonce: u64,
  signatures[]
}

PolicyAction {
  SetPolicy { policy, activation }
  ActivatePolicy { policy_id }
  DeactivatePolicy { policy_id }
}

ActivationMode = Dormant | Immediately
```

> **Draft extension (not normative until implemented):** [ADR 0005](../adr/0005-policy-deferred-activation.md) describes a minimal third mode **`Deferred { activate_at_height }`**: policies become evaluator-active when `chain_tip_height >= activate_at_height`, without delayed execution of ordinary `Transfer` and without address flags or «conservation» semantics. The grammar in this subsection remains **baseline V4 shipped** until a normative RFC/PR lands.

Rules:

* `target_account` MUST sign every `PolicyTx`.
* `PolicyTx` pays a fee and increments the target account nonce.
* `PolicyTx` MUST NOT debit or credit PWM value except for fees.
* JSON/API encoding for `fee: u128` follows the existing PWM convention for large integer wire values: decimal string on public JSON surfaces, with binary/canonical signing preimage unchanged.
* `SetPolicy { activation = Immediately }` installs and activates in one transition.
* `SetPolicy { activation = Dormant }` stores the policy until `ActivatePolicy`.
* `DeactivatePolicy` is valid only for policies marked reversible.
* Emergency-routing activation additionally requires a signature from the `rescue_address` and finalizes the target account.

Initial policies may also appear in an extended `InitTx`, but post-init updates use `PolicyTx`.

### 5.6 Extended InitTx fields (MVP V4)

The legacy MVP fields remain valid:

```text
InitTx {
  index: u32,
  flags: u32,
}
```

MVP V4 may append an optional extension:

```text
InitV4Extension {
  owner_kind,
  owner_display_name,
  owner_country_hint,
  company_metadata_commitment,
  external_verification_ref,
  requested_domain_lo,
  rescue_address?,
  initial_policies[],
  cosign_policy?
}
```

Semantics:

* short owner/company text is canonical on-chain text with strict byte limits;
* long or mutable metadata is committed by hash and referenced externally;
* `requested_domain_lo = 0` is root/generic corporate registration, not a leased namespace;
* `rescue_address` enables emergency routing activation but does not activate it by itself;
* `initial_policies[]` follows the same `ActivationMode` lifecycle as `PolicyTx`.

---

## 6. Transaction Validation

---

### 6.1 General Flow

```text id="ajtsfd"
validate(tx):

  check_structure(tx)
  check_inputs(tx)
  check_signatures(tx)

  apply_policy(tx)      // RFC 0006
  apply_state_rules(tx)

  return valid/invalid
```

---

## 7. Input and account validation

```text id="ruo8kv"
assert sender account exists
assert sender has sufficient balance
assert owner signature valid

if tx_type == BurnMarkTx:
  assert sender.marks_quota >= mark_amount
```

---

## 8. Export Logic

---

### 8.1 Export ID

```text id="0rl9ht"
export_id = hash(txid || index || nonce)
```

---

### 8.2 State Transition

```text id="f9l8h2"
debit sender account
create EXPORT_COMMIT record
```

---

### 8.3 Constraints

```text id="j5frk8"
EXPORT_COMMIT:
  - cannot be replayed
  - only used in export proof
```

---

## 9. Import Logic

---

### 9.1 Validation

```text id="kg8y0k"
assert export_id not in ImportedSet
verify FinalityCertificate
verify AdmissionCertificate (if required)
```

---

### 9.2 State Transition

```text id="z6vtf7"
add export_id to ImportedSet
credit recipient account
```

---

## 10. Double Spend Protection

---

### 10.1 Local

Nonce + balance/state checks prevent local replay/double spend.

---

### 10.2 Cross-Domain

```text id="b2z2j8"
ImportedSet prevents re-import
```

---

## 11. Domain Enforcement

```text id="a7c92n"
if sender.domain_hi != receiver.domain_hi:
    require ExportTx
```

---

## 12. Signature Model

---

### 12.1 Ownership

```text id="rx0f84"
input must be signed by owner
```

---

### 12.2 Policy Signatures

```text id="0u3v6g"
additional signatures required by policy
```

Examples:

* organization cosign
* witness cosign
* rescue-address cosign for emergency routing activation

For MVP V4, cosignatures are part of the signed transaction envelope for `PolicyTx` or another transaction whose policy explicitly requires them. A cosignature signs the same canonical transaction intent plus its signer role; it is not a side-channel approval and it is not fetched during validation.

---

## 13. State Update

---

### 13.1 Apply Transaction

```text id="62t7dw"
apply(tx):

  update account balances and metadata
  update ImportedSet
```

---

## 14. Error Conditions

| Error               | Meaning                 |
| ------------------- | ----------------------- |
| ERR_INVALID_INPUT   | missing account/invalid input |
| ERR_INVALID_SIG     | signature failure       |
| ERR_POLICY          | policy violation        |
| ERR_EXPORT_REQUIRED | missing roaming         |
| ERR_DUP_IMPORT      | export already imported |

---

## 15. MVP Scope

MVP v1 testnet MUST include:

* account-based state core (strict-upgrade from v0)
* TransferTx (same-shard)
* ExportTx / ImportTx (cross-shard additive flow)
* ImportedSet (or equivalent used-export-id guard)
* export commitment/finality proof path
* protocol-derived shard routing by domain comparison (no forced route mode)

MVP MUST NOT include:

* script engine
* smart contracts
* mandatory full advanced policy engine

---

## 16. Future Extensions

* optional UTXO-oriented optimization layer
* programmable outputs
* confidential transactions
* batching
* multi-hop roaming

---

## 17. Conclusion

PWM state model provides:

* explicit value lifecycle
* safe cross-domain transfers
* policy-aware transaction validation
* simple implementation path

This enables a **practical MVP** that can evolve into a full multi-domain network.
