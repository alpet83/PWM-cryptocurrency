# Review: MVP V4-4 emergency routing and finalized account behavior

**Status:** Initial run: **PASS_WITH_NITS** (2026-05-17). **Final re-review (post nit closure):** **PASS** — see [Addendum A](#addendum-a-final-re-review-post-pass_with_nits-closure).

**Ticket:** `tasks/20260517-v4-sprint4-emergency-routing.json`  
**Scope (claimed):** `crates/pwm-core/src/state.rs` — emergency activation (rescue cosign), irreversible finalization, finalized sender restrictions, incoming `Transfer` redirect to rescue; aligned with `docs/plans/mvp_v4.md` Sprint V4-4, RFC 6 §7.3.3 / §10, RFC 7 cosign envelope, RFC 14 `E_POLICY_*` contract.  
**Reviewer:** pwm-review (independent)  
**Date:** 2026-05-17  

---

## 1. Scope recap

The ticket targets Sprint V4-4: rescue-backed emergency policy activation, `finalized` account semantics, deterministic crediting of incoming **transfers** to `rescue_address` when emergency routing is active, stable policy errors, evaluator/apply separation, and no unrelated policy DSL/registry work. Implementation is confined to `pwm-core` state transition and in-module tests (`state.rs`).

---

## 2. Requirements fit

### Findings (severity order)

1. **Low — RFC 6 narrative drift (documentation).** Section **10.1 “MVP V4-3 minimal semantics”** still states that `routing.emergency_redirect` is reserved and that the V4-3 implementation “does not move balances or finalize accounts,” with apply behavior deferred to V4-4. V4-4 is now implemented in `state.rs`; this paragraph should be revised so readers are not directed at obsolete behavior. **Impact:** spec confusion only; no code defect.

2. **Low — Ingress scope boundary.** Emergency redirect is wired only for **`TxBody::Transfer`** (via `redir_to`). Other value ingress (e.g. **`Import`** still credits `to` as written) does not consult `PolicyDecision::Redirect`. Sprint acceptance wording emphasizes **incoming transfer**; RFC 6 uses “incoming value transfers,” which could be read more broadly. **Recommendation:** treat as an explicit MVP scope line (“redirect applies to `Transfer` only”) in RFC/plan if cross-ingress parity is not desired yet; otherwise backlog.

3. **Low — Error bucketing for rescue account not ready on activation.** `validate_pol_action` maps both “missing/uninitialized rescue row” and “no valid `CosignRole::Rescue` signature” to `TxError::PolicyEmergencyCosignRequired`. RFC 14 ties this code to missing/invalid rescue cosign; uninitialized rescue account is adjacent but not textually distinct. Acceptable for MVP; optional follow-up for finer RPC observability.

### Requirements checklist (verification)

| Goal | Status |
|------|--------|
| Emergency activation requires **target (PolicyTx) signature** plus **rescue-address cosign**, not arbitrary cosign | **Met.** Primary signature is validated via normal account binding; `has_role_cosign` requires `CosignRole::Rescue`, matching `rescue_id`’s **initialized** account `signing_pubkey`, and valid Ed25519 over canonical intent. Generic `has_valid_cosign` / witness cosign does not satisfy emergency. |
| Missing rescue / missing or bad rescue cosign → stable rejects | **Met.** `PolicyRescueRequired` when `rescue_address` is `None`; `PolicyEmergencyCosignRequired` when rescue row missing/uninitialized or cosign missing/invalid (tests: `policy_emerg_act_*`). |
| Successful activation finalizes irreversibly and activates emergency policy | **Met.** `apply_policy_action` sets `active_policies` bit, clears dormant bit, sets `finalized = true` for `RoutingEmergencyRedirect`; emergency is irreversible via `is_reversible()`. |
| Finalized old key cannot spend/stake/set arbitrary policies | **Met.** `evaluate_policy` rejects with `PolicyAccountFinalized` when sender is finalized and `is_finalized_blocked` applies; `Policy` actions are narrowed by `finalized_policy_allowed` to only the one-shot dormant emergency activation path. |
| Incoming **transfer** to finalized+emergency credits rescue, not old account | **Met.** Redirect in evaluator + `dst = redir_to.unwrap_or(*to)` in apply; test `policy_xfer_rescue_credit`. |
| Missing rescue on recipient with active emergency + finalized | **Met.** `eval_emerg_redirect` → `PolicyRescueRequired`; test `policy_xfer_no_rescue`. |
| Uninitialized rescue row on incoming transfer | **Met.** `RecipientNotInitialized`; test `policy_xfer_rescue_no_init`. |
| Cross-domain redirect disallowed | **Met.** `same_hi_domain` gate → `PolicyRoutingDenied`; aligns with “shard/domain rules” in RFC 6 / plan. |
| No mutation on failed activation | **Met.** `policy_emerg_act_no_cosign` and `policy_emerg_act_bad_cosign` assert full state and fee pool equality vs clone before apply. |
| Scope creep beyond V4-4 emergency routing | **No material creep observed** in reviewed file; changes are localized to policy/emergency/finalized/transfer redirect. |

---

## 3. Style and module shape

- Module has a clear `//!` banner; production helpers remain focused.
- `python scripts/check_entity_name_segments.py crates/pwm-core/src/state.rs` → **no violations** (prod ≤4 segments, tests ≤5).
- `ExportProvenance` already uses `#[serde(with = "crate::ser_json_u128")]` for JSON snapshot fields (unchanged pattern).

### Wire JSON / u128

**Scope:** This slice does not change peer wire payloads (`PeerWireMsg`, sync JSON), framed RPC bodies, or normative RFC wire field definitions. It updates **ledger `State` transition logic** and tests.

**Assessment:** Wire JSON / u128: **not applicable (no peer wire / RFC wire contract change in this slice).** Existing `u128` in `ExportProvenance` remains serde_json-safe via `ser_json_u128`.

---

## 4. Safety

- **Evaluator purity:** `evaluate_policy` only reads state and returns `PolicyDecision`; redirects and finalization are applied only after the decision in `apply_tx_with_ctx`.
- **Cosign binding:** Emergency path avoids the generic “any valid cosign” pitfall by requiring role `Rescue` and pubkey match to rescue account identity.
- **Trust / panic surfaces:** Transfer apply uses `expect("recipient gated")` after recipient checks; consistent with existing patterns (`require_recipient` / initialized filter). Not introduced as a new trust class in this slice.
- **Finalization bypass:** `Init` on already-initialized account fails before policy; cannot clear `finalized` via `Init`.

---

## 5. Tests

**Observed (from codebase + ticket):** focused tests cover rescue missing, cosign missing, bad cosign **with no-mutation asserts**, happy-path finalization, finalized sender blocked operations (`Transfer`, `Stake`, `SetPolicy`), redirect credit to rescue, missing rescue on transfer, uninitialized rescue on transfer.

**Testing report (per ticket / pwm-testing):** PASS — `cargo test -p pwm-core policy_` (19/19), `cargo test -p pwm-core --lib` (133 passed, 1 ignored), `cargo check -p pwm-core`, `cargo check -p pwmd`; bad rescue cosign path retested after no-mutation hardening.

**Gaps (non-blocking):** no dedicated unit test for **same-domain check failure** on redirect (cross-`hi` rescue) — behavior is present in `eval_emerg_redirect` but not asserted in-repo in the reviewed tests.

---

## 6. Verdict

**PASS_WITH_NITS** — Core security and acceptance behavior for V4-4 emergency routing match the ticket and RFC intent for `Transfer`-based ingress; tests and no-mutation hardening are strong. Nits: update RFC 6 §10.1 stale “V4-3 redirect/finalize” wording; clarify whether non-`Transfer` ingress should ever participate in emergency redirect; optional test for cross-domain redirect deny.

---

## 7. Participation / token estimate

```json
{
  "agent": "pwm-review",
  "result": "PASS_WITH_NITS",
  "artifacts": "docs/reviews/20260517-v4-sprint4-emergency-routing-review.md",
  "token_usage": {
    "source": "estimate",
    "input": 12000,
    "output": 4800,
    "total": 16800,
    "confidence": "medium"
  }
}
```

*(Not sprint-final V4; no `docs/GLOSSARY.md` update required per `docs/AGENT_PROMPT_review.md`.)*

---

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260517-v4-sprint4-emergency-routing-review.md'
git commit -m 'docs(review): V4-4 emergency routing independent review'
```

_(Orchestrator: run only if committing traceability; user requested no commit from this session.)_

---

## Addendum A: Final re-review (post PASS_WITH_NITS closure)

Re-review scope: confirm documentation and tests close the items flagged in the initial **PASS_WITH_NITS** verdict (RFC drift, Transfer-only redirect clarity, cross-domain redirect test). Implementation files were spot-checked only where needed for traceability; full `cargo test` was not re-run in this review session (orchestrator reported pwm-testing **PASS**, `policy_` **20/20**, including `policy_xfer_rescue_cross_deny`).

### A.1 Nit closure matrix

| Initial finding | Close-out |
|-----------------|-----------|
| **RFC 6 §10.1** stale claim that V4-3 reserved emergency redirect / deferred apply | **Closed.** `docs/rfc/6-policy-engine.md` §10.1 now documents V4-4: `routing.emergency_redirect` applies to incoming **`TRANSFER` only** (same-shard redirect text); **`IMPORT`** and other ingress excluded until a later RFC. |
| **Ingress scope** (redirect only in `Transfer` apply path vs broader RFC wording) | **Closed.** `docs/plans/mvp_v4.md` Sprint V4-4 states redirect applies to incoming **`Transfer` only** and defers `Import`/cross-shard ingress parity to backlog unless a later RFC extends semantics. |
| **Missing test** for cross-`hi`/cross-domain rescue redirect rejection | **Closed.** `policy_xfer_rescue_cross_deny` in `crates/pwm-core/src/state.rs` expects `TxError::PolicyRoutingDenied` and asserts no `accounts` / `fee_pool` mutation vs `st.clone()` before apply. |
| **Coarse** `PolicyEmergencyCosignRequired` for multiple “rescue not ready” situations | **Acknowledged (optional follow-up).** Unchanged by design; not treated as a release blocker (as in initial review). |

### A.2 Normative text consistency (informational)

Section 7.3.3 of RFC 6 still uses the phrase “Incoming value transfers” in one activation bullet; §10.1 and MVP v4 now narrow **implementation scope** to `TRANSFER` only. No contradiction requiring a gate: §10.1 is the explicit V4-4 scope pin.

### A.3 Wire JSON / u128 (re-review)

**Unchanged:** not applicable — follow-up touched docs and unit tests, not peer wire or normative wire types.

### A.4 Final verdict (re-review)

**PASS** — Tracked nits are closed in repository state; no remaining blockers identified for the V4-4 emergency routing slice against the stated scope.

### A.5 Participation / token estimate (re-review session)

```json
{
  "agent": "pwm-review",
  "result": "PASS",
  "artifacts": "docs/reviews/20260517-v4-sprint4-emergency-routing-review.md",
  "token_usage": {
    "source": "estimate",
    "input": 4500,
    "output": 3200,
    "total": 7700,
    "confidence": "medium"
  },
  "note": "Re-review only; see Section 7 for original review token estimate (PASS_WITH_NITS)."
}
```
