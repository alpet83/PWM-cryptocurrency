# Review: V6-2 slice 4 — V6 reject codes + validate_tx_shape stubs

**Ticket:** `tasks/20260605-v6-s2-slice4-reject-stubs.json`  
**Worktree:** `P:/opt/docker/PWM-cryptocurrency-worktrees/v6-sprint2-core-model`  
**Reviewer:** pwm-review  
**Date:** 2026-06-05

---

## 1. Scope recap

Slice 4 targets **V6-2 sprint table row 4** (`docs/plans/mvp_v6.md` § Sprint V6-2): additive `TxError` variants and stable HTTP reject codes (`E_POLICY_*`, `E_CONSERVATION_*`, `E_EVIDENCE_*`), plus **shape-only** `validate_tx_shape` stubs — **no apply enforcement**.

Claimed diff:

| Path | Change |
|------|--------|
| `crates/pwm-core/src/tx.rs` | New `TxError` variants; `ActivatePolicy` shape guard for non-emergency `activation_target`; unit test |
| `crates/pwm-core/src/reject_wire.rs` | Centralized `tx_err_wire`; V6 code mapping tests |
| `crates/pwmd/src/api/common.rs` | Removed duplicate `tx_err_wire`; imports core mapping |
| `crates/pwmd/Cargo.toml` | `0.1.66` → `0.1.67` |
| `issues-report.md` | Documents deferred activation-target enforcement |

Normative anchors: ADR 0011 (activation target rejects), ADR 0010 (`E_EVIDENCE_DUPLICATE`), umbrella brief «additive reject stubs, no apply logic».

---

## 2. Requirements fit

**Met for slice scope.**

- Eight new `TxError` variants align with ADR 0011 / ADR 0009 / ADR 0010 stub lists.
- Wire mapping covers all new variants with stable `E_*` strings and `response_class` values consistent with existing policy rejects.
- `validate_tx_shape` enforces **only** `PolicyActivationTargetNotAllowed` when a non-emergency policy carries `activation_target: Some(_)` — matches ADR 0011 § «reject if non-null and policy is not emergency» and the issues-report decision to defer `required` / `mismatch` / fee-zero checks to V6-6/V6-7.
- `PolicyActivationTargetRequired`, `PolicyActivationTargetMismatch`, `PolicyActivationFeeMustBeZero`, conservation and flag errors exist as **types + wire stubs** without state/apply emission — correct for «no enforcement logic yet» acceptance.
- Refactoring `tx_err_wire` into `pwm-core::reject_wire` gives a single source of truth for pwmd (and future CLI) — good structural fit.

**Gaps (acceptable within slice, track forward):**

- No integration test asserting `/v1/tx` JSON body for a live `DuplicateImport` path (unit mapping only).
- `docs/pwmd.md` stable reject list not updated with new V6 codes (doc lag).
- Full activation-target contract (fee=0, target required/match rescue) intentionally deferred — documented in `issues-report.md`.

---

## 3. Style and module shape

- **`python scripts/check_entity_name_segments.py`** on touched paths: **zero violations** (prod ≤4, test ≤5).
- Module banners present on `reject_wire.rs` and `tx.rs`.
- Test names (`maps_v6_pol_evd_codes`, `pol_act_tgt_non_emerg`) within policy.
- `tx_err_wire` relocation removes ~30 lines of duplication from `common.rs` without expanding façade modules.
- `reject_wire.rs` now depends on `tx::TxError`; dependency direction is acceptable (wire helper ← domain errors).

Minor style note: `tx_err_wire` is public in `reject_wire` but not re-exported at `pwm_core` root (only `summarize_tx_reject_json` is). Current pwmd import path is fine; optional future `pub use` if CLI needs direct mapping.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice). Changes affect HTTP `/v1/tx` reject JSON and core `TxError` mapping only; no `PeerWireMsg`, handshake, or new on-wire serde fields. Existing `activation_target: AccountId` serde from slice 2 unchanged.

---

## 4. Safety

- Shape guard is deterministic and bounded; no new panics or unchecked trust boundaries.
- Centralized mapping reduces drift risk between pwmd and core.
- **Wire code stability — intentional change:** `DuplicateImport` previously fell through to `E_SCHEMA_INVALID` / `VALIDATION_ERROR`; now maps to `E_EVIDENCE_DUPLICATE` / `STATE_CONFLICT`. HTTP status remains `409 CONFLICT` (`handlers_tx.rs`). This is a **client-visible JSON contract change** (code + `response_class`), not a peer-protocol break. Low operational risk but operators parsing `error.code` should be aware.
- **`DuplicateImport` → `E_EVIDENCE_DUPLICATE` semantics:** ADR 0010 defines `E_EVIDENCE_DUPLICATE` for duplicate slashing **evidence records** (`record_id` replay). `DuplicateImport` is import **export_id** replay (RFC 9 / long-standing state guard). Collapsing both into one wire code is **pragmatic for stub slice** (shared `STATE_CONFLICT` bucket) but **overloads the evidence name** for import replay. Recommend documenting the alias in `docs/pwmd.md` or splitting to a dedicated `E_DUPLICATE_IMPORT` when import reject taxonomy is next touched — not a blocker for stub-only slice.
- `EvidenceDuplicate` variant is stub-only (not yet emitted from apply) — safe.

---

## 5. Tests

**Present:**

- `reject_wire::tests::maps_v6_pol_evd_codes` — all new V6 mappings including `DuplicateImport` and `EvidenceDuplicate`.
- `tx::tests::pol_act_tgt_non_emerg` — shape rejection for non-emergency `activation_target`.
- Existing slice-2 JSON/signing tests unchanged and still relevant.

**Ran locally:** `cargo test -p pwm-core -- reject_wire` and `pol_act_tgt_non_emerg` — **PASS**.

**Missing (non-blocking for stub slice):**

- pwmd handler test for `DuplicateImport` reject JSON shape after mapping move (regression guard for HTTP layer).
- Negative test that emergency activate **without** target still passes shape (confirms deferred enforcement).

---

## 6. Verdict

**Approve with nits.**

Prioritized nits for pwm-coding or follow-up slice:

1. **Medium (docs):** Update `docs/pwmd.md` reject code table with V6 additive codes; note `DuplicateImport` now emits `E_EVIDENCE_DUPLICATE` / `STATE_CONFLICT` (replacing generic `E_SCHEMA_INVALID`).
2. **Low (naming):** Consider whether import replay should keep a distinct `E_DUPLICATE_IMPORT` long-term vs sharing `E_EVIDENCE_DUPLICATE` with slashing evidence — align with ADR 0010 wording in a short ADR addendum or pwmd doc footnote.
3. **Low (pwmd version):** `0.1.67` bump is **justified** (HTTP reject contract + build marker); add one-line note in ticket/commit message that this is reject-API alignment, not peer wire — no `PWM_PROTOCOL_VERSION` change required.

No REQUEST_CHANGES items for production code in this slice.

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260605-v6-s2-slice4-reject-stubs-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 22000
  confidence: medium
```

---

**Verdict (one line):** APPROVE_WITH_NITS — slice meets V6-2 stub scope; wire mapping centralized correctly; flag DuplicateImport→E_EVIDENCE_DUPLICATE doc/naming for follow-up.
