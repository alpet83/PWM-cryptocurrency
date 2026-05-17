# Review: MVP V4-1 doc-only slice — policy RFC freeze

**Reviewer role:** `pwm-review`  
**Date:** 2026-05-17  
**Artifact paths:** `docs/rfc/6-policy-engine.md`, `docs/rfc/7-tx-and-state-model.md`, `docs/rfc/14-claim-burn-api-error-contract.md`, `docs/CONCEPT_ROADMAP.md`, `docs/plans/mvp_v4.md`, `tasks/20260517-v4-sprint1-policy-rfc-freeze.json`

---

## 1. Scope recap

The slice aims to **freeze an implementable V4 policy contract before product Rust**: dedicated policy transactions (no self-transfer carrier), activation lifecycle, hybrid corporate `INIT` + `rescue_address`, emergency routing with rescue cosign + irreversible finalization, pure `evaluate_policy`, additive RFC 14 policy error codes, and alignment across roadmap / RFCs / `mvp_v4` plan.

The task JSON scopes checklist pointers to CONCEPT_ROADMAP V4 section and RFCs 6, 7, 14; plan `mvp_v4` Sprint V4-1 lists the same files plus roadmap.

---

## 2. Requirements fit

| Requirement | Assessment |
|-------------|------------|
| Dedicated `PolicyTx` / policy actions; self-transfer not carrier | **Met.** RFC 6 §7.3.2 and RFC 7 §5.5 normatively forbid zero-PWM self-transfer as policy carrier and keep ordinary self-transfer invalid. |
| `ActivationMode = Dormant \| Immediately`; Set / Activate / Deactivate | **Met.** Defined consistently in RFC 6 §7.3.2–7.3.3 and RFC 7 §5.5; lifecycle rules spelled out. |
| Hybrid corporate INIT + `rescue_address` | **Met.** RFC 6 §7.3.1 `CorporateInitExtension` and RFC 7 §5.6 `InitV4Extension`; hybrid on-chain short fields + commitment + external ref. |
| Emergency activation: target signature + rescue cosign | **Met.** RFC 6 §7.3.3 activation rules; RFC 7 §5.5 (`target_account` MUST sign; emergency activation needs rescue signature); RFC 7 §12.2 examples. |
| Finalization irreversible in V4 | **Met.** RFC 6 §7.3.3 (“irreversible in MVP V4”), §7.3.2 deactivate only for reversible policies / system irreversible. |
| `evaluate_policy(tx, &ReadOnlyState) -> PolicyDecision` pure; no VM / DSL / callbacks | **Met.** RFC 6 §10–§11, §14.1, §15 MUST NOT list; separation of evaluation vs apply stated (§7.3.3 last bullets). |
| RFC 14 additive; Claim/Burn/Import stable | **Met.** Explicit additive policy codes, compatibility clause (must not change semantics of existing codes). |

**Gaps (non-blocking for “concept freeze”, relevant for next sprint):**

- RFC 7 §3.1 `Account` struct still lacks normative **storage fields** for per-account policy blobs, `finalized`, persisted `rescue_address` — acceptable if V4-2 intentionally owns the serialized model, but implementers will pull finalized/routing behavior mainly from RFC 6 prose until §3.1 is extended.
- RFC 6 §5 `PolicyContext` remains a generic sketch and does not enumerate PolicyTx-specific inputs; fine for narrative baseline, thin for codegen-from-spec.

---

## 3. Style and module shape

Doc-only slice; **`AGENT_PROMPT_coding.md`** production naming rules **not applicable**.

Notable **terminology drift** across documents:

- Roadmap bullet uses enum label **`emergency_routing`**; RFC 6 names system policy **`routing.emergency_redirect`**. Same concept; naming should be unified before wire/error UX hardening.
- Plan / roadmap pair **`PolicyTx` / `PolicyActionTx`**; RFCs define **`PolicyTx`** carrying an embedded **`PolicyAction`** enum — no distinct “PolicyActionTx” type in RFC text. Clarify whether “PolicyActionTx” is informal shorthand or a planned second tx kind.

---

### Wire JSON / u128

**Scope:** RFC 7 touches transaction shapes (`fee`, `amount` as `u128` on existing tx types); this slice adds **`PolicyTx`** with `fee: u128`. RFC text does **not** normatively specify peer JSON encoding for large integers (decimal string vs crate helpers). No new mandatory peer-wire field typing is fully pinned for PolicyTx-only payloads in RFC body.

**Assessment:** For this **pre-code freeze**, flag as **documentation follow-up for V4-2 wire slice**: when `PolicyTx` hits serde JSON on the wire, normative alignment with existing `pwm_core::ser_json_u128`-style patterns should be stated or cross-referenced. Not treated as blocking RFC freeze unless orchestrator treats wire ambiguity as exit criterion for V4-1 (plan says “no unresolved wire ambiguity” — currently **partial ambiguity** remains on PolicyTx JSON encoding).

---

## 4. Safety

Documentation-level:

- Emergency routing correctly requires **two-party authorization** for activation; finalized account stripping ordinary spend authority is stated.
- **Potential ambiguity:** RFC 6 §10 pseudocode allows “only explicitly permitted system-policy actions” on finalized accounts — the **enumeration** of what remains permitted is not closed; acceptable as intentional deferral but increases interpretation risk during implementation.

---

## 5. Tests

No executable tests in slice (docs + task JSON only). Acceptance for V4-1 is documentary; tracing to future tests is via plan V4-3/V4-4 acceptance criteria — **not missing for this slice**.

---

## 6. Verdict

**PASS_WITH_NITS**

Prioritized nits:

1. **Roadmap inconsistency:** `docs/CONCEPT_ROADMAP.md` still states that INIT field formats “should be moved to a separate RFC/ADR before V4 implementation” (approx. section on corporate domains). RFC 6/7 **already** specify `CorporateInitExtension` / `InitV4Extension`. Update roadmap prose to reference those RFC sections **or** explicitly retire that sentence — otherwise freeze reads internally contradictory.
2. **Terminology alignment:** unify `emergency_routing` vs `routing.emergency_redirect`; clarify **`PolicyActionTx`** vs embedded `PolicyAction` in `PolicyTx`.
3. **Wire clarity:** add a single normative sentence or cross-ref for **`u128` JSON encoding** when PolicyTx appears on JSON-RPC/wire (defer to RFC 7 appendix or implementation crate doc per project convention).

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
verdict: PASS_WITH_NITS
result: PASS_WITH_NITS
artifacts:
  - docs/reviews/20260517-v4-sprint1-policy-rfc-freeze-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 38000
  confidence: low
notes: Approximate aggregate (prompt + six scoped files full read + draft report); no provider token meter available.
```

---

## 8. Sprint-final glossary traceability

Not a sprint-final review. **GLOSSARY.md:** no update required for this sub-slice.

---

**Verdict (one-line):** PASS_WITH_NITS
