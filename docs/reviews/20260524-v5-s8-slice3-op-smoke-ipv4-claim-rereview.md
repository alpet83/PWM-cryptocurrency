# V5-8 Slice 3 Rereview: ClaimIPv4Batch Operator Smoke

## Scope recap

Rereview ticket: `20260524-v5-s8-slice3-op-smoke-ipv4-claim-rereview`

Reviewed fix artifacts:

1. `scripts/devnet_v5_operator_smoke.ps1`
2. `crates/pwm-cli/src/bin/claim_ipv4_batch.rs`
3. `docs/runbooks/devnet-v5-operator-smoke.md`
4. Prior review baseline: `docs/reviews/20260524-v5-s8-slice3-op-smoke-ipv4-claim-review.md`

## Requirements fit

Closed from prior blockers:

1. Script now has explicit `Submit-ClaimIPv4Batch` path.
2. Script attempts real `POST /v1/tx` submit with generated signed tx JSON.
3. Script includes claimant polling with checks for `ipv4_claimed_phase` and positive balance delta.
4. Script emits `PASS_EVIDENCE: slice=ipv4_claim ...` on success path.
5. Runbook now has a dedicated slice-3 section with acceptance criteria and PASS excerpt.

New blocking issue:

1. The new helper binary does not compile, so the submit path is currently non-runnable.
2. `cargo check -p pwm-cli` fails in `crates/pwm-cli/src/bin/claim_ipv4_batch.rs` with:
   - `error[E0433]: failed to resolve: could not find signer in the crate root`
3. Because the helper fails to build, the script cannot reliably execute ClaimIPv4Batch submit in real runs.

## Style and module shape

Changes remain in harness/runbook/helper boundaries, but compile health is a gate for this slice.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract change in this rereview).

## Safety

No consensus runtime behavior change was reviewed here. The primary operational risk is false readiness: the smoke flow appears complete in script text but is blocked at build time.

## Tests

Verification command run:

1. `cargo check -p pwm-cli`

Observed result: FAIL

Key compiler error:

1. `crates/pwm-cli/src/bin/claim_ipv4_batch.rs:84:29`
2. `could not find signer in the crate root`

## Verdict

Request changes.

Priority blockers:

1. Fix helper wiring so `claim-ipv4-batch` compiles under `cargo check -p pwm-cli`.
2. Re-run `cargo check -p pwm-cli` clean.
3. Keep current slice-3 script/runbook logic; revalidate after helper compile fix.

## Participation / token estimate

agent: pwm-review

result: FAIL

artifacts: docs/reviews/20260524-v5-s8-slice3-op-smoke-ipv4-claim-rereview.md

token_usage: { "source": "estimate", "input": 6200, "output": 1700, "total": 7900, "confidence": "medium" }

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-s8-slice3-op-smoke-ipv4-claim-rereview.md'
git add 'tasks/20260524-v5-s8-slice3-op-smoke-ipv4-claim-rereview.json'
git commit -m 'docs(v5): slice3 ipv4 claim rereview request changes (helper compile gate)'
```