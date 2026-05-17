# Review: RFC 0018 (Effective Address Cosignatures) vs MVP V4 implementation

**Date:** 2026-05-17  
**Reviewer role:** `pwm-review` compatibility pass (RFC vs `pwm-core` / policy paths)  
**RFC:** `docs/rfc/18-effective-address-cosignatures.md` — superseded for normative text by **v0.2** (2026-05-17, implementation-aligned); this review compared against **v0.1**.  
**Code focus:** `crates/pwm-core/src/tx.rs`, `crates/pwm-core/src/state.rs` (cosign envelope, `evaluate_policy`, emergency activation)

---

## 1. Scope recap

The RFC normatively specifies transaction-level cosignatures: envelope shape, canonical signing hash over `TxBody` only, `Primary` / `Cosigner` roles, duplicate-signer rejection, effective-address checks, policy integration, fee-quota interaction, and MVP scope items including corporate INIT cosign tests and emergency activation cosign tests.

MVP V4 shipped a working cosignature **additive** field on `SignedTx`, policy-gated verification, and rescue-key cosign for emergency routing activation.

---

## 2. Requirements fit

| RFC expectation | V4 implementation | Fit |
|-----------------|-------------------|-----|
| `TxEnvelope { body, signatures }` with a **vector of peer signatures** | `SignedTx` keeps **primary** `signer_pk` + `signature` at top level; additional signs in `cosigns: Vec<Cosignature>` | **Partial:** same intent (body + primary + extra sigs), different wire/mental model |
| `TxSignature.signer_address` + `SignatureRole` Primary / Cosigner | Cosigns use **`signer_pk` only** (32-byte key); roles are **`Rescue`, `Organization`, `Witness`** — not Primary/Cosigner | **Gap:** naming and taxonomy differ; `Organization` is **unused** in core |
| `signing_hash = HASH(canonical_encode(TxBody))`; signatures **excluded** from hash | `signing_message()` prefixes `PWMv0/TX`, includes `domain_code`, `signer_pk`, `derivation_index`, `nonce`, full body (and `init_v4`, policy fee payload, etc.) | **Gap:** cosigners sign the **full PWM signing transcript**, not body-only RFC hash; aligns primary and cosigners with each other, but **not** with RFC §6 literal |
| Duplicate `signer_address` rejected | **`validate_tx_shape` does not** check duplicate `cosigns[].signer_pk` or cosigner duplicate of primary | **Gap:** RFC §6.2 / §14.3 |
| `primary_signature_present` in pipeline | Primary sig exists as today’s `SignedTx::signature`; no separate envelope field | **OK** with documentation |
| Policy requires cosign; reject before mutation | `evaluate_policy` → `PolicyMissingCosign` / `PolicyEmergencyCosignRequired`; state apply gated | **OK** |
| `CosignRequired` policy: any valid extra sig | Implementation accepts **any** cosign with valid Ed25519 over message; tests use `Witness` | **Partial:** RFC §7.3 / §14.4 — **no role-to-policy binding** for generic cosign (known V4-3 boundary) |
| Emergency: reserve cosigner matches precommitted target | `CosignRole::Rescue` + rescue account pubkey match | **OK** (semantics match §12.2; naming differs) |
| Corporate INIT: Primary + Cosigner (domain) | **No INIT-time cosign enforcement** in state; `InitV4Extension.cosign_policy` exists for metadata but is not enforced like §12.1 | **Gap** |
| Fee quota scaling by signatures / `ERR_FEE_QUOTA_EXCEEDED` | Policy `fee` on `TxBody::Policy`; no RFC-style fee quota model in this slice | **Gap** vs §11 / §15 |
| Structured errors (`ERR_DUPLICATE_SIGNER`, …) | `TxError` / RFC 14 wire mapping for **policy** rejects; duplicate signer **not** a distinct code | **Partial** |

---

## 3. Style and module shape

RFC-only review slice; production Rust style not re-scored. The RFC uses abstract types (`Address`, `HASH`, `canonical_encode`) — implementation uses concrete `AccountId`, custom `signing_message` builder, and Ed25519 `verify` — acceptable if RFC is updated to reference PWM’s existing signing transcript.

---

### Wire JSON / u128

**Wire JSON / u128:** not applicable for this RFC-vs-implementation compatibility pass (no new peer wire change in scope; `SignedTx` / `Cosignature` already follow existing serde patterns in-tree). If RFC 18 later normatively fixes JSON field types for cosignatures, add an explicit cross-reference to `pwm-core` serde for `SignedTx` and `Cosignature`.

---

## 4. Safety

- **Duplicate cosigners:** absence of duplicate checks is a **real protocol footgun** (multi-satisfy policy with one key repeated) relative to RFC claims.

- **Role confusion:** `CosignRole::Witness` satisfies generic `cosign_required`; emergency requires `Rescue`. Documented minimal semantics in RFC 6 partially cover this; RFC 18’s strict Primary/Cosigner split is **stronger** than code.

- **Canonical binding:** all signers use the **same** `signing_message()` — good for intent binding; differs from RFC body-only hash text.

---

## 5. Tests

- **Present:** emergency activation rescue cosign (good/bad/no cosign), `CosignRequired` policy gate, tampered cosign, finalized redirect paths.

- **Missing vs RFC §15 MVP list:** explicit **corporate INIT cosign** scenario as in §12.1; **duplicate signer** negative tests; **role / Primary vs Cosigner** naming alignment is not test-covered because types differ.

---

## 6. Verdict

**Approve with nits** (compatibility lens): the **spirit** of RFC 18 — multi-key authorization without multisig addresses, policy-enforced cosigns, deterministic checks before state mutation — **matches** V4 for **policy actions** and **emergency activation**.

The RFC text as **normative Draft v0.1** is **not yet aligned** with the codebase on: (1) signing transcript (body-only hash vs PWM v0 message), (2) role enum (`Primary`/`Cosigner` vs `Witness`/`Rescue`/…), (3) duplicate signer rule, (4) corporate INIT cosign enforcement, (5) fee quota section.

**Recommended follow-ups (documentation or future slice):**

1. Amend RFC 18 to reference **actual** `SignedTx.signing_message()` semantics (or define PWM canonical layer explicitly).

2. Either add **duplicate `signer_pk`** validation in `validate_tx_shape`, or soften RFC MUST language until implemented.

3. Map `Cosigner` ↔ `Witness` / `Rescue` / reserved roles in a short compatibility table.

4. Mark corporate INIT cosign as **post-V4** or implement enforcement + test.

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PARTIAL
artifacts: docs/reviews/20260517-rfc18-v4-cosign-compatibility-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 4500
  confidence: low
```

**Glossary:** not a sprint-final review; **GLOSSARY.md: без изменений** for this artifact.

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260517-rfc18-v4-cosign-compatibility-review.md'
git commit -m 'docs(review): RFC 18 vs V4 cosign compatibility'
```

*(Commit only if orchestrator or owner requests traceability commit.)*
