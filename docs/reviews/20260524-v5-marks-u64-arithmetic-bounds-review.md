# Review: lazy marks u64 arithmetic bounds (RFC 0012 v2)

Ticket: 20260524-v5-marks-u64-arithmetic-profile-rereview

## Scope recap

Reviewed:

1. docs/rfc/12-claim-maturity-and-state-model.md (satur_hours rationale + u64 profile)
2. docs/plans/mvp_v5.md (overflow statement anchor)
3. crates/pwm-core/src/marks.rs (current lazy-marks arithmetic shape)

Goal of this rereview slice: verify that prior numeric-bound findings are closed and that RFC terminology is now internally consistent.

## Requirements fit

What matches:

1. RFC text correctly states the satur_hours ceiling requirement and its role in preventing floor-stall.
2. RFC includes an explicit informative u64 profile and keeps staked_pwm_raw storage semantics as u128 on Account.
3. Bound shape generated <= remaining + per_hour <= u32::MAX + per_hour is consistent with the stated formula.

What is now fixed:

1. RFC switched to a single normative cap symbol MARKS_CAP and uses it consistently in formulas and invariants.
2. The 21B conservative bound in RFC is corrected to approximately 8.78x10^8, which matches recomputation for S = 21x10^9 whole PWM.
3. Narrative about u64 profile and satur_hours remains coherent after the numeric correction.

Verification command used:

python - <<'PY'
S=21_000_000_000
print(((2**64)-1)//S)
PY

Observed output: 878416384

## Style and module shape

No production Rust edits were made in this review slice.

Current code note for traceability:

1. crates/pwm-core/src/marks.rs still uses u128 intermediates in compute_lazy_marks (ceil_div_u128, u128 per_hour/generated path).
2. This is acceptable for current behavior and is explicitly out of scope for this rereview (spec/bounds gate only).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## Safety

No remaining spec-safety blockers were found in the reviewed RFC segment. The previous arithmetic mismatch is resolved.

## Tests and verification evidence

Commands run:

1. python integer checks for R_max scenarios:
	- S = 21,000,000,000 => R_max = 878,416,384
	- S = 100,000,000 => R_max = 184,467,440,737
	- S = 1,000,000 => R_max = 18,446,744,073,709
2. Source inspection of marks.rs confirms current u128 intermediate implementation.

## Verdict

PASS.

No blocking findings remain for the spec/bounds slice.

## Participation / token estimate

agent: pwm-review

result: PASS

artifacts: docs/reviews/20260524-v5-marks-u64-arithmetic-bounds-review.md

token_usage: { "source": "estimate", "input": 6600, "output": 1700, "total": 8300, "confidence": "medium" }

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-marks-u64-arithmetic-bounds-review.md'
git add 'tasks/20260524-v5-marks-u64-arithmetic-profile-rereview.json'
git commit -m 'docs(v5): rereview lazy marks u64 bounds PASS'
```
