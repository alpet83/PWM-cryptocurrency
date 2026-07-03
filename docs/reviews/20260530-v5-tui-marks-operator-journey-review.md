# Review: V5 TUI marks operator journey + stale ClaimTx copy

**Date:** 2026-05-30  
**Agent:** pwm-review  
**Ticket:** `20260530-v5-tui-marks-operator-journey-review`  
**Scope:** review-only UX/spec audit; no product code edits  

---

## 1. Scope recap

Owner reports that V5 removed `ClaimTx`, but TUI still mentions “Claim” and the operator path from zero marks is unclear. This review defines the normative operator journey, inventories stale TUI copy, and scopes one coding ticket for copy/runbook cleanup.

Reviewed sources:

- `docs/rfc/12-claim-maturity-and-state-model.md`
- `docs/plans/mvp_v5.md`
- `docs/GLOSSARY.md`
- `tasks/done/20260528-v5-audit-warn5-tui-retire-claimtx-ui.json`
- `crates/pwm-tui/src/tui_loop.rs`
- `crates/pwm-tui/src/burn_form.rs`
- `crates/pwm-tui/src/status.rs`
- `crates/pwm-tui/src/models.rs`
- `crates/pwm-tui/src/marks_display.rs`

---

## 2. Normative V5 operator journey

V5 marks are lazy and staked-only. There is no standalone `ClaimTx` for marks materialization.

Operator path in TUI/devnet:

1. **Get PWM balance** in an owner wallet row.
2. **Stake PWM with `S`**. Staking touches the account and starts lazy mark accrual from the current chain height.
3. **Wait for chain head advance**. Marks accrue by height: `delta_hours = floor((head - marks_last_block) / blocks_per_hour)`. With default `blocks_per_hour=3600`, one nominal hour of blocks is needed for one hour of mark generation.
4. **Watch the marks column**. TUI may display effective marks from `stored_marks + lazy_delta` without mutating state.
5. **Burn with `F5`** once marks are available. `BurnMark` touches the owner, materializes effective marks into stored marks, then checks/burns the requested amount.
6. **Use `U`/Unstake or transfer as touch paths when needed**. Stake/Unstake/Transfer/Burn/PolicyTx touch marks per RFC 0012; display-only surfaces can calculate effective marks without a state mutation.

Important distinction:

- `ClaimTx` / `claim_mark` is retired in V5 and must not appear as an operator path for marks.
- `ClaimIPv4Batch` is a separate on-chain IPv4 allocation transaction tied to registry phases. It is not a TUI marks materialization path and is currently handled through CLI/harness flows, not the TUI F5 burn journey.

Short operator wording suitable for TUI/runbook:

```text
V5 marks path: Stake PWM (S), wait for block height to advance, watch Marks grow, then burn with F5. No ClaimTx in V5.
```

Russian-friendly version:

```text
Путь V5: сначала застейкать PWM (S), подождать рост высоты блоков, затем жечь накопленные марки через F5. ClaimTx для марок в V5 нет.
```

---

## 3. TUI stale copy inventory

| File / line | Current text / reference | Classification | Recommendation |
|---|---|---|---|
| `crates/pwm-tui/src/tui_loop.rs:41` | `ClaimTx is retired in V5; burn uses materialized marks only; fill burn fields` | Rewrite | Replace with positive operator path: `V5 marks: stake PWM (S), wait for blocks, then burn materialized marks with F5.` |
| `crates/pwm-tui/src/tui_loop.rs:591` | `No materialized marks yet. ClaimTx is retired in V5; Burn uses materialized marks only.` | Rewrite | If `staked == 0`: tell operator to stake first. If `staked > 0`: tell operator to wait for block accrual / head advance. Avoid making retirement jargon the main instruction. |
| `crates/pwm-tui/src/tui_loop.rs:1135` | `f5_burn_status()` composes `F5_BURN_V5_STATUS` | Keep function, update constant | Function shape is fine; copy source is stale. |
| `crates/pwm-tui/src/tui_loop.rs:1424` | `Marks materialize via Claim or Stake/Unstake. Burn uses materialized marks.` | Remove/Rewrite | Remove `Claim`. Suggested: `Marks accrue lazily while staked; Stake/Unstake/Transfer/Burn touch and materialize them.` |
| `crates/pwm-tui/src/tui_loop.rs:1556` | test `f5_retired_claim_no_submit` asserts no legacy submit copy | Update test | Rename or update asserts to ensure no `Claim` path is advertised and no legacy submit text appears. |
| `crates/pwm-tui/src/burn_form.rs:49` | `Marks materialize via Claim or Stake/Unstake. Burn uses materialized marks.` | Remove/Rewrite | Same as modal line; source constructor copy should match rendered help. |
| `crates/pwm-tui/src/status.rs:142-149` | Footer has `Stake`, `Unstake`, `F5 burn` | Keep | This already exposes the right keys; optional one-line action hint can point to S -> wait -> F5 when zero marks. |
| `crates/pwm-tui/src/models.rs` | `marks`, `effective_marks`, `marks_sat_pct`, `staked` | Keep | Data model supports the V5 path. |
| `crates/pwm-tui/src/marks_display.rs` | effective marks calculation/display | Keep | Correctly computes display-only lazy marks against head height. |

No TUI user-visible references to `claim_mark` were found. Remaining problematic references are copy strings that imply a marks “Claim” path.

---

## 4. UX gap assessment

Current code technically prevents the retired `ClaimTx` flow from being submitted, which satisfies the original WARN-005 safety finding. However, the UX is still confusing:

- The zero-marks block tells users only that `ClaimTx` is retired, not how to get marks.
- The burn modal explicitly says “Claim or Stake/Unstake,” which contradicts RFC 0012 v2.
- The positive path `S -> wait blocks -> marks column -> F5` is not documented in the TUI or an operator runbook.
- `ClaimIPv4Batch` and retired `ClaimTx` are easy to confuse because both contain “Claim,” but they are different subsystems.

This is not a consensus bug and not a product-code safety blocker, but it is a closeout UX gap for V5 operator testing.

---

## 5. Requirements fit

| Requirement | Result |
|---|---|
| Operator path doc section | PASS — see §2 |
| Inventory all user-visible Claim references in `pwm-tui` | PASS — see §3 |
| Distinguish `ClaimTx` vs `ClaimIPv4Batch` | PASS — see §2 |
| Recommend coding ticket copy/runbook | PASS — see §7 |
| No product Rust edits in review | PASS |

---

## 6. Verification performed

Commands run:

```text
python scripts/check_entity_name_segments.py crates/pwm-tui/src/tui_loop.rs crates/pwm-tui/src/burn_form.rs crates/pwm-tui/src/status.rs crates/pwm-tui/src/models.rs
cargo test -p pwm-tui --lib f5_retired_claim_no_submit
cargo test -p pwm-tui --lib marks_display
cargo check -p pwm-tui
```

Results:

- Naming policy: PASS, no violations.
- `f5_retired_claim_no_submit`: PASS.
- `marks_display`: PASS, 5 tests.
- `cargo check -p pwm-tui`: PASS.

Additional searches:

```text
grep: ClaimTx|claim_mark|Claim or Stake|Claim or|Claim\b|claim\b|materialized marks|F5_BURN_V5_STATUS|No materialized marks|Marks materialize
```

Result: no active `claim_mark` UI path found; stale copy remains in `tui_loop.rs` and `burn_form.rs`.

---

## 7. Recommended coding ticket

Use the already queued ticket:

```text
20260530-v5-tui-v5-marks-copy-operator-path-coding
```

Minimal scope:

- `crates/pwm-tui/src/tui_loop.rs`
- `crates/pwm-tui/src/burn_form.rs`
- `docs/runbooks/v5-tui-marks-operator-path.md`
- Optional link from `docs/runbooks/v5-cy-cluster-precloseout-soak.md` or `docs/plans/mvp_v5.md`

Acceptance notes for coding:

1. Replace “ClaimTx is retired…” status with a positive V5 path.
2. Replace all “Marks materialize via Claim…” copy; no user-facing “Claim or Stake” for marks.
3. Improve zero-marks modal:
   - if `staked == 0`: `Stake PWM with S, wait for block height, then burn marks with F5.`
   - if `staked > 0`: `Marks accrue lazily while staked; wait for head height to advance or touch with Stake/Unstake/Transfer.`
4. Add a short runbook with the operator path and the `ClaimIPv4Batch != ClaimTx` distinction.
5. Update tests to assert no stale ClaimTx/Claim path copy in TUI burn messages.

---

## 8. Verdict

**PASS_WITH_NITS** — the retired `ClaimTx` submit path is gone and V5 marks display primitives exist, but the TUI copy still fails the operator journey test. This should be fixed by the queued copy/runbook coding ticket before V5 CY closeout.

**Verdict line:** `PASS_WITH_NITS — no active ClaimTx path; stale Claim copy must be replaced with S -> wait blocks -> F5 operator guidance.`

---

## 9. Participation / token estimate

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts: docs/reviews/20260530-v5-tui-marks-operator-journey-review.md
token_usage:
  source: estimate
  input: 22000
  output: 3600
  total: 25600
  confidence: medium
```

---

## 10. Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'P:\opt\docker\pwm-protocol'
git add 'docs/reviews/20260530-v5-tui-marks-operator-journey-review.md'
git add 'tasks/done/20260530-v5-tui-marks-operator-journey-review.json'
git commit -m 'docs(v5-tui): review marks operator journey'
```
