# RFC 1: Address Format Baseline (Phase 1)

**Status:** Draft  
**Version:** 0.1  
**Applies to:** Phase 1 (MVP-concept)  
**Related:** [RFC 2: Subject Model](./2-subject-model.md), [Phase 1 Address Spec](../ADDRESS_SPEC_PHASE1_bech32dx.md)

---

## 1. Abstract

This RFC defines the Phase 1 baseline for PWM addresses. It specifies:

- the canonical `bech32DX` form and strict pretty form,
- the logical field layout (`version`, `domain`, `flags`, `tail`, `checksum`),
- 16-bit domain semantics (`domain_hi`, `domain_lo`) including witness class behavior,
- migration and compatibility expectations for current Phase 1 flows.

The document is normative for implementers of parsing, formatting, and recipient validation in user transaction flows.

---

## 2. Scope

This RFC covers address encoding and interpretation rules at the address-format layer.

This RFC does **not** define full transaction authorization semantics. Address semantics are policy hints and classification signals; final permissions are policy- and INIT-driven as specified in [RFC 2: Subject Model](./2-subject-model.md).

---

## 3. Address Forms

### 3.1 Canonical Form (Normative)

Canonical form MUST be represented as:

```text
pwm1<bech32dx_payload>
```

Where payload logically contains:

1. `version`
2. `domain` (Phase 1 profile: 16-bit)
3. `flags`
4. `tail` (subaccount tail)
5. `checksum`

Canonical form is the technical source-of-truth representation and MUST be accepted for input/output paths that require strict integrity (including checksum validation).

### 3.2 Strict Pretty Form (Normative UX Form)

Strict pretty form MUST be represented as:

```text
pwm1-<DOMAIN_HINT>-f<FLAGS8HEX>-t<TAIL52HEX>
```

Example:

```text
pwm1-CY/4B-f8A31C02-t57A9C1A4D3B9E00112233445566778899AABBCCDDEEFF0011
```

Rules:

- Pretty form MUST NOT embed canonical payload fragments.
- Pretty form MUST include full `TAIL52HEX` (no truncation).
- In Phase 1 CLI/TUI flows, strict pretty form MUST be the primary user-facing display form.
- Canonical and pretty forms MUST remain semantically equivalent for valid addresses.
- Unknown-domain/domain-miss display in pretty form MUST use Pascal HEX fallback with `!` marker:
  - format: `$<DOMAIN16HEX>!`
  - example: `$BF10!`
  - `DOMAIN16HEX` MUST be uppercase and preserve 16-bit width (leading zeros allowed).

---

## 4. Field Layout (Logical)

Address payload is interpreted in the following logical order:

```text
[version][domain][flags][tail][checksum]
```

### 4.1 `version`

- Identifies address format evolution.
- Parsers MUST reject unsupported versions.

### 4.2 `domain`

- Phase 1 baseline uses a strict 16-bit domain field.
- Domain is split as:

```text
domain = [domain_hi: u8][domain_lo: u8]
```

### 4.3 `flags`

- Carries address profile and purpose hints.
- In Phase 1 user-profile policy, low 10 bits are used for baseline matching/filtering.

### 4.4 `tail`

- Subaccount tail (52 hex chars in strict pretty form).
- Must be preserved exactly in strict pretty output.

### 4.5 `checksum`

- Provides error-detection integrity in canonical form.
- Canonical decoders MUST validate checksum before semantic processing.

---

## 5. Domain Classes and 16-bit Split

### 5.1 Phase 1 Domain Classes

Phase 1 domain model is defined by `domain_raw` classes:

- Country (Regulatory): 195 indexed values in `0x0300..=0xC5FF`
- Country prelude reserve: `0x0000..=0x02FF` (reserved, outside country index)
- Sector: 11 indexed values
- Reserve: range `0xE000..=0xEFFF`
- Witness: range `0xF000..=0xFFFF`

`domain_hi` remains the shard-level identity byte, while recipient policy class is resolved from the Phase 1 domain model above.

### 5.2 `domain_lo` Semantics

- Country class: reserved for future regionalization (not used in primary user flow).
- Sector class: `domain_lo` acts as a selector inside sector domains.
- Reserve/Witness classes: `domain_lo` is interpreted by class-specific policy rules.
- `domain_lo = 0x00` is valid and MUST NOT be rejected solely for being zero.

---

## 6. Witness Constraints (Normative)

Witness addresses are a dedicated class with non-spendable semantics.

Implementations MUST enforce:

- Witness address cannot store value as a regular spendable account.
- Witness address cannot be used as a normal recipient for coin transfer.
- Witness signatures are valid only in designated multisig/additional-authorization scenarios.
- Witness `flags` MUST NOT be interpreted as enabling spendability.

---

## 7. Recipient Policy Constraints (User Tx Flow)

For user-facing transaction flows (`tx-send`, `tx-burn-mark` recipient validation path in Phase 1 policy):

- Unknown domain recipient MUST be rejected.
- Reserve class recipient MUST be rejected.
- Witness class recipient MUST be rejected.
- Only recognized regular domains from the domain index MAY be accepted.

These constraints apply to baseline user flow and are consistent with the Phase 1 compatibility-first policy.
An address can therefore be decode-valid (canonical form + checksum + structural parse) but still policy-invalid as a recipient in user transaction flow.

---

## 8. Compatibility and Migration Notes (Phase 1)

Phase 1 migration expectations:

- `pwmd` API MAY remain hex-centric during transition.
- CLI/TUI MUST present strict pretty form as primary UX display.
- Runtime input paths SHOULD prioritize canonical `bech32DX` parsing with mandatory checksum validation.
- Address-book canonical storage SHOULD use canonical-only records (`bech32DX`) as source of truth.
- Legacy input (`hex`, `PWMv0-`) MAY be accepted during transition period where currently documented.

Implementations MUST NOT introduce behavior conflicting with [Phase 1 Address Spec](../ADDRESS_SPEC_PHASE1_bech32dx.md).

---

## 9. Linkage to Subject Model (RFC 2)

Address fields (domain/flags) provide classification and routing hints, not complete authorization.

Therefore:

- Address semantics MUST be treated as input to policy evaluation.
- Final transaction permissions MUST be determined by policy + INIT-driven metadata and role relationships, as defined in [RFC 2: Subject Model](./2-subject-model.md).

This separation prevents overloading address format with full authorization logic.

---

## 10. Validity Layers and Terminology

To avoid ambiguity, implementations MUST distinguish two validity layers:

1. **Decode validity (format/integrity layer):**
   - canonical parse succeeds,
   - checksum is valid,
   - version/field structure is supported.
2. **Policy acceptance (recipient/use layer):**
   - recipient class/domain is accepted by current Phase 1 user-flow policy.

Implications:

- Decode-valid does not imply policy-accepted.
- For unknown-domain/domain-miss addresses, pretty display MUST show `$<DOMAIN16HEX>!` marker even if decode-valid.
- User transaction flows MUST treat policy-invalid recipients as rejectable targets.

These definitions align with [Phase 1 Address Spec](../ADDRESS_SPEC_PHASE1_bech32dx.md) and [RFC 2: Subject Model](./2-subject-model.md), where policy rules are evaluated separately from parsing integrity.

---

## 11. Implementation Notes

- Use canonical parsing for integrity and storage truth.
- Use strict pretty formatting for user-facing consistency.
- Keep recipient checks aligned with Phase 1 baseline constraints.
- Keep policy/authorization checks aligned with RFC 2, not hardcoded from address hints alone.

