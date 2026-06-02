# V5-2 Slice 3 Review: ClaimTx Retirement

## 1. Scope recap

Reviewed V5-2 slice3 after coding PASS and testing PARTIAL for ClaimTx retirement across:

- [crates/pwm-core/src/tx.rs](../../crates/pwm-core/src/tx.rs)
- [crates/pwm-core/src/state.rs](../../crates/pwm-core/src/state.rs)
- adjacent CLI/TUI/pwmd consumers

Claimed scope was:

- remove `TxBody::Claim`, `ClaimMode`, `CLAIM_ALL`, and active claim apply/validate paths;
- retire legacy claim-only errors from the active model;
- keep `ClaimIPv4Batch` for later slice coverage;
- ensure legacy `claim_mark` input degrades via a structured retired/unsupported path instead of panic.

## 2. Requirements fit

Most of the retirement work is present:

- `TxBody::Claim` is removed from the active `pwm-core` enum;
- claim apply/validate logic is removed from active state paths;
- CLI/TUI flows are retired;
- snapshot compatibility now returns an explicit "retired in V5" error for legacy claim snapshot bodies.

The remaining issue is the exact legacy wire-name path.

## 3. Style and module shape

The slice is focused and mostly well-contained given the number of downstream consumers that had to be touched.

No style-level issue stands out beyond the missing compatibility proof for the old wire alias.

### Wire JSON / u128

Not applicable for the main finding in this slice.

This review is about transaction-kind retirement and legacy schema handling, not newly introduced `u128` wire fields. No new peer-wire `u128` concern stood out in the diff reviewed here.

## 4. Safety

Findings:

1. Medium: the slice removes active ClaimTx support, but I did not find direct evidence that legacy wire input with `tx_type = "claim_mark"` now maps to an explicit structured retired/unsupported error. Testing reported the same gap as `PARTIAL`, and the code search only found retirement messages for CLI/TUI and snapshot compatibility, not for the legacy wire-name itself. Since the ticket acceptance criteria called this out explicitly, the retirement path is still under-specified at the actual legacy wire boundary.

## 5. Tests

Evidence reviewed:

- coding handoff in [tasks/done/20260524-v5-s2-slice3-drop-claim.json](../../tasks/done/20260524-v5-s2-slice3-drop-claim.json)
- testing handoff in [tasks/done/20260524-v5-s2-slice3-drop-claim-testing.json](../../tasks/done/20260524-v5-s2-slice3-drop-claim-testing.json)
- commit `bf68bd8`
- targeted search for `claim_mark`, retired-in-V5 handling, and active `TxBody::Claim` references

Testing already covered the core compile/runtime gates, and its `PARTIAL` result is justified: I could confirm retirement of the model, but not the exact wire-name proof.

## 6. Verdict

Request changes.

Priority:

1. Add or point to direct evidence for legacy `claim_mark` deserialization returning a structured unsupported/retired error.

Acceptable fixes could be either:

- a focused deserialize test that proves the exact wire-name path, or
- an explicit compatibility mapping in code if the alias still needs to be recognized before rejection.

## 7. Participation / token estimate

```text
agent: pwm-review
result: FAIL
artifacts: docs/reviews/20260524-v5-s2-slice3-drop-claim-review.md
token_usage: { "source": "estimate", "input": 16500, "output": 1900, "total": 18400, "confidence": "medium" }
```

## 8. Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-s2-slice3-drop-claim-review.md'
git add 'tasks/20260524-v5-s2-slice3-drop-claim-review.json'
git commit -m 'docs(v5-2): add slice3 review gate report'
```