# Review: V5 pwm-tui build regression fix

**Date:** 2026-06-02
**Agent:** pwm-review
**Ticket:** `20260602-v5-pwm-tui-build-regression-coding`
**Verdict:** PASS_WITH_NITS

---

## 1. Scope recap

This slice restores a buildable, coherent state for `crates/pwm-tui` after integration conflicts
accumulated from three closed bridge slices:
- `20260530-v5-tui-form-amount-pct-hint-coding` — `form_amount_hint.rs` was produced but not wired
- `20260530-v5-marks-mechanics-tui-observability-coding` — `marks_display` re-exports drifted from `lib.rs`
- `20260530-v5-tui-v5-marks-copy-operator-path-coding` — `F5_BURN_V5_STATUS` copy and claim-path removal

MVP checklist: §6 operator / TUI devnet.

Claimed acceptance criteria (from ticket):
- `cargo check -p pwm-tui` + `cargo test -p pwm-tui --lib` green
- No `submit_claim` / `claim_mark` / `ClaimTx` in `crates/pwm-tui/src/`
- `form_amount_hint.rs` wired into `mod` tree + used in render modals
- `SendForm.balance_units` set in `f6_build_send_form`
- F5 opens burn modal, not claim

Orchestrator pre-verified: `cargo check` OK; `33/33` lib tests; `7/7` form_amount; naming 0 violations; rg on claim symbols — empty.

---

## 2. Requirements fit

| Criterion | Verdict | Evidence |
|---|---|---|
| Build regression fixed | PASS | orchestrator verify + review reads |
| No `submit_claim`/`claim_mark`/`ClaimTx` in src | PASS | `tui_loop.rs` F5 handler calls `f5_build_burn_form`; test `f5_retired_claim_no_submit` asserts no "Claim submitted" text |
| `form_amount_hint.rs` wired | PASS | `mod form_amount_hint` in `lib.rs:33`; re-exports at `lib.rs:34–37`; used in `render_send_modal` and `render_burn_modal` |
| `SendForm.balance_units` set | PASS | `lib.rs:174`: `form.balance_units = owner.balance_pwm;` |
| F5 % hint renders | PASS | `tui_loop.rs:1488`: `mark_pct_hint(value, form.marks_available)` |
| F6 % hint renders | PASS | `tui_loop.rs:1369`: `pwm_pct_hint(value, form.balance_units)` |
| F5 opens burn modal | PASS | `tui_loop.rs:585–624`: `KeyCode::F(5)` → preflight → hint gate → `f5_build_burn_form` |

One partial-coverage gap (see Nit 1 below).

---

## 3. Style and module shape

### Naming policy check

`python scripts/check_entity_name_segments.py` run on all four diff paths:

```json
{
  "policy": { "prod_max": 4, "test_max": 5 },
  "files": [
    { "path": "crates/pwm-tui/src/form_amount_hint.rs", "violations": [] },
    { "path": "crates/pwm-tui/src/lib.rs",              "violations": [] },
    { "path": "crates/pwm-tui/src/send_form.rs",        "violations": [] },
    { "path": "crates/pwm-tui/src/tui_loop.rs",         "violations": [] }
  ]
}
```

**0 violations.** All new identifiers satisfy ≤4-segment policy.

### Module shape

- `form_amount_hint.rs` has a proper `//!` banner ("Percentage hints for modal amount inputs.").
- `lib.rs` banner unchanged ("Network account table (public-friendly). Optional debug JSON via PWM_TUI_DEBUG=1.").
- `send_form.rs` banner present ("Send modal form state and validation.").
- `tui_loop.rs` banner present ("Ratatui event loop: keyboard routing, modals, send flow, redraw cadence.").
- `form_amount_hint` is a tight focused module (~60 lines + 57 lines test). No blob growth.
- New render-only logic in `tui_loop.rs` is inside existing `render_send_modal`/`render_burn_modal` functions — no new large top-level blocks.

### Wire JSON / u128

`Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).`

All `u128` values in this diff are local TUI form state (`SendForm.balance_units`, `amount_pct_hint` parameters) and render-side percentage calculations. No network-facing serialization touched.

---

## 4. Safety

No critical safety issues found.

**Low — float precision in percentage helper:** `format_pct_of_limit` casts both `u128` arguments to `f64` before division. For values above `2^53` (~9 × 10^15) this loses integer precision, but the result is used only for a display label (e.g., `"95.23% of balance"`). For marks (`u32` ≤ `MARKS_CAP` ≈ 4.29 × 10^9) and typical PWM balances, the display error is sub-unit. Not a user-visible bug; acceptable for a hint label. Not a blocker.

**No panics introduced:** `mark_pct_hint` and `pwm_pct_hint` use `?.` to propagate `None` on parse failure. `pad_input_field` counts characters (not bytes) — correct Unicode handling for the display-width constraint.

**`value_with_caret` byte-index slicing** (`&value[..i]`, `&value[i..]`) is pre-existing across `send_form.rs`. If `cursor` were a byte offset landing mid-multibyte char the function would panic. `TextInput` cursor management presumably maintains char boundaries; this is a pre-existing concern not introduced in this slice.

---

## 5. Tests

### form_amount_hint (7 tests — all new, all cover the new module)

| Test | Scenario |
|---|---|
| `pct_hint_empty_input` | `""` / `"  "` → `None` |
| `pct_hint_parse_fail` | `"abc"` / `"1.x"` → `None` |
| `pct_hint_zero_limit` | limit=0 → `None` |
| `pct_hint_exact_limit` | parsed==limit → `over_limit=false`, `"100.0%"` |
| `pct_hint_over_limit_pct` | parsed>limit → `over_limit=true` |
| `pct_hint_precision` | < 10% → 2 decimal places; ≥ 10% → 1 decimal place |
| `pad_input_field_fixed_width` | truncation and padding at exact width |

Good coverage of boundary conditions.

### tui_loop inline tests (new since this slice)

| Test | What it pins |
|---|---|
| `marks_cell_zero_stake` | `effective_marks=None`, `marks=0` → `"0"` |
| `marks_cell_sat_red` | `effective_marks=MARKS_CAP` → red cell style |
| `marks_cell_plain_style` | ordinary marks → no color |
| `panel_focus_active_bright` | active panel → LightYellow + BOLD |
| `panel_focus_inactive_neutral` | inactive panel → no style |
| `f5_retired_claim_no_submit` | `f5_burn_status(None)` == `F5_BURN_V5_STATUS`; no "Claim submitted" text |

**Missing test coverage (follow-up):**
- No test for `f5_burn_hint_text(staked > 0, marks == 0)` — i.e., the "wait for accrual" path. The previous slice had `f5_hint_wait_accrual` (per 20260530 review §3), but this test does not appear to exist in the current tree for the new 2-state logic.
- No unit test for `pwm_pct_hint` / `mark_pct_hint` wiring inside `render_send_modal` / `render_burn_modal` (render-layer tests are generally absent — acceptable for ratatui layer).

### Integration tests
`crates/pwm-tui/tests/send_form.rs` and `wallet_roaming.rs` fail to compile due to pre-existing `AcctRow` field drift. Confirmed out-of-scope for this slice per ticket `non_goals`. Flagged as follow-up.

---

## 6. Verdict

**PASS_WITH_NITS**

Build regression is fully resolved. Claim path is gone. `form_amount_hint` is correctly wired, tested, and rendered. `balance_units` is properly populated. F5/F6 % hints display correctly. Naming clean.

### Nits (non-blocking, follow-up recommended)

**Nit 1 — Medium: F5 hint missing "wait for accrual" branch**

`f5_burn_hint_text` (and `f5_burn_hint_needed`) only gates on `staked == 0 && marks == 0`. When `staked > 0` but `marks == 0`, the burn modal opens with `marks_available = 0` and no actionable guidance. The prior reviewed slice (`20260530-v5-tui-marks-copy-observability-post-review.md §3`) described a 3-state hint covering this case explicitly (`f5_hint_wait_accrual`: "staked > 0, marks == 0 → tells operator to wait"). That test/behavior was not carried forward.

Impact: operator with freshly staked PWM presses F5, sees "Marks available: 0", attempts to fill the marks field, gets a validation error — confusing but not dangerous. Medium severity (UX regression vs. documented spec; no safety impact).

Recommendation for `pwm-coding` follow-up: restore 3-state hint or add an in-modal banner ("No marks materialized yet; wait for blocks to accrue marks before burning.") when `marks_available == 0`.

**Nit 2 — Low: `F5_BURN_V5_STATUS` copy is less operator-friendly**

Changed from the prior-reviewed:
> "V5 marks: stake PWM with S, wait for blocks, then burn materialized marks with F5."

to:
> "Burn uses materialized marks only; fill burn fields and submit with F5 modal"

The new text is accurate but does not guide new operators. Acceptable for devnet. Suggest restoring the action-oriented wording in a polish pass.

**Nit 3 — Info: `format_marks_detail` not present**

Orchestrator confirmed this was optional/observability in this ticket. No action required here.

**Nit 4 — Info: Integration test compile failure pre-existing**

`AcctRow` field drift in `tests/send_form.rs` and `wallet_roaming.rs`. Separate follow-up ticket needed.

---

## 7. Participation / token estimate

```
agent: pwm-review
result: PASS_WITH_NITS
artifacts: docs/reviews/20260602-v5-pwm-tui-build-regression-review.md
token_usage:
  source: estimate
  input: 28000
  output: 2200
  total: 30200
  confidence: medium
```

---

```powershell
# git-handoff
Set-Location 'P:\opt\docker\pwm-protocol'
git add 'docs/reviews/20260602-v5-pwm-tui-build-regression-review.md'
git add 'tasks/20260602-v5-pwm-tui-build-regression-coding.json'
git commit -m 'docs(v5-tui): build regression review PASS_WITH_NITS + task update'
```
