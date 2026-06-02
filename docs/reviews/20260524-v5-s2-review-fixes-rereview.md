# V5-2 Review Fixes Re-Review

## 1. Scope recap

Re-reviewed the integrated V5-2 fixup pass after coding PASS on commit `87af492`.

Primary re-check targets were the three earlier blocking findings:

- `marks_last_block` must be a chain-height cursor in active state and snapshot-facing paths;
- `GenCfg.season_coeff_ppm` must match RFC 0019 as `u64`;
- legacy wire `tx_type = "claim_mark"` must return a structured retired/unsupported error with direct evidence.

I also spot-checked that the previously approved slice4 `ClaimIPv4Batch` shape was not regressed.

## 2. Requirements fit

The prior blocking issues are closed.

- [crates/pwm-core/src/state.rs](../../crates/pwm-core/src/state.rs) now stores `marks_last_block` from `inclusion_height` and uses block-height deltas rather than wall-clock seconds in active maturity/touch paths.
- [crates/pwmd/src/snapshot/repair.rs](../../crates/pwmd/src/snapshot/repair.rs) now asserts height-based replay semantics instead of timestamp-based semantics.
- [crates/pwm-core/src/genesis.rs](../../crates/pwm-core/src/genesis.rs) now defines `season_coeff_ppm` as `u64`, matching [docs/rfc/19-float-inflation.md](../rfc/19-float-inflation.md).
- [crates/pwm-core/src/tx.rs](../../crates/pwm-core/src/tx.rs) now has explicit retired handling for legacy `claim_mark` input and a focused test proving that exact wire-name path.

The integrated V5-2 gate is now coherent enough to clear review.

## 3. Style and module shape

The fixup commit is narrow and well-targeted to the earlier findings. It repairs semantics at the owning code points instead of layering more compatibility glue on top.

Slice4 still looks stable after the rereview spot-check; no regression was introduced around `ClaimIPv4Batch` naming or deferred apply boundaries.

### Wire JSON / u128

Applicable.

This rereview did not introduce new `u128` wire fields, but it preserved the existing V5 JSON contract while fixing the earlier blockers. Public economic `u128` fields remain on decimal-string JSON surfaces, and the one field whose RFC type mattered here, `season_coeff_ppm`, is now correctly represented as `u64` instead of an RFC-divergent `u128`.

No new peer-wire `u128` issue stood out in the fixup diff.

## 4. Safety

No blocking safety findings in this rereview.

The main earlier risk was contract ambiguity across core state, snapshot migration, and legacy tx retirement. The current fixup resolves those issues to a level that is sufficient for the V5-2 review gate to pass.

## 5. Tests

Evidence reviewed:

- coding handoff in [tasks/done/20260524-v5-s2-review-fixes.json](../../tasks/done/20260524-v5-s2-review-fixes.json)
- prior FAIL reports in:
  - [docs/reviews/20260524-v5-s2-slice1-gencfg-review.md](20260524-v5-s2-slice1-gencfg-review.md)
  - [docs/reviews/20260524-v5-s2-slice2-account-review.md](20260524-v5-s2-slice2-account-review.md)
  - [docs/reviews/20260524-v5-s2-slice3-drop-claim-review.md](20260524-v5-s2-slice3-drop-claim-review.md)
  - [docs/reviews/20260524-v5-s2-slice5-snapshot-v3-review.md](20260524-v5-s2-slice5-snapshot-v3-review.md)
- fixup commit `87af492`
- validation already recorded by coding for:
  - `cargo test -p pwm-core --lib`
  - `cargo fmt --check`
  - `cargo test -p pwmd snapshot:: --lib`
  - `cargo check --workspace`

The direct proof for the previous `claim_mark` gap is now present in `tx::tests::claim_mark_wire_retires_with_structured_error`.

## 6. Verdict

Approve with nits.

No blocking request-changes items remain from the prior V5-2 review matrix.

Non-blocking note: the optional Rust field rename from `marks_per_hour` to `marks_per_coin_per_hour` is still reasonable if the team wants code terminology to mirror the frozen V5 text exactly, but it is not a gate issue.

## 7. Participation / token estimate

```text
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260524-v5-s2-review-fixes-rereview.md
token_usage: { "source": "estimate", "input": 17000, "output": 2100, "total": 19100, "confidence": "medium" }
```

## 8. Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-s2-review-fixes-rereview.md'
git add 'tasks/20260524-v5-s2-review-fixes-rereview.json'
git commit -m 'docs(v5-2): add rereview gate report for review fixes'
```