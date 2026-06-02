# V5-2 Slice 5 Review: Snapshot Schema v3 Integrated Gate

## 1. Scope recap

Reviewed the final V5-2 slice after coding and testing PASS for snapshot schema v3 and v2 migration support across:

- [crates/pwmd/src/snapshot/types.rs](../../crates/pwmd/src/snapshot/types.rs)
- [crates/pwmd/src/snapshot/io.rs](../../crates/pwmd/src/snapshot/io.rs)
- [crates/pwmd/src/snapshot/repair.rs](../../crates/pwmd/src/snapshot/repair.rs)

This review also cross-checks the whole V5-2 sprint gate against earlier review findings from slice1, slice2, and slice3.

## 2. Requirements fit

The snapshot slice itself does useful work:

- canonical snapshot version is bumped to v3;
- explicit v2 and v1 loading paths remain gated rather than silently accepted;
- public snapshot JSON carries the new V5 account shape and keeps decimal-string encoding for economic values;
- targeted snapshot migration/replay tests exist.

But the full V5-2 sprint gate is not ready to approve.

## 3. Style and module shape

The snapshot code is structured reasonably: separate v2 and v3 wire structs, explicit conversion helpers, and clear version branching in load/decode.

The problem is not snapshot layout. It is that the integrated gate now hardens a `marks_last_block` meaning that still conflicts with RFC 0012 in active runtime code.

### Wire JSON / u128

Applicable.

Snapshot v3 public JSON uses decimal-string representations for economic values, which is aligned with the V5 JSON contract.

No new peer-wire `u128` issue stood out in the snapshot v3 slice itself.

## 4. Safety

Findings:

1. High: the integrated V5-2 gate still carries the slice2 blocker. RFC 0012 v2 defines `marks_last_block` as a chain-height cursor, but active runtime code in [crates/pwm-core/src/state.rs](../../crates/pwm-core/src/state.rs) writes timestamp-based values. The snapshot slice then codifies that ambiguity further: [crates/pwmd/src/snapshot/repair.rs](../../crates/pwmd/src/snapshot/repair.rs) asserts `marks_last_block == applied_block_ts`, and the coding handoff explicitly notes that strict height-based cursors remain unresolved for later work. That means V5-2 does not yet satisfy its own RFC-aligned core-model contract.

2. Medium: the full sprint gate also still inherits the unresolved slice1 and slice3 blockers:
   - `season_coeff_ppm` remains `u128` in code while RFC 0019 specifies `u64`.
   - the exact structured retire-path for legacy `claim_mark` wire input is still not evidenced directly.

Because this ticket explicitly asks for a holistic sprint-closeout assessment, those previously documented blockers must remain part of the final verdict.

## 5. Tests

Evidence reviewed:

- coding handoff in [tasks/done/20260524-v5-s2-slice5-snapshot-v3.json](../../tasks/done/20260524-v5-s2-slice5-snapshot-v3.json)
- testing handoff in [tasks/done/20260524-v5-s2-slice5-snapshot-v3-testing.json](../../tasks/done/20260524-v5-s2-slice5-snapshot-v3-testing.json)
- commit `73aa13c`
- prior review findings from:
  - [docs/reviews/20260524-v5-s2-slice1-gencfg-review.md](20260524-v5-s2-slice1-gencfg-review.md)
  - [docs/reviews/20260524-v5-s2-slice2-account-review.md](20260524-v5-s2-slice2-account-review.md)
  - [docs/reviews/20260524-v5-s2-slice3-drop-claim-review.md](20260524-v5-s2-slice3-drop-claim-review.md)
  - [docs/reviews/20260524-v5-s2-slice4-ipv4-batch-review.md](20260524-v5-s2-slice4-ipv4-batch-review.md)

The slice5 test matrix is strong. The blocking issue is contract coherence across the sprint, not lack of executable coverage.

## 6. Verdict

Request changes.

Priority:

1. Resolve the `marks_last_block` unit mismatch before approving V5-2 as a sprint gate. The core model and snapshot model must agree on height vs timestamp semantics.
2. Resolve the earlier slice1 blocker for `season_coeff_ppm` type parity with RFC 0019.
3. Resolve the earlier slice3 blocker by proving or implementing structured handling for legacy `claim_mark` wire input.

Slice5 itself is close to acceptable, but the integrated sprint gate cannot pass while those blockers remain open.

## 7. Participation / token estimate

```text
agent: pwm-review
result: FAIL
artifacts: docs/reviews/20260524-v5-s2-slice5-snapshot-v3-review.md
token_usage: { "source": "estimate", "input": 19000, "output": 2200, "total": 21200, "confidence": "medium" }
```

## 8. Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-s2-slice5-snapshot-v3-review.md'
git add 'tasks/20260524-v5-s2-slice5-snapshot-v3-review.json'
git commit -m 'docs(v5-2): add slice5 integrated review gate report'
```