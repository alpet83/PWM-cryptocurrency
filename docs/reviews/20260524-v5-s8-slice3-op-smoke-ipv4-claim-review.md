# V5-8 Slice 3 Review: ClaimIPv4Batch Operator Smoke Harness

## Scope recap

Reviewed artifacts for ticket `20260524-v5-s8-slice3-op-smoke-ipv4-claim-review`:

1. `scripts/devnet_v5_operator_smoke.ps1`
2. `docs/runbooks/devnet-v5-operator-smoke.md`
3. `tasks/done/20260524-v5-s8-slice3-op-smoke-ipv4-claim.json`
4. `docs/adr/0002-ipv4-claiming-design.md` (expectation anchor)

Ticket acceptance requires a real ClaimIPv4Batch happy-path smoke: signed tx submit via `POST /v1/tx`, account assertions, and `PASS_EVIDENCE: slice=ipv4_claim`.

## Requirements fit

What is implemented:

1. Slice-3 scaffolding exists in `devnet_v5_operator_smoke.ps1`.
2. `-Ipv4ClaimOnly` switch is present.
3. `Ensure-TestIPv4ClaimPhase` injects a deterministic phase into genesis JSON.

What is missing (blocking):

1. No signed `ClaimIPv4Batch` tx is built/submitted in slice3.
2. No `POST /v1/tx` call for ClaimIPv4Batch exists in the script.
3. No account-level assert for `ipv4_claimed_phase` + balance delta exists.
4. No `PASS_EVIDENCE: slice=ipv4_claim ...` line exists.
5. Runbook remains slice1/slice2-only and does not document slice3 acceptance flow.

Because the happy path is not executed, this slice cannot be treated as complete operator smoke for IPv4 claim.

## Style and module shape

Scope remains harness/docs only; no product Rust edits were introduced by this review slice.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract change in this slice).

## Safety

No direct consensus/runtime code change is introduced by this harness update. The main risk is false confidence: the current slice3 section can report pass-like status without exercising on-chain claim apply semantics.

## Tests

This review is static against acceptance criteria. Live smoke execution belongs to pwm-testing after coding closes the missing ClaimIPv4Batch steps.

## Verdict

Request changes.

Priority blockers:

1. Add signed `ClaimIPv4Batch` submit path (`POST /v1/tx`) in slice3.
2. Add account poll/assertions for `ipv4_claimed_phase` and allocation balance delta.
3. Emit `PASS_EVIDENCE: slice=ipv4_claim ...` on success.
4. Update runbook with slice3 expected PASS criteria and example evidence lines.

## Participation / token estimate

agent: pwm-review

result: FAIL

artifacts: docs/reviews/20260524-v5-s8-slice3-op-smoke-ipv4-claim-review.md

token_usage: { "source": "estimate", "input": 5600, "output": 1500, "total": 7100, "confidence": "medium" }

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-s8-slice3-op-smoke-ipv4-claim-review.md'
git add 'tasks/20260524-v5-s8-slice3-op-smoke-ipv4-claim-review.json'
git commit -m 'docs(v5): slice3 ipv4-claim smoke review request changes'
```