# RFC 0018: Effective Address Cosignatures

**Status:** Draft  
**Version:** 0.2  
**Depends on:**

  * RFC 0001 (Address Format)
  * RFC 0002 (Subject Model)
  * RFC 0006 (Policy Engine & Transaction Authorization)
  * RFC 0007 (Transaction & State Model)
  * RFC 0014 (Claim/Burn API Error Contract)

---

## Document note (v0.2)

Version 0.1 was an abstract sketch. **This revision aligns normative V4 text with the shipped `pwm-core` transaction model** (`SignedTx`, `signing_message()`, `Cosignature`, policy hooks). Wording that contradicted the code (for example, a body-only signing hash) is corrected. Reserved concepts (fee quota as a separate field, full “effective address” matrix) remain **forward-looking** where the runtime is still minimal.

---

## 1. Abstract

This document defines **Effective Address Cosignatures** for PWM.

A cosignature is **transaction-level** authorization: in addition to the **primary** Ed25519 signature on a `SignedTx`, one or more **cosignatures** attach to the same transaction. Every cosignature verifies over the **same signing transcript** as the primary signature. Cosignatures are **not** multisig addresses and **not** a separate address type.

**MVP V4** (as implemented) evaluates cosignatures inside the Policy Engine for:

  * **Policy-gated actions** when `cosign_required` is active on the target account (generic extra signature).
  * **Emergency routing activation** when activating `routing.emergency_redirect`: a **rescue** cosignature from the precommitted rescue account is required.

PWM V4 MUST NOT introduce script-addresses, threshold signature aggregation, M-of-N address scripts, or a programmable authorization VM as part of this RFC.

Longer-term goals (corporate INIT multi-sign, domain authority roles, membership flows) remain **design targets**; they are not fully enforced by state rules in the baseline described here unless called out explicitly.

---

## 2. Terminology

### 2.1 Effective Address

An **effective address** is a PWM address that can act as a protocol-recognized signer in the current validation context.

Ideally, an address is effective only if its format, domain semantics, initialization state, and policy allow the requested operation. **MVP V4** implements a **subset** of this (for example: initialized account for rescue-key resolution; domain checks for emergency redirect). Stricter “corporate approval” or frozen-account matrices are **out of scope** for the minimal V4 runtime unless extended in a later RFC slice.

### 2.2 Cosignature

A **cosignature** is an **additional** Ed25519 signature, carried in `SignedTx.cosigns`, over the **same** signing transcript as the primary signature (see Section 6). It includes the cosigner’s verifying **public key** (`signer_pk`, 32 bytes) and a **role** tag (see Section 7).

A cosignature:

  * does not create a new address type
  * does not imply persistent authority unless the transaction changes state
  * authorizes only the transaction whose signing transcript it signs
  * is evaluated under deterministic policy rules (`evaluate_policy` and helpers)

### 2.3 Primary signer

The **primary signer** is not a separate struct: it is the key identified by `SignedTx.signer_pk` / `derivation_index` and the outer `SignedTx.signature`. This is the analogue of an abstract “Primary” role in older sketches.

### 2.4 Multisig Address

A **multisig address** is deferred: an address class whose authority is M-of-N keys, script, or aggregate keys **inside the address identity**.

Multisig addresses are **not** part of this RFC.

### 2.5 Validator Attestation

Validator attestations (blocks, commitments, bootstrap) are **not** user cosignatures. Do not mix layers.

---

## 3. Design Principles

  1. Cosignatures are transaction-level, not a new ledger-level address primitive.
  2. All signatures (primary and cosign) cover the **same** PWM signing transcript (Section 6).
  3. The Policy Engine decides when cosignatures are required; rejection happens **before** state mutation.
  4. No script-address or programmable authorization VM.
  5. Validation remains deterministic.
  6. Cosignature **roles** are explicit on each `Cosignature`.
  7. **Duplicate cosigners** (same key posing as multiple parties) are unsafe; **V4 shape validation does not yet reject duplicates** (Section 6.3) — operators SHOULD treat this as a known gap until enforced.
  8. Cosignatures MUST NOT be confused with validator attestations.

---

## 4. Motivation

PWM needs multi-key approval without introducing a new address family tied to scripts or threshold crypto inside the address. **Envelope-level** cosignatures reuse existing addresses and tie approval to one concrete `SignedTx`.

Use cases:

  * **Emergency routing:** precommitted rescue key must cosign activation (implemented in V4).
  * **Governance / corporate:** INIT extension can carry `cosign_policy` metadata; **full INIT-time enforcement** of a second organizational signature is **not** implemented in state in V4 (Section 12.1).
  * **Membership / whitelist / domain authority:** policy vocabulary direction; not fully realized in minimal V4 semantics.

---

## 5. Transaction envelope (as implemented)

### 5.1 `SignedTx` shape

The live type is `pwm_core::tx::SignedTx` (JSON / bincode as elsewhere in the stack). Conceptually:

```text
SignedTx {
  domain_code: u16
  signer_pk: [u8; 32]           // primary verifying key
  derivation_index: u32
  nonce: u64
  body: TxBody
  burn_purpose, import_fee, import_provenance, init_v4  // optional fields per body
  cosigns: Vec<Cosignature>    // additive; excluded from signing transcript
  signature: [u8; 64]           // primary Ed25519 signature
}
```

```text
Cosignature {
  signer_pk: [u8; 32]
  role: CosignRole
  signature: [u8; 64]
}
```

The **account identity** of the primary signer is derived from `signer_pk` and `derivation_index` (see RFC 0001 / account derivation). Cosigners are identified by **`signer_pk` only** on the wire; index is not repeated in `Cosignature` (implementations resolve accounts as needed).

### 5.2 Relation to abstract “Vec of signatures”

Earlier drafts showed `TxEnvelope { body, signatures: Vec<…> }`. The **implemented** layout keeps the primary signature at the top level and places secondary signatures in **`cosigns`**. Behavioural intent matches: one body, one primary authorization, zero or more additional authorizations.

---

## 6. Signing transcript (PWMv0)

### 6.1 Definition

**All** signatures on a transaction (primary and every cosignature) MUST verify against the **same** message bytes produced by `SignedTx::signing_message()` in `pwm-core`.

The message begins with the domain tag **`PWMv0/TX`**, then encodes `domain_code`, `signer_pk`, `derivation_index`, `nonce`, and a **tagged, deterministic encoding** of `body` and the optional fields that affect consensus meaning (for example `init_v4` payload on `Init`, `burn_purpose` / `import_fee` / `import_provenance` where applicable).

This is **not** `HASH(simple_canonical_encode(TxBody))` in isolation: **wallet and policy documentation MUST refer to the Rust `signing_message()` implementation** as the single source of truth until a standalone normative binary spec is published.

### 6.2 Cosignatures excluded from the transcript

The **`cosigns` vector is not hashed** into `signing_message()`. Cosigners sign the **same** bytes the primary signer signed (the payload to which the primary `signature` is bound). That avoids circular dependency and matches the implementation.

### 6.3 Duplicate signers

**Intent:** the same `signer_pk` MUST NOT appear more than once across cosignatures, and a cosigner MUST NOT duplicate the primary key in a way that fakes multi-party approval.

**MVP V4:** `validate_tx_shape` **does not** reject duplicate `cosigns[].signer_pk` or overlap with `signer_pk`. Implementations SHOULD plan to add this check; until then, treat duplicate keys as a protocol footgun (see Section 14.3).

---

## 7. Signature roles (`CosignRole`)

Rust enum (serde snake_case):

```text
CosignRole:
  rescue
  organization
  witness
```

### 7.1 Mapping from abstract “Primary / Cosigner”

| Abstract role (informative) | Implemented |
|----------------------------|-------------|
| Primary | Top-level `SignedTx.signature` + `signer_pk` |
| Generic policy-required extra signer | **`witness`** — any verifying cosignature counts toward `cosign_required` in V4 minimal semantics |
| Emergency rescue approval | **`rescue`** — MUST match the rescue account’s `signing_pubkey` from state for `routing.emergency_redirect` activation |

### 7.2 `organization`

Reserved for corporate / domain workflows. **Not enforced by dedicated state logic in V4** beyond carrying related metadata in INIT; future RFCs may bind it to policy.

### 7.3 Role semantics

A role tag **labels** intent; **`evaluate_policy`** and action-specific checks decide sufficiency. For example, **`cosign_required`** accepts a valid **`witness`** cosignature in the minimal engine; emergency activation requires **`rescue`** with the correct key.

---

## 8. Validation pipeline (conceptual)

Order is illustrative; exact order matches `validate_tx_shape` → `evaluate_policy` → `apply_tx_with_ctx` in `pwm-core`:

```text
validate(signed_tx):
  assert primary_ed25519_valid(signing_message())
  assert structural / domain / shape rules (validate_tx_shape)
  assert policy decision allows tx (evaluate_policy), including cosign rules
  assert action-specific rules (e.g. rescue cosign for emergency activation)
```

Cosignature failure MUST reject before state mutation.

---

## 9. Effective address evaluation

Full matrix (frozen / pruned / per-role domain rules) is **aspirational**.

**MVP V4** highlights:

  * **Emergency activation:** rescue account must exist, be initialized, and the **`rescue`** cosignature must verify against its stored signing public key.
  * **Redirects:** incoming transfer routing may require same high-domain rescue path (policy engine + state; see RFC 0006 / MVP V4 notes).

---

## 10. Policy integration

The Policy Engine reads installed policies on relevant accounts and returns allow / reject (and redirect for emergency ingress where applicable).

**Examples expressed in policy intent (not a separate DSL on wire in V4):**

```text
cosign_required on target account  ->  at least one valid cosignature (witness path in minimal engine)
routing.emergency_redirect dormant ->  activate requires rescue cosignature; then account may finalize
```

Cosignature evaluation is **pure** with respect to policy read-only checks (no hidden writes).

---

## 11. Fees

**Abstract “fee quota” as a dedicated field** is **not** part of the current `SignedTx` cosign story.

**`TxBody::Policy`** carries an explicit **`fee`** amount (u128) in the signing transcript; policy evaluation and lifecycle use that model. Scaling fees by number of cosignatures is **not** specified in V4; future work may add accounting hooks.

---

## 12. Examples (aligned with types)

### 12.1 Corporate INIT (metadata today)

```text
TxBody::Init { index, flags }
init_v4: Some(InitV4Extension { …, cosign_policy: Some({ min_signers }), … })
```

**Implemented:** extensions and commitments are hashed into `signing_message()`; **`cosign_policy` records intent**.

**Not implemented in V4 state:** mandatory second signature from an “organization” key on INIT analogous to a corporate branch approval flow. That remains a **future enforcement** item when domain binding rules land.

### 12.2 Emergency routing activation (implemented)

Roughly:

```text
TxBody::Policy {
  target_account: Bob,
  action: ActivatePolicy { policy_id: routing.emergency_redirect },
  fee: <u128>,
}
```

Primary signer: Bob (policy target must equal sender account).

**Required:** `CosignRole::rescue` with `signer_pk` equal to the **rescue** account’s verifying key (from `rescue_address` set at INIT). After successful activation, the account becomes **finalized** for ordinary operations per MVP V4 rules (see RFC 0006 / 0007).

### 12.3 Membership add (future)

Illustrative only — no dedicated `TxBody` variant exists for this in V4 as described here.

---

## 13. Error model

Wire-level detail is in RFC 0014. At the core layer, expect variants along the lines of:

  * Bad / missing primary signature (`BadSignature`, etc.)
  * `PolicyMissingCosign` when `cosign_required` is active and no valid cosignature verifies
  * `PolicyEmergencyCosignRequired` when activating emergency routing without a valid **`rescue`** cosignature

Abstract names such as `ERR_DUPLICATE_SIGNER` are **not** exposed until duplicate detection exists.

---

## 14. Security Considerations

### 14.1 Replay and intent binding

All signatures bind to the same **`signing_message()`**, including nonce and domain fields carried there. Do not assume body-only hashing.

### 14.2 Signature substitution

A cosignature is valid for **one** transcript only.

### 14.3 Duplicate signer attack

Uniqueness of cosigner keys **should** be enforced; V4 shape checks **may not** catch duplicates yet — see Section 6.3.

### 14.4 Role confusion

`witness` MUST NOT satisfy **`rescue`** checks and vice versa. Minimal `cosign_required` does not filter by role beyond “any valid cosignature”; tightening is policy-engine work.

### 14.5 Validator vs user signatures

Keep consensus and user authorization separate.

### 14.6 Address compromise

Emergency activation requires the rescue key to cosign — reduces accidental activation; does not help if all required keys are compromised.

---

## 15. MVP V4 scope (as implemented)

**Included:**

  * `SignedTx` with `cosigns: Vec<Cosignature>`
  * PWMv0 signing transcript; cosigns excluded from hashed message; same bytes signed by all parties
  * Roles: **`rescue`**, **`witness`** (and **`organization`** reserved)
  * Policy **`cosign_required`** gate with Ed25519 verification
  * Emergency activation **`rescue`** cosignature requirement
  * Structured policy-related errors (see RFC 0014)

**Explicit gaps / non-goals in baseline:**

  * Duplicate signer rejection in `validate_tx_shape` (Section 6.3)
  * Full corporate INIT two-party enforcement (Section 12.1)
  * Fee scaling purely by signature count
  * Multisig addresses, scripts, thresholds, aggregate keys, nested cosign chains

---

## 16. Future Extensions

Future RFCs MAY define: multisig address classes, threshold / aggregate schemes, duplicate-key rules at consensus, richer `organization` binding, explorer receipts for cosign display, bootstrap snapshot lineage for policy-bearing INITs.

---

## 17. Open Questions

  * Enforce **duplicate `signer_pk`** in `validate_tx_shape` and align wire errors with RFC 0014.
  * Should **`organization`** INIT cosign be mandatory once domain registry rules exist?
  * Normative standalone spec for **`signing_message()`** bytes (non-Rust) for third-party wallets.
  * Fee policy when multiple cosignatures become common.

---

## 18. Conclusion

Effective Address Cosignatures provide multi-key authorization at the **transaction envelope** without new address classes. **V4** matches this pattern for policy cosigns and rescue-key emergency activation; **stricter corporate and duplicates rules** are documented as follow-ons.
