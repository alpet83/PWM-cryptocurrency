# Review: V5 TUI — F5 hint 3-state + operator copy polish

**Date:** 2026-06-02
**Agent:** pwm-review
**Ticket:** `20260602-v5-tui-f5-hint-three-state-coding`
**Prior review:** `docs/reviews/20260602-v5-pwm-tui-build-regression-review.md`

---

## 1. Scope recap

This slice resolves NIT #1 and NIT #2 from the build-regression review:

- **NIT #1 (Medium):** `f5_burn_hint_text` was 2-state only (staked==0 && marks==0 gate). The "staked > 0 but marks not yet accrued" branch was missing, causing confusing UX for fresh-staked operators.
- **NIT #2 (Low):** `F5_BURN_V5_STATUS` copy was replaced with a less operator-friendly version ("Burn uses materialized marks only…") that lost the action-oriented guidance.

MVP checklist: §6 operator / TUI devnet.

Claimed acceptance criteria:
- Unit tests for all three `f5_burn_hint_text` branches
- F5 still opens `f5_build_burn_form` only; no `submit_claim` / `ClaimTx` copy
- `F5_BURN_V5_STATUS` matches operator journey
- `cargo check -p pwm-tui && cargo test -p pwm-tui --lib`
- `python scripts/check_entity_name_segments.py` on touched pwm-tui src

Orchestrator pre-verified: `cargo check` OK; `36/36` lib tests.

---

## 2. Requirements fit

| Criterion | Verdict | Evidence |
|---|---|---|
| 3-state `f5_burn_hint_text` | PASS | `lib.rs:220-232`: three branches via `effective_marks.unwrap_or(marks)` |
| Unit tests for all three branches | PASS | `lib.rs:238-253`: `f5_hint_allow_burn`, `f5_hint_stake_first`, `f5_hint_wait_accrue` |
| F5 passes `effective_marks` | PASS | `tui_loop.rs:590-593`: `f5_burn_hint_text(owner.staked, owner.marks, owner.effective_marks)` |
| `F5_BURN_V5_STATUS` restored | PASS | `tui_loop.rs:42-43`: "V5 marks: stake PWM with S, wait for blocks, then burn materialized marks with F5." |
| No `submit_claim` / `ClaimTx` in src | PASS | `rg` on full `crates/pwm-tui/src/` returns empty for all three symbols |
| `cargo test -p pwm-tui --lib` green | PASS | 36/36 per orchestrator pre-verify |

All criteria satisfied.

---

## 3. Style and module shape

### Naming policy check

`python scripts/check_entity_name_segments.py` on all three diff paths:

```json
{
  "policy": { "prod_max": 4, "test_max": 5 },
  "files": [
    { "path": "crates/pwm-tui/src/lib.rs",      "violations": [] },
    { "path": "crates/pwm-tui/src/tui_loop.rs", "violations": [] },
    { "path": "crates/pwm-tui/src/burn_form.rs", "violations": [] }
  ]
}
```

**0 violations.** All new identifiers satisfy the segment policy.

Notable test names (test budget ≤5 segments):
- `f5_hint_allow_burn` — 4 segments ✓
- `f5_hint_stake_first` — 4 segments ✓
- `f5_hint_wait_accrue` — 4 segments ✓

### Module shape

- No new modules introduced; all changes are within existing files.
- `f5_burn_hint_text` (12 lines) and its `#[cfg(test)]` block are a focused, coherent addition to `lib.rs`.
- `f5_burn_status` helper in `tui_loop.rs` is a tight 6-line fn already present from the previous slice; no structural bloat.
- Module banners unchanged and still accurate.

### Wire JSON / u128

`Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).`

All values are local TUI state (`staked: u128`, `marks: u32`, `effective_marks: Option<u32>`) used exclusively for display routing. No network-facing serialization touched.

---

## 4. Safety

**No critical issues.**

**Low — orphaned `f5_burn_hint_needed` (2-state):** `lib.rs:211-216` still declares:

```
#[cfg(test)]
pub(crate) fn f5_burn_hint_needed(staked: u128, marks: u32) -> bool {
    staked == 0 && marks == 0
}
```

This is the prior 2-state gate, marked `#[cfg(test)]` and used only in `stake_form.rs` inline tests (`192-195`). Its logic is intentionally simpler (does not consult `effective_marks`), which is consistent with the stake-form context (the form checks whether staking is even needed before marks are considered). Not a correctness problem — but future maintainers could confuse the 2-state function with the authoritative 3-state `f5_burn_hint_text`. Recommend an `///` doc comment explaining the narrower purpose and pointing to `f5_burn_hint_text` as the primary gate.

**Info — `form.marks_available` uses raw `marks` not `effective_marks`:** `tui_loop.rs:611` sets `form.marks_available = fresh_owner.marks`. When the burn modal opens (guaranteed: `effective_marks.unwrap_or(marks) > 0`), there is a theoretical case where `marks = 10` but `effective_marks = 8` — the % hint in the modal would show percentage of 10 while the effective burn limit is 8. For V5 devnet this is an acceptable cosmetic discrepancy; calling convention is consistent with the rest of the burn flow.

**Low — `burn_form.rs:49` default status references "Claim":** The `BurnForm::new` default status is:
> "Marks materialize via Claim or Stake/Unstake. Burn uses materialized marks."

V5 removed the Claim path from the TUI. The text is not reachable via any F5 keyboard route (it is overwritten by `f5_burn_status()` at the call site), but if `BurnForm` is constructed in tests without the overwrite, the stale copy is visible. Not a blocker for this slice but warrants cleanup in a copy-polish pass.

---

## 5. Tests

### New tests (lib.rs `tests` module — 3 tests)

| Test | Branch covered | Assertion |
|---|---|---|
| `f5_hint_allow_burn` | `effective_marks.unwrap_or(marks) > 0` | `f5_burn_hint_text(10, 2, Some(1)) == None` |
| `f5_hint_stake_first` | `staked == 0` | message contains "Stake PWM" |
| `f5_hint_wait_accrue` | `staked > 0, effective == 0` | message contains "Wait for head advance" |

All three branches of the spec are exercised. Boundary: `effective_marks = Some(0)` with `staked = 10` correctly triggers the wait-accrual message.

**Minor gap:** `f5_hint_stake_first` passes `marks=0, effective_marks=None`. There is no test for `staked=0, marks=0, effective_marks=Some(0)` (should also trigger the stake-first path since `effective.unwrap_or(0) = 0`). The current test set is sufficient for functional verification; an extra boundary case would add confidence. Not a blocker.

**Regression scope:** The `f5_retired_claim_no_submit` test in `tui_loop.rs` (from the previous slice) continues to pin that `F5_BURN_V5_STATUS` has no "Claim submitted" copy — still green ✓.

### Coverage gaps (non-blocking, follow-up)

- No render-layer test for the info-modal path (`info_modal = Some(msg.into())`) triggered by the hint — this is acceptable for ratatui layer.
- `burn_form.rs:49` legacy "Claim" copy is not guarded by any test asserting its absence.

---

## 6. Verdict

**PASS_WITH_NITS**

NIT #1 (3-state hint) and NIT #2 (operator copy) from the prior review are fully resolved. Implementation is correct, clean, and well-tested for all three branches. No claim path in the F5 flow. Naming clean.

### Nits (non-blocking)

**Nit A — Low: add `///` doc to `f5_burn_hint_needed` clarifying its narrower scope vs `f5_burn_hint_text`.** Without a doc comment the two functions look like alternatives; the difference (no `effective_marks` parameter) is surprising.

**Nit B — Low: `burn_form.rs` default BurnForm status still mentions "Claim".** Schedule a copy-polish pass to remove the stale reference.

**Nit C — Info: `form.marks_available` uses raw `marks` rather than `effective_marks`.** Cosmetic inconsistency when `effective_marks < marks`; acceptable for devnet.

---

## 7. Participation / token estimate

```
agent: pwm-review
result: PASS_WITH_NITS
artifacts: docs/reviews/20260602-v5-tui-f5-hint-three-state-review.md
token_usage:
  source: estimate
  input: 22000
  output: 1800
  total: 23800
  confidence: medium
```

---

```powershell
# git-handoff
Set-Location 'P:\opt\docker\pwm-protocol'
git add 'docs/reviews/20260602-v5-tui-f5-hint-three-state-review.md'
git add 'tasks/20260602-v5-tui-f5-hint-three-state-coding.json'
git commit -m 'docs(v5-tui): F5 hint 3-state review PASS_WITH_NITS + task update'
```
