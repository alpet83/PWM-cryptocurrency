# V5-1 Spec/RFC/ADR Freeze Review

## 1. Scope recap

Reviewed the doc-only V5-1 freeze for lazy marks, retired `ClaimTx`, deferred policy activation, and V5 spec-only ADR boundaries.

Claimed scope from [docs/plans/mvp_v5.md](../plans/mvp_v5.md) and the ticket is:

- RFC 0012 v2 lazy marks model;
- RFC 0011 / 0013 / 0014 addenda retiring `ClaimTx` active scope;
- RFC 0019 float inflation;
- ADR 0005 accepted deferred activation contract;
- ADR 0006 address flags spec-only boundary;
- ADR 0007 domain lease governance.

Reviewed the current diff for:

- [docs/adr/0005-policy-deferred-activation.md](../adr/0005-policy-deferred-activation.md)
- [docs/adr/README.md](../adr/README.md)
- [docs/rfc/11-burn-purpose-and-claim-tx.md](../rfc/11-burn-purpose-and-claim-tx.md)
- [docs/rfc/12-claim-maturity-and-state-model.md](../rfc/12-claim-maturity-and-state-model.md)
- [docs/rfc/13-claim-policy-matrix.md](../rfc/13-claim-policy-matrix.md)
- [docs/rfc/14-claim-burn-api-error-contract.md](../rfc/14-claim-burn-api-error-contract.md)
- [docs/rfc/6-policy-engine.md](../rfc/6-policy-engine.md)
- [docs/rfc/7-tx-and-state-model.md](../rfc/7-tx-and-state-model.md)

Also spot-checked current repo state for:

- [docs/rfc/19-float-inflation.md](../rfc/19-float-inflation.md)
- [docs/adr/0006-address-flags-and-nondisableable-profiles.md](../adr/0006-address-flags-and-nondisableable-profiles.md)
- [docs/adr/0007-domain-lease-parameter-governance.md](../adr/0007-domain-lease-parameter-governance.md)
- [docs/plans/mvp_v5.md](../plans/mvp_v5.md)

## 2. Requirements fit

Most V5-1 acceptance items are present:

- RFC 0012 v2 now defines `marks_last_block`, `blocks_per_hour`, saturation logic, touch semantics, and retirement of anchor/free-day claim state.
- RFC 0011 / 0013 / 0014 now retire `ClaimTx` from active V5 scope.
- RFC 0019 contains the required float inflation formula and zero-coefficient fallback.
- ADR 0005 is now Accepted and no longer carries blocking draft language.
- ADR 0006 and ADR 0007 match the requested V5 spec-only boundaries.

However the freeze is not internally consistent enough to clear a coding gate yet.

## 3. Style and module shape

The edited docs mostly follow the repo's current RFC/ADR style: English normative prose, explicit status blocks, bounded scope, and small normative lists.

The main style problem is structural consistency, not wording. [docs/rfc/7-tx-and-state-model.md](../rfc/7-tx-and-state-model.md) still carries the old account and burn-resource model while [docs/rfc/12-claim-maturity-and-state-model.md](../rfc/12-claim-maturity-and-state-model.md) now defines the V5 replacement. That leaves two active RFCs describing different state contracts.

### Wire JSON / u128

Applicable as a doc-gap check, even though this slice does not change peer-framed transport structs directly.

RFC 0012 v2 introduces `staked_pwm_raw: u128` in the normative account state model and RFC 0019 uses `u128` config values, but the slice does not explicitly say whether these fields are internal-only or how they must be represented on JSON/API or snapshot-facing surfaces. RFC 0007 still has one explicit decimal-string rule for `fee: u128`, but no matching normative rule for the newly emphasized `staked_pwm_raw` path. This leaves a V5 field-encoding ambiguity that the ticket explicitly asked to avoid before code starts.

## 4. Safety

No production code was changed, so the main safety risk here is spec ambiguity rather than runtime defects.

Findings:

1. High: [docs/rfc/7-tx-and-state-model.md](../rfc/7-tx-and-state-model.md) still defines `Account { balance_pwm, staked, marks, marks_quota, initialized, index, flags }` and says `MarkBurnTx` burns `marks_quota`, while [docs/rfc/12-claim-maturity-and-state-model.md](../rfc/12-claim-maturity-and-state-model.md) now defines the V5 marks state as `stored_marks: u32`, `staked_pwm_raw: u128`, `marks_last_block: u64` with lazy touch semantics. This is a normative contradiction inside the same freeze. Coding sprint V5-2 cannot safely implement schema/state changes with both contracts active.

2. Medium: [docs/rfc/12-claim-maturity-and-state-model.md](../rfc/12-claim-maturity-and-state-model.md) introduces `staked_pwm_raw: u128` and migration language, but the slice does not add a matching encoding rule or an explicit statement that the field is not part of any public JSON/wire contract. The ticket asked for no unresolved V5 wire ambiguity; this one remains open.

## 5. Tests

This was a doc-only review. Evidence used:

- `git diff --name-only` on the claimed slice;
- `git diff` on the changed RFC/ADR files;
- targeted reads of RFC 0012, RFC 0007, RFC 0019, ADR 0005, ADR 0006, ADR 0007, and the V5 plan;
- repo grep for `u128` and related encoding language in RFC docs.

Missing before approving the freeze:

- one consistency pass that updates RFC 0007 core account/burn sections to the V5 lazy-marks model or explicitly scopes them to historical baseline only;
- one explicit V5 statement for `u128` encoding or non-wire status of `staked_pwm_raw` and any related snapshot/API field.

## 6. Verdict

Request changes.

Priority:

1. Reconcile [docs/rfc/7-tx-and-state-model.md](../rfc/7-tx-and-state-model.md) with RFC 0012 v2 so the active V5 state contract is singular.
2. Close the `u128` representation ambiguity for `staked_pwm_raw` before V5-2 serialization work starts.

## 7. Participation / token estimate

```text
agent: pwm-review
result: FAIL
artifacts: docs/reviews/20260523-v5-sprint1-spec-adr-freeze-review.md
token_usage: { "source": "estimate", "input": 18000, "output": 2200, "total": 20200, "confidence": "medium" }
```

## 8. Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260523-v5-sprint1-spec-adr-freeze-review.md'
git add 'tasks/20260523-v5-sprint1-spec-adr-freeze.json'
git commit -m 'docs(v5-1): add review gate report and traceability'
```