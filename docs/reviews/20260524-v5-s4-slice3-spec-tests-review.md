# V5-4 Slice 3 Review: RFC Alignment, Snapshot Deferred Wire, and Deferred Scenario Tests

## 1. Scope recap

Reviewed commit d3bd26b for the final coding slice of V5-4 (pre-testing gate).

Claimed slice3 scope in this review:

- RFC 6 and RFC 7 normative alignment for Deferred activation semantics;
- snapshot activation wire parse/format symmetry for deferred heights;
- deferred scenario tests in state and snapshot test suites;
- no CLI/TUI changes and no behavioral drift outside docs/tests.

Scope files:

- [docs/rfc/6-policy-engine.md](../rfc/6-policy-engine.md)
- [docs/rfc/7-tx-and-state-model.md](../rfc/7-tx-and-state-model.md)
- [crates/pwmd/src/snapshot/types.rs](../../crates/pwmd/src/snapshot/types.rs)
- [crates/pwm-core/src/state.rs](../../crates/pwm-core/src/state.rs)

Normative anchor:

- [docs/adr/0005-policy-deferred-activation.md](../adr/0005-policy-deferred-activation.md)

## 2. Requirements fit

Slice3 acceptance criteria are satisfied.

- RFC updates explicitly document Deferred in Init/Policy rules, immediate activation when activate_at_height is less than or equal to inclusion_height, and DeactivatePolicy behavior before activation height.
- Snapshot wire mapping is now symmetric for Deferred height form:
  - encode: deferred:<activate_at_height>
  - decode: deferred:<u64> (plus legacy-compatible deferred/<u64> acceptance)
  - bare deferred without height is rejected with explicit error.
- Added snapshot tests cover SetPolicy deferred roundtrip, Init policy deferred roundtrip, and rejection of deferred string without height.
- Added/renamed deferred scenario tests in state.rs satisfy MVP naming intent via explicit scenario comments tied to linter-compliant identifiers.
- No CLI/TUI files touched.

## 3. Style and module shape

Naming policy is compliant for touched Rust files.

Evidence:

- python scripts/check_entity_name_segments.py crates/pwmd/src/snapshot/types.rs crates/pwm-core/src/state.rs
- Result: zero violations.

Change shape remains focused: docs and snapshot/state tests only, without new cross-module coupling.

### Wire JSON / u128

Scope applies: this slice touches persistence/wire string representation for Deferred activation in snapshot transport shape.

Observed wire form is explicit and deterministic: deferred:<u64>.

`u128` risk check: no new peer-facing `u128` JSON fields were introduced by this slice; changes concern activation mode strings and `u64` height parsing. No `u128 is not supported` wire hazard was added.

## 4. Safety

No blocking safety findings.

- Decoder now rejects ambiguous deferred mode without height instead of silently accepting incomplete data.
- Height parsing errors are explicit and path-qualified.
- Behavioral rules remain aligned with ADR and prior slice2 evaluator logic.

## 5. Tests

Mandatory ticket checks passed.

- cargo test -p pwm-core policy_ --lib
  - 28 passed, 0 failed.
- cargo test -p pwmd snapshot::types::tests::
  - targeted snapshot types suite passed (deferred roundtrip tests included).

Key passing tests observed:

- state: policy_deferred_auto_at_h, policy_deferred_act_pre_h, policy_deferred_act_at_h, policy_deferred_deact_pre_h, init_deferred_inactive_pre_h
- snapshot: tx_set_pol_def_rt, tx_init_pol_def_rt, tx_v2_deferred_needs_h

## 6. Verdict

Approve.

No blocking gaps found for slice3 review scope.

## 7. Participation / token estimate

```text
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260524-v5-s4-slice3-spec-tests-review.md
token_usage: { "source": "estimate", "input": 17000, "output": 2300, "total": 19300, "confidence": "medium" }
```

## 8. Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-s4-slice3-spec-tests-review.md'
git add 'tasks/20260524-v5-s4-slice3-spec-tests-review.json'
git commit -m 'docs(v5-4): add slice3 spec and snapshot review gate'
```