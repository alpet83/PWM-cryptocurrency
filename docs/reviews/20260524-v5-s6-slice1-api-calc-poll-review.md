# Review: V5-6 Slice1 (API marks_last_block + effective marks poll)

## 1. Scope recap

- Ticket: `20260524-v5-s6-slice1-api-calc-poll-review`
- Coding commit under review: `2d5c6cb`
- Claimed MVP item: `docs/plans/mvp_v5.md#sprint-v5-6-tui-marks-saturation`
- Reviewed scope files:
  - `crates/pwmd/src/api/types.rs`
  - `crates/pwmd/src/api/common.rs`
  - `crates/pwm-tui/src/marks_display.rs`
  - `crates/pwm-tui/src/models.rs`
  - `crates/pwm-tui/src/account_view.rs`
  - `crates/pwm-tui/src/lib.rs`
  - `crates/pwm-tui/src/tui_loop.rs` (scope-boundary spot check)

## 2. Requirements fit

- `AcctOut` got additive `marks_last_block: u64` and runtime mapping from `Account.marks_last_block` in `acct_out_for_runtime`.
- `marks_display` uses `pwm_core::compute_lazy_marks` (no formula reimplementation found).
- `poll_snapshot` enriches each `AcctRow` with `effective_marks` and saturation percent when `head_height` is known.
- Zero-stake path is safe: effective marks remain stored marks.
- `GenCfg` inputs in marks display are sourced from `pwm_core::genesis` defaults (`DEF_*`).
- Slice boundary check: in `2d5c6cb`, `tui_loop.rs` changes are test-fixture field additions only; no table rendering in slice1 commit.

Result: scope requirements are met for slice1.

## 3. Style and module shape

- Naming gate (`python scripts/check_entity_name_segments.py` on touched files): no violations.
- New module `marks_display.rs` has proper `//!` banner and focused responsibility.
- Production/test naming remains within current segment limits.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

- No new panic-prone hot-path `unwrap` introduced in reviewed production deltas.
- `marks_sat_pct` uses bounded conversion (`u8::try_from(...).unwrap_or(100)`), which degrades safely.
- Poll enrichment is guarded by `head_height` presence and does not affect submit paths.

## 5. Tests

- Verified `marks_display` unit tests:
  - `marks_display_zero_stake`
  - `marks_display_sat_cap`
  - `marks_display_lazy_delta`
- `cargo check -p pwm-tui -p pwmd` passes.
- `cargo test -p pwm-tui marks_display --lib` passes (`3 passed`).

## 6. Verdict

PASS.

Priority notes:

- `pwmd` version bump `0.1.55 -> 0.1.56` is present and consistent with additive account API surface.
- No slice2 UI rendering drift found inside commit `2d5c6cb`.

## 7. Participation / token estimate

```json
{
  "agent": "pwm-review",
  "result": "PASS",
  "artifacts": [
    "docs/reviews/20260524-v5-s6-slice1-api-calc-poll-review.md"
  ],
  "token_usage": {
    "source": "estimate",
    "input": null,
    "output": null,
    "total": 9500,
    "confidence": "medium"
  }
}
```