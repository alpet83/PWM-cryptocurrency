# V5-4 Slice 2 Review: Height-gated Deferred Evaluator and Activate/Deactivate Rejects

## 1. Scope recap

Reviewed commit 260dccb for V5-4 slice2 against the sprint plan and ADR 0005.

Claimed scope validated in this review:

- evaluator takes chain height and stays read-only;
- deferred policy activation is height-gated in evaluator checks;
- ActivatePolicy on deferred policy is rejected before and at or after activation height per ADR;
- DeactivatePolicy removes pending deferred entry before activation height;
- no slice3 responsibilities were pulled in.

Primary code scope:

- [crates/pwm-core/src/state.rs](../../crates/pwm-core/src/state.rs)

Anchors:

- [docs/plans/mvp_v5.md](../plans/mvp_v5.md)
- [docs/adr/0005-policy-deferred-activation.md](../adr/0005-policy-deferred-activation.md)

## 2. Requirements fit

Slice2 acceptance criteria are met.

- apply path now calls evaluator with inclusion height: evaluate_policy(tx, inclusion_height).
- evaluate_policy signature now includes chain_tip_height and applies this value in all policy checks where active state matters.
- policy_is_active_at includes active bit and deferred entry check with chain_tip_height greater or equal activate_at_height.
- ActivatePolicy on a deferred policy returns:
  - PolicyNotActive before activation height;
  - PolicyDenied at or after activation height (already auto-active by rule).
- DeactivatePolicy removes pending deferred entry (before activation height) for reversible policies.
- Scope discipline is preserved: no RFC 6/7 edits and no snapshot/genesis deferred roundtrip work in this commit.

## 3. Style and module shape

Naming policy check on touched production file is clean.

Evidence:

- python scripts/check_entity_name_segments.py crates/pwm-core/src/state.rs
- Result: zero violations.

Module shape is coherent and minimal: evaluator behavior and policy transitions are implemented in state layer only, without cross-crate drift.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire or RFC wire contract in this slice).

The patch changes policy evaluator and state transition logic only; no network-facing JSON payload schema is introduced or modified.

## 4. Safety

No blocking safety findings.

- Evaluator remains pure and does not mutate state.
- Rejection paths for deferred activation are explicit and deterministic.
- Pending deferred removal in DeactivatePolicy is scoped to pre-activation state and keeps irreversible guard behavior.

## 5. Tests

Mandatory verification commands passed:

- python scripts/check_entity_name_segments.py crates/pwm-core/src/state.rs
- cargo test -p pwm-core policy_ --lib

Targeted evidence from test output includes:

- policy_eval_deferred_h
- policy_act_deferred_before_h
- policy_act_deferred_at_h
- policy_deact_deferred_before_h

Observed result: 28 passed, 0 failed.

## 6. Verdict

Approve.

No blocking gaps found for slice2 scope.

## 7. Participation / token estimate

```text
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260524-v5-s4-slice2-evaluator-review.md
token_usage: { "source": "estimate", "input": 15000, "output": 1900, "total": 16900, "confidence": "medium" }
```

## 8. Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-s4-slice2-evaluator-review.md'
git add 'tasks/20260524-v5-s4-slice2-evaluator-review.json'
git commit -m 'docs(v5-4): add slice2 evaluator review gate'
```