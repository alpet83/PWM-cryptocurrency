# V5-8 Slice 2 Review: Deferred Operator Smoke Harness

## Scope recap

Reviewed slice-2 harness artifacts:

1. `scripts/devnet_v5_operator_smoke.ps1`
2. `docs/runbooks/devnet-v5-operator-smoke.md`
3. `docs/adr/0005-policy-deferred-activation.md`

Slice goal: operator-level deferred activation smoke flow only (no product Rust edits).

## Requirements fit

Acceptance checks pass:

1. Deferred flow is implemented in the script: deferred set at `head + lead`, explicit pre-height reject check for `tx-policy-activate`, wait for `head >= activate_at`, post-height active-policy confirmation, and post-height `tx-policy-activate` reject as already active.
2. Combined-run semantics are explicit: `-MarksOnly` and `-DeferredOnly` are mutually exclusive; deferred slice is gated by marks result in full mode and can run independently in `-DeferredOnly` mode.
3. PASS evidence line is emitted in grep-friendly form: `PASS_EVIDENCE: slice=deferred ...`.
4. Exit-code contract is defined in script/runbook (`0` pass, `3` harness exception, `4` marks timeout path, `5` deferred failure path).
5. Runbook is updated for slice 2 with expected behavior and a concrete PASS excerpt.

## Style and module shape

The slice stays in script/runbook boundaries and does not introduce product Rust changes. Naming and structure are readable for operator handoff.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## Safety

No direct consensus/runtime code changes were found in this slice. The harness validates both pre- and post-height behavior, reducing operator false-positive risk for deferred activation checks.

## Tests

This review validated static harness logic and runbook consistency against ADR 0005 semantics. Live execution belongs to the downstream pwm-testing slice.

## Verdict

PASS.

No blockers found for moving this slice to testing.

## Participation / token estimate

agent: pwm-review

result: PASS

artifacts: docs/reviews/20260524-v5-s8-slice2-op-smoke-deferred-review.md

token_usage: { "source": "estimate", "input": 5200, "output": 1300, "total": 6500, "confidence": "medium" }

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-s8-slice2-op-smoke-deferred-review.md'
git add 'tasks/20260524-v5-s8-slice2-op-smoke-deferred-review.json'
git commit -m 'docs(v5): slice2 deferred smoke review PASS'
```