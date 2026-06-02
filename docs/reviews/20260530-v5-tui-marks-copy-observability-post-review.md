# Review: TUI marks copy + observability post-coding gate

**Date:** 2026-05-30  
**Agent:** pwm-review  
**Ticket:** `20260530-v5-tui-marks-copy-post-coding-review`  
**Verdict:** PASS

---

## 1. Gate checklist

| Criterion | Result |
|---|---|
| No ClaimTx/claim_mark user copy in pwm-tui | PASS — grep found only positive `F5_BURN_V5_STATUS` |
| No "Claim or Stake", "Marks materialize" stale strings | PASS — all removed |
| Runbook `docs/runbooks/v5-tui-marks-operator-path.md` exists | PASS — 23 lines, path + distinction |
| Runbook linked from `v5-cy-cluster-precloseout-soak.md` | PASS — line 53 |
| `format_marks_detail` + F5 hints match operator journey | PASS — tests confirm stake-first/wait-accrual/allow-effective |
| All pwm-tui tests pass | PASS — 38/38 |
| Naming policy | PASS — 0 violations across 5 files |
| `cargo check -p pwm-tui` | PASS |
| No product Rust edits in review | PASS |

---

## 2. Copy audit (grep results)

Single hit in `crates/pwm-tui/src/tui_loop.rs:43`:

```rust
const F5_BURN_V5_STATUS: &str =
    "V5 marks: stake PWM with S, wait for blocks, then burn materialized marks with F5.";
```

This is the positive operator path — no stale `ClaimTx`/`claim_mark`/`Claim or Stake` remains.

---

## 3. F5 hints verification

Three new hint tests added by coding ticket:

| Test | Scenario | Result |
|---|---|---|
| `f5_hint_stake_first` | `staked == 0` → tells operator to stake | PASS |
| `f5_hint_wait_accrual` | `staked > 0`, `marks == 0` → tells operator to wait | PASS |
| `f5_hint_allows_effective` | `effective_marks > 0` → allows burn | PASS |

All align with the normative journey from `20260530-v5-tui-marks-operator-journey-review.md`.

---

## 4. marks_display tests

| Test | Purpose | Result |
|---|---|---|
| `marks_detail_uses_effective` | Uses effective marks for hint | PASS |
| `marks_detail_pending_hint` | Show "(pending)" when lazy delta | PASS |
| `marks_detail_no_small_hint` | No hint when trivial | PASS |

---

## 5. Runbook content summary

- Step-by-step: Stake S → wait blocks → watch marks → burn F5
- ClaimTx vs ClaimIPv4Batch distinction documented
- Quick wording: "V5 marks path: stake PWM with S, wait for block height to advance, watch Marks grow, then burn with F5."
- Russian version available if needed (omitted from runbook per coding ticket spec)

---

## 6. Verdict

**PASS** — all stale Claim copy removed, positive operator path in place, runbook linked, hints match journey, tests clean.

**Verdict line:** `PASS — zero stale Claim copy; F5 hints cover stake-first/wait/allow-effective; runbook linked from soak doc; 38/38 tests.`