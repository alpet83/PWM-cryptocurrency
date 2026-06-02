# V5-1 Spec/RFC/ADR Freeze Re-Review

## 1. Scope recap

Re-reviewed the V5-1 doc freeze after the follow-up fixes from [tasks/done/20260523-v5-sprint1-review-fixes.json](../../tasks/done/20260523-v5-sprint1-review-fixes.json).

Primary re-check targets were:

- singular active V5 Account/state contract across RFC 0007 and RFC 0012 v2;
- explicit normative `u128` encoding guidance for `staked_pwm_raw` and related public V5 economic fields;
- continued validity of the original V5-1 freeze items for RFC 0011, RFC 0013, RFC 0014, RFC 0019, ADR 0005, ADR 0006, ADR 0007, and [docs/plans/mvp_v5.md](../plans/mvp_v5.md).

## 2. Requirements fit

The previously blocking issues are closed.

- [docs/rfc/7-tx-and-state-model.md](../rfc/7-tx-and-state-model.md) now names the active V5 account fields as `balance_pwm`, `staked_pwm_raw`, `stored_marks`, `marks_last_block`, and explicitly demotes `staked`, `marks`, and `marks_quota` to historical terminology only.
- RFC 0007 burn semantics now align with RFC 0012 v2: `MarkBurnTx` touches the sender first and burns from effective lazy marks rather than from the retired `marks_quota` model.
- [docs/rfc/12-claim-maturity-and-state-model.md](../rfc/12-claim-maturity-and-state-model.md), [docs/rfc/7-tx-and-state-model.md](../rfc/7-tx-and-state-model.md), and [docs/rfc/19-float-inflation.md](../rfc/19-float-inflation.md) now make the public JSON decimal-string rule explicit for the relevant V5 `u128` fields.
- The original V5-1 freeze criteria remain satisfied for the ClaimTx retirement addenda, float inflation formula/fallback, deferred activation contract, address-flag spec-only boundary, and domain lease governance boundary.

## 3. Style and module shape

The repaired slice is internally cleaner than the first freeze pass. RFC 0007 now acts as the umbrella transaction/state RFC while explicitly delegating the active marks submodel to RFC 0012 v2, which removes the earlier contract duplication.

No naming-policy or structure regression stood out in the reviewed docs.

### Wire JSON / u128

Applicable as a documentation-contract check.

The rerun slice closes the prior ambiguity. Public JSON/API/operator surfaces now normatively require decimal-string encoding for `staked_pwm_raw` and related V5 economic `u128` fields, while binary state hashing and signing preimages remain unchanged.

No derive-only peer-wire `u128` issue is introduced in this doc-only slice.

## 4. Safety

No blocking safety findings in this rereview.

The main earlier risk was spec ambiguity before V5-2 schema work. The current text resolves that ambiguity to a level that is sufficient for coding to proceed.

## 5. Tests

This was a doc-only review.

Evidence used:

- prior FAIL report in [docs/reviews/20260523-v5-sprint1-spec-adr-freeze-review.md](20260523-v5-sprint1-spec-adr-freeze-review.md);
- fixes handoff in [tasks/done/20260523-v5-sprint1-review-fixes.json](../../tasks/done/20260523-v5-sprint1-review-fixes.json);
- targeted reads of RFC 0007, RFC 0012, RFC 0019, and the V5 plan;
- `git diff` on the review-fixes doc slice;
- `git diff --check` on the rereviewed files.

No missing blocking validation was identified for this review role.

## 6. Verdict

Approve with nits.

No blocking request-changes items remain for starting V5-2.

Non-blocking note: implementation tickets should preserve the same decimal-string language in code-adjacent docs or serde wrappers so the written contract does not drift during V5-2 snapshot/API work.

## 7. Participation / token estimate

```text
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260523-v5-sprint1-spec-adr-freeze-rereview.md
token_usage: { "source": "estimate", "input": 14000, "output": 1800, "total": 15800, "confidence": "medium" }
```

## 8. Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260523-v5-sprint1-spec-adr-freeze-rereview.md'
git add 'tasks/20260523-v5-sprint1-review-rerun.json'
git commit -m 'docs(v5-1): add rereview gate report and traceability'
```