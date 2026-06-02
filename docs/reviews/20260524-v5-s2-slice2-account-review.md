# V5-2 Slice 2 Review: Account V5 Fields Migration

## 1. Scope recap

Reviewed V5-2 slice2 after coding and testing PASS for the Account V5-field migration across:

- [crates/pwm-core/src/types.rs](../../crates/pwm-core/src/types.rs)
- [crates/pwm-core/src/state.rs](../../crates/pwm-core/src/state.rs)
- adjacent compile-fix adapters in `pwm-cli` and `pwmd`

Claimed scope was:

- add `marks_last_block`, `deferred_policies`, `ipv4_claimed_phase`;
- remove legacy claim-era Account fields;
- avoid adding `address_flags` to `Account`;
- keep full lazy-marks semantics and snapshot-v3 behavior for later slices unless minimal compile shims are required.

## 2. Requirements fit

The structural field migration mostly lands:

- legacy Account claim fields are removed from the active `pwm-core` Account model;
- `deferred_policies` and `ipv4_claimed_phase` are present;
- no `address_flags` field was added to `Account`;
- compile adapters were added where needed.

But the slice does not preserve the RFC 0012 meaning of `marks_last_block`.

## 3. Style and module shape

The code change is larger than slice1 but still follows a reasonable migration pattern: core type first, then mechanical adapter updates.

The problem is not naming or layout. It is that the new V5 field is already being used with the old time-based semantics in core state transitions.

### Wire JSON / u128

Applicable.

This slice touches the active Account economic fields, especially `staked_pwm_raw`, and keeps public adapter output as decimal strings in [crates/pwmd/src/api/common.rs](../../crates/pwmd/src/api/common.rs). That part is consistent with the V5 JSON contract.

No new peer-wire `u128` hazard stood out in this slice.

## 4. Safety

Findings:

1. High: [crates/pwm-core/src/state.rs](../../crates/pwm-core/src/state.rs) writes `block_unix_time` into `marks_last_block` and computes `matured_units_available` from `delta_seconds / 3600`. RFC 0012 v2 defines `marks_last_block` as a chain-height cursor and the V5 timing basis as `delta_blocks / blocks_per_hour`. This slice therefore renames the field to the V5 name while keeping the retired V2 time semantics underneath it. That is a direct spec mismatch in the active state path, not just a deferred follow-up.

2. Medium: the snapshot compatibility adapters in [crates/pwmd/src/snapshot/types.rs](../../crates/pwmd/src/snapshot/types.rs) are acceptable as temporary shims, but because core state currently stores `marks_last_block` in Unix-time units on some paths, these shims now inherit an ambiguous mixed-unit field. That ambiguity should not be allowed to persist into later V5 slices.

## 5. Tests

Evidence reviewed:

- coding handoff in [tasks/done/20260524-v5-s2-slice2-account.json](../../tasks/done/20260524-v5-s2-slice2-account.json)
- testing handoff in [tasks/done/20260524-v5-s2-slice2-account-testing.json](../../tasks/done/20260524-v5-s2-slice2-account-testing.json)
- commit `0a42522`
- workspace/test results reported by pwm-testing

The testing gate correctly proved compile health and field removal, but it did not cover the semantic unit of `marks_last_block`. That is the missing check that exposes the current defect.

## 6. Verdict

Request changes.

Priority:

1. Make `marks_last_block` consistently chain-height based in active state code, or keep the old field semantics and defer the rename until the V5 timing model is actually introduced.
2. Keep snapshot compatibility shims only after the unit of `marks_last_block` is unambiguous across core state paths.

## 7. Participation / token estimate

```text
agent: pwm-review
result: FAIL
artifacts: docs/reviews/20260524-v5-s2-slice2-account-review.md
token_usage: { "source": "estimate", "input": 17000, "output": 2000, "total": 19000, "confidence": "medium" }
```

## 8. Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-s2-slice2-account-review.md'
git add 'tasks/20260524-v5-s2-slice2-account-review.json'
git commit -m 'docs(v5-2): add slice2 review gate report'
```