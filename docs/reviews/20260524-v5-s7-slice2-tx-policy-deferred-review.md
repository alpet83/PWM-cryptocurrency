# Review Report: V5-7 Slice2 Tx Policy Deferred

## 1. Scope recap

- Ticket: `20260524-v5-s7-slice2-tx-policy-deferred-review`
- Commit reviewed: `fed8426`
- Claimed scope: CLI support for `--activation deferred` plus mandatory `--activate-at-height` in `tx-policy-set`, with tests.
- MVP anchor: `docs/plans/mvp_v5.md#sprint-v5-7-cli-enhancements--21b-genesis-design-doc`

## 2. Requirements fit

Status: covered.

- `TxPolicySet` received `activate_at_height: Option<u64>` CLI flag.
- Dispatch path now forwards `activate_at_height` into `run_tx_policy_set`.
- Activation parser supports `deferred` and enforces `--activate-at-height > 0`.
- Error message for missing/invalid deferred height is explicit and user-facing.
- Existing `InitPolicy` parsing remains strict (`deferred` rejected there by design due missing height context).

## 3. Style and module shape

Status: aligned.

- Naming policy check on touched files reports zero violations.
- Help text was updated consistently in CLI command definition.
- Module boundaries are preserved (no `pwm-core` behavior edits in this slice).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

Status: no blocking safety findings.

- Deferred activation path rejects ambiguous/unsafe defaults by requiring explicit positive height.
- No new unchecked unwraps in hot command path for this slice.
- Change is input-validation oriented and narrows invalid states.

## 5. Tests

Executed:

- `cargo test -p pwm-cli tx_policy_set` -> PASS (3 relevant tests)
- `cargo check -p pwm-cli` -> PASS
- `python scripts/check_entity_name_segments.py crates/pwm-cli/src/cli_cmd.rs crates/pwm-cli/src/cli_dispatch.rs crates/pwm-cli/src/cmd_tx.rs crates/pwm-cli/src/tests/mod.rs` -> PASS (no violations)

Coverage notes:

- Added tests validate deferred parse happy path and missing-height failure.
- Existing tx-policy parse tests continue to pass.

## 6. Verdict

Verdict: approve.

Priority findings:

1. None blocking.

## 7. Participation / token estimate

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260524-v5-s7-slice2-tx-policy-deferred-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 9200, "confidence": "low" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-s7-slice2-tx-policy-deferred-review.md'
git add 'tasks/20260524-v5-s7-slice2-tx-policy-deferred-review.json'
git commit -m 'docs(v5-7): slice2 deferred activation review gate report'
```