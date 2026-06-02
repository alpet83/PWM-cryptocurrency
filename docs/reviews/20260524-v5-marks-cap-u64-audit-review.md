# Review: MARKS_CAP adoption audit + marks u128 math scope

Ticket: `20260524-v5-marks-cap-u64-audit-review`

Status: PASS

Parent: `20260524-v5-marks-u64-arithmetic-profile` (closed)

Baseline: `735d9fa` (`pub const MARKS_CAP`, u64 `compute_lazy_marks`)

## Scope Recap

This audit checked two things:

1. Where `u32::MAX` still appears in marks-related code paths and whether each site should become `MARKS_CAP`, stay as-is, or be treated as out of scope.
2. Which remaining `u128` uses in marks-adjacent math are still justified after the RFC 0012 v2 u64 lazy-marks profile.

Reviewed anchors:

- [RFC 0012 v2](../rfc/12-claim-maturity-and-state-model.md)
- [Previous bounds review](./20260524-v5-marks-u64-arithmetic-bounds-review.md)
- [marks.rs](../../crates/pwm-core/src/marks.rs)

## Requirements Fit

The repository already has the core adoption in place: `MARKS_CAP` is defined once in [crates/pwm-core/src/marks.rs](../../crates/pwm-core/src/marks.rs), and the lazy marks hot path there is already u64-based. The remaining work is mostly mechanical clamp replacement and inventory hygiene.

I found three categories:

### Replace with `MARKS_CAP`

These are marks-cap clamps or tests that should use the named constant rather than an inline `u32::MAX`.

| File | Site | Rationale |
|---|---|---|
| [crates/pwm-core/src/state.rs](../../crates/pwm-core/src/state.rs) | `as_marks_u32` clamp | clamp belongs to marks cap semantics; replace `u32::MAX as u128` with `MARKS_CAP as u128` |
| [crates/pwm-core/src/state.rs](../../crates/pwm-core/src/state.rs) | saturation test at `u32::MAX` | use named cap in assertions |
| [crates/pwm-core/src/genesis.rs](../../crates/pwm-core/src/genesis.rs) | premine `stored_marks` init | seed should clamp via `MARKS_CAP` |
| [crates/pwm-core/src/genesis.rs](../../crates/pwm-core/src/genesis.rs) | saturation test | use named cap in assertions |
| [crates/pwm-core/src/types.rs](../../crates/pwm-core/src/types.rs) | `migrate_marks_legacy` clamp | legacy decode still clamps to the marks ceiling; use `MARKS_CAP` for the ceiling value |
| [crates/pwmd/src/snapshot/types.rs](../../crates/pwmd/src/snapshot/types.rs) | snapshot marks decode clamps | same ceiling semantics as core migration |
| [crates/pwm-core/src/marks.rs](../../crates/pwm-core/src/marks.rs) | test literals | use `MARKS_CAP` in saturation assertions |
| [crates/pwm-tui/src/tui_loop.rs](../../crates/pwm-tui/src/tui_loop.rs) | marks cell saturation branch and tests | display should follow the same named cap |
| [crates/pwm-tui/src/marks_display.rs](../../crates/pwm-tui/src/marks_display.rs) | test literals | use named cap in display tests |

### Keep as-is

| File | Site | Rationale |
|---|---|---|
| [crates/pwm-core/src/marks.rs](../../crates/pwm-core/src/marks.rs) | `pub const MARKS_CAP: u32 = u32::MAX;` | definition site only; the literal belongs here |
| [crates/pwm-core/src/marks.rs](../../crates/pwm-core/src/marks.rs) | u64 `compute_lazy_marks` implementation | matches RFC 0012 v2 profile; no u128 hot-path debt here |
| [crates/pwm-tui/src/marks_display.rs](../../crates/pwm-tui/src/marks_display.rs) | percentage calc with `u128` | display-only percentage math; harmless and not accumulation math |

### Out of scope

| File | Site | Rationale |
|---|---|---|
| [crates/pwmd/src/transport/peer_session/sync_live.rs](../../crates/pwmd/src/transport/peer_session/sync_live.rs) | `retry_after_ms` clamp | transport heartbeat cap, not marks |
| [crates/pwm-core/src/marks.rs](../../crates/pwm-core/src/marks.rs) | `compute_block_reward` | inflation math, not lazy marks |
| [crates/pwm-core/src/state.rs](../../crates/pwm-core/src/state.rs) | `accrue_marks` / `accrue_marks_v2` | legacy accumulation helpers; no current lazy-marks hot-path call site was found in this audit |
| [crates/pwm-cli/src/cli_cmd.rs](../../crates/pwm-cli/src/cli_cmd.rs) | `CLAIM_ALL` / `ClaimTx` help text | retired ClaimTx path, not marks cap semantics |
| [crates/pwm-core/src/tx.rs](../../crates/pwm-core/src/tx.rs) | retired `claim_mark` wire error | legacy claim wire surface, not marks cap semantics |

## U128 Lazy-Marks Debt

The u128 uses that remain are mostly compatibility or non-lazy-marks math:

- [crates/pwm-core/src/types.rs](../../crates/pwm-core/src/types.rs) `migrate_marks_legacy(raw: u128)` and `de_marks_compat`: keep `u128` input handling for legacy wire compatibility, but clamp to `MARKS_CAP`.
- [crates/pwmd/src/snapshot/types.rs](../../crates/pwmd/src/snapshot/types.rs) legacy snapshot decode helpers: same compatibility rationale.
- [crates/pwm-core/src/state.rs](../../crates/pwm-core/src/state.rs) `accrue_marks` / `accrue_marks_v2`: legacy helpers, not the RFC 0012 lazy-marks fast path.
- [crates/pwm-tui/src/marks_display.rs](../../crates/pwm-tui/src/marks_display.rs) percentage formatting: display-only, not accumulation.
- [crates/pwm-core/src/marks.rs](../../crates/pwm-core/src/marks.rs) `compute_lazy_marks`: already migrated to u64 and aligned with the RFC profile.

## Safety

No security blocker was found in the audit itself. The only real risk is maintainability drift: if the remaining clamp sites keep using bare `u32::MAX`, future reviewers can miss the shared marks ceiling and accidentally reintroduce inconsistent semantics.

## Tests

The targeted marks suite already passes in the checked tree: `cargo test -p pwm-core marks_ --lib` returned success. That is enough for this audit because the work here is inventory and classification, not runtime behavior changes.

## Verdict

PASS.

The codebase is already on the new `MARKS_CAP`/u64 lazy-marks contract. Remaining edits are mechanical and safe to hand to a single narrow coding slice.

Recommended follow-up coding slice:

1. Replace the mechanical `u32::MAX` clamp/test literals in `state.rs`, `genesis.rs`, `types.rs`, `pwmd/src/snapshot/types.rs`, `pwm-tui/src/tui_loop.rs`, and marks-related tests.
2. Leave the definition site in `marks.rs`, the legacy claim text/wire surfaces, and transport heartbeat clamps alone.

## Participation / token estimate

agent: pwm-review

result: PASS

artifacts: [docs/reviews/20260524-v5-marks-cap-u64-audit-review.md](docs/reviews/20260524-v5-marks-cap-u64-audit-review.md)

token_usage: { "source": "estimate", "input": 5800, "output": 1600, "total": 7400, "confidence": "medium" }

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-marks-cap-u64-audit-review.md'
git commit -m 'docs(v5): audit MARKS_CAP adoption and u64 marks scope'
```
