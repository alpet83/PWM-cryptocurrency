# V5-5 Slice 2 Review: ClaimIPv4Batch Reject Matrix + Fixture

## 1. Scope recap

Reviewed commit `795d170` (on top of slice1 baseline `f016074`) for V5-5 slice2 review gate.

Claimed scope:

- reject-matrix tests for `TxBody::ClaimIPv4Batch` in `crates/pwm-core/src/state.rs`:
  - unknown phase,
  - bad registry signature,
  - double-claim,
  - claimant not initialized;
- assertions that state is unchanged on reject;
- no CLI/TUI/pwmd expansion.

Checked artifacts:

- `tasks/done/20260524-v5-s5-slice2-reject-fixture.json`
- `tasks/done/20260524-v5-s5-slice1-apply-happy.json`
- `tasks/introductory/20260524-v5-s5-ipv4-claim-onchain.md`
- `crates/pwm-core/src/state.rs`

## 2. Requirements fit

Slice2 acceptance criteria are satisfied.

- All four reject scenarios are present as dedicated `claim_` tests:
  - `claim_ipv4_phase_unknown` -> `TxError::PolicySchemaInvalid`
  - `claim_ipv4_sig_bad_reject` -> `TxError::BadSignature`
  - `claim_ipv4_double_reject` -> `TxError::PolicyDenied`
  - `claim_ipv4_uninit_reject` -> `TxError::NotInitialized`
- Each reject test snapshots pre-state and verifies no mutation after failure (`accounts` and `fee_pool` unchanged).
- Happy path remains covered by `claim_ipv4_batch_happy_apply` and passes in the same `claim_` suite.
- Diff scope is limited to `crates/pwm-core/src/state.rs` test code plus task metadata; no CLI/TUI/pwmd scope creep observed.

## 3. Style and module shape

Naming policy check on touched Rust file passed with zero violations.

Evidence:

- `python scripts/check_entity_name_segments.py crates/pwm-core/src/state.rs`
- Output: `violations: []`.

Module shape remains focused and appropriate for final coding slice of V5-5.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

No blocking safety findings within scope.

- Reject-path tests reduce regression risk by asserting stable error taxonomy and non-mutation on failures.
- Claim authorization behavior introduced in slice1 is not loosened by slice2 changes.

## 5. Tests

Executed checks:

- `python scripts/check_entity_name_segments.py crates/pwm-core/src/state.rs` -> PASS
- `cargo test -p pwm-core claim_ --lib` -> PASS

Observed claim-suite result: 10 passed, 0 failed.

## 6. Verdict

Approve.

Slice2 review gate passes; ready for pwm-testing and sprint V5-5 closeout chain.

## 7. Participation / token estimate

```text
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260524-v5-s5-slice2-reject-fixture-review.md
token_usage: { "source": "estimate", "input": 15000, "output": 2100, "total": 17100, "confidence": "medium" }
```

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-s5-slice2-reject-fixture-review.md'
git add 'tasks/20260524-v5-s5-slice2-reject-fixture-review.json'
git commit -m 'docs(v5-5): add slice2 claim reject-matrix review gate'
```