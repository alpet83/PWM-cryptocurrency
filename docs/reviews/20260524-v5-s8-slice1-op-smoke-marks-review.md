# V5-8 Slice 1 Re-Review: Devnet V5 Operator Smoke Marks Path

## Scope Recap

This re-review covers [scripts/devnet_v5_operator_smoke.ps1](../../scripts/devnet_v5_operator_smoke.ps1) and [docs/runbooks/devnet-v5-operator-smoke.md](../../docs/runbooks/devnet-v5-operator-smoke.md) against [docs/plans/mvp_v5.md](../../docs/plans/mvp_v5.md#sprint-v5-8-integrated-devnet-gate-и-closeout). The stated goal is a slice 1 operator smoke harness for tx-init, stake, head polling, account marks observation, report generation, and exit-code handling, with no pwm-core or pwmd economics changes.

## Requirements Fit

The parameter surface is present: CleanState, SkipGenesis, RpcUrl, ReportPath, SmokeSeconds, and MarksOnly are all defined in the harness.

The main smoke loop now does exercise the account mark path: Get-AccountMarks is used to capture the baseline and to compare marks / marks_last_block during polling. I also re-ran the PowerShell parser, and it now passes cleanly.

The remaining acceptance gap is in the runbook: it covers prerequisites, quick start, and CARGO_TARGET_DIR, but it still does not include the cq_process_ctl note that the ticket explicitly asks for in the testing guidance.

## Style and Module Shape

No production Rust naming policy issues were introduced in this slice, and the reviewed PowerShell helper names are short enough for the local style budget. The script structure now looks coherent enough for review; no brace mismatch remains in the current parseable version.

### Wire JSON / u128

Not applicable (no peer wire / RFC wire contract in this slice).

## Safety

Beyond the now-fixed syntax issue, the slice does not introduce crypto changes, unchecked trust-boundary expansion, or obvious resource-limit regressions. The review did not find any pwm-core or pwmd economics edits in the target slice.

## Tests

I ran the PowerShell parser against [scripts/devnet_v5_operator_smoke.ps1](../../scripts/devnet_v5_operator_smoke.ps1), and it passed cleanly. No live smoke execution was attempted because this is still a review gate, not the testing slice.

## Verdict

Request changes.

Priority items:

1. Add the cq_process_ctl testing note to the runbook so the operator/testing handoff is complete.
2. Consider adding one explicit example of the expected PASS evidence for the account marks comparison to make the report output easier to validate by eye.

## Participation / token estimate

agent: pwm-review

result: PARTIAL

artifacts: docs/reviews/20260524-v5-s8-slice1-op-smoke-marks-review.md

token_usage: { "source": "estimate", "input": 5400, "output": 1400, "total": 6800, "confidence": "medium" }

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-s8-slice1-op-smoke-marks-review.md'
git commit -m 'docs(slice-o): slice1 operator smoke review'
```