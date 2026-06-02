# V5-4 Slice 1 Review: ActivationMode Deferred and SetPolicy Storage

## 1. Scope recap

Reviewed commit 4471085 for V5-4 slice1 against the sprint target and ADR 0005:

- ActivationMode includes Deferred with explicit activate_at_height.
- SetPolicy and Init apply path store deferred entries.
- Immediate active bit is set when activate_at_height is less than or equal to inclusion height.
- Slice boundary is preserved: no evaluate_policy height parameter work in this slice.

Primary scope files:

- [crates/pwm-core/src/tx.rs](../../crates/pwm-core/src/tx.rs)
- [crates/pwm-core/src/state.rs](../../crates/pwm-core/src/state.rs)
- [crates/pwmd/src/snapshot/types.rs](../../crates/pwmd/src/snapshot/types.rs)

Plan and normative anchors:

- [docs/plans/mvp_v5.md](../plans/mvp_v5.md)
- [docs/adr/0005-policy-deferred-activation.md](../adr/0005-policy-deferred-activation.md)

## 2. Requirements fit

The implementation satisfies slice1 acceptance criteria.

- ActivationMode now supports Deferred with serde shape that requires activate_at_height.
- Signing payload includes Deferred tag 2 and height bytes for both Init extension policies and PolicyAction SetPolicy.
- State apply path uses a shared helper to enforce mode transitions and deferred entry storage.
- ADR rule for immediate activation on inclusion is implemented: if activate_at_height is less than or equal to inclusion height, active bit is set immediately.
- evaluate_policy signature remains unchanged in this slice, which matches the declared slice2 boundary.
- No premature ActivatePolicy deferred reject behavior was added here, also matching slice2 ownership.

## 3. Style and module shape

No naming policy violations detected on touched files.

Checker evidence:

- python scripts/check_entity_name_segments.py crates/pwm-core/src/tx.rs crates/pwm-core/src/state.rs crates/pwmd/src/snapshot/types.rs
- Result: zero violations in all three files.

Module shape is coherent: tx model and signing logic are changed in tx.rs, state transition logic in state.rs, and snapshot string mapping touched minimally.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire or RFC wire contract change in this slice).

The snapshot touch in pwmd is local snapshot conversion support and not a peer-to-peer transport contract update.

## 4. Safety

No blocking safety findings in scope.

- State transitions clear conflicting active and dormant bits before applying a new mode.
- Deferred entries are deduplicated per policy through retain before push, avoiding unbounded duplicates for the same policy kind.
- All added numeric handling stays in u64 for activation height and does not introduce unchecked casts.

## 5. Tests

Reviewed and confirmed targeted evidence:

- cargo test -p pwm-core policy_ --lib passed with 24 passed, 0 failed.
- New coverage includes deferred JSON roundtrip, deferred JSON missing-height rejection, deferred signing by height, SetPolicy deferred storage, and Init deferred immediate activation at matching height.

## 6. Verdict

Approve with nits.

Non-blocking nit for slice3 tracking:

- In snapshot conversion, activation_to_str now emits deferred while activation_from_str still accepts only dormant and immediately. Ticket explicitly allows compile-only snapshot changes in slice1, but slice3 should close this asymmetry with full deferred snapshot roundtrip semantics.

## 7. Participation / token estimate

```text
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260524-v5-s4-slice1-activation-mode-review.md
token_usage: { "source": "estimate", "input": 16000, "output": 2100, "total": 18100, "confidence": "medium" }
```

## 8. Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-s4-slice1-activation-mode-review.md'
git add 'tasks/20260524-v5-s4-slice1-activation-mode-review.json'
git commit -m 'docs(v5-4): add slice1 deferred activation review gate'
```