# V2-4 Slice 0 — UX Freeze: BURN_MARK full operator path

**Date:** 2026-05-06  
**Sprint:** V2-4 (docs/plans/mvp_v2.md)  
**Author:** orchestrator

---

## Context

Sprint E-3 shipped a working `tx-burn-mark` CLI command and F5 burn-mark TUI modal.  
Sprint V2-2 migrated the RPC `AcctOut` to a single `marks` field (removed `marks_quota`).

Current gaps:

| # | Gap | Surface |
|---|-----|---------|
| G1 | `AcctRow` (TUI model) has no `marks` field → marks not shown in account table | `pwm-tui/src/models.rs` |
| G2 | TUI burn-form (F5) does not pre-fill/display current marks balance | `burn_form.rs`, `account_view.rs` |
| G3 | CLI `run_tx_burn_mark` does not print `marks` before/after submit | `cmd_tx.rs`, `rpc_helpers.rs` |
| G4 | No negative e2e for `InsufficientMarks` (marks < burn amount) | test coverage |
| G5 | Error text from `pwmd` on InsufficientMarks not checked for UX match in CLI/TUI | — |
| G6 | `docs/tester-guide-cli-tui-scenarios.md` lacks stake→accrue→burn scenario | docs |

---

## Acceptance criteria (Sprint V2-4 overall)

1. **AC-1 (CLI):** `pwm cli acct show` (or any balance command) prints `marks: <N>` alongside PWM balance.
2. **AC-2 (CLI burn):** `pwm cli tx-burn-mark` prints current marks before submit (fetched from RPC) and confirms post-submit.
3. **AC-3 (TUI table):** Account table shows `marks` column (or sub-row) for owned accounts.
4. **AC-4 (TUI burn form):** F5 burn form shows current marks balance at top of form as read-only info.
5. **AC-5 (negative test):** `cargo test -p pwm-cli` includes a unit/integration test asserting that submitting `BurnMark { mark_amount > marks }` returns an error matching `InsufficientMarks` (or equivalent node error string).
6. **AC-6 (error consistency):** CLI and TUI surface the same human-readable text for rejection; verified by `pwm-review` in Slice 3.
7. **AC-7 (docs):** `docs/tester-guide-cli-tui-scenarios.md` gets a section: `§ stake → accrue → burn` scenario with commands and expected output snippets.

---

## Slice plan

| Slice | Owner | Scope |
|-------|-------|-------|
| **0** (this doc) | orchestrator | UX freeze, AC list, commands/fields inventory |
| **1** | `pwm-coding` | G1 (AcctRow marks), G2 (TUI burn form), G3 (CLI pre/post print) + AC-5 negative unit test |
| **2** | `pwm-coding` | G4 if harness available (negative e2e), or skip to unit; refine TUI renders |
| **3** | `pwm-coding` + `pwm-review` | G6 (docs), AC-6 review of error text consistency |
| **4** (reserved) | `pwm-review` + `pwm-coding` | Full workspace naming audit |

---

## Commands / flags inventory (Slice 1 scope)

### CLI

| Command | Current status | Required change |
|---------|---------------|----------------|
| `tx-burn-mark --amount N [--beneficiary B] [--purpose P]` | Implemented (E-3) | Add pre-submit marks fetch + print; post-submit confirm line |
| `acct show` / `acct list` | Shows PWM balance | Add `marks: N` line in output |

### TUI

| Screen | Current status | Required change |
|--------|---------------|----------------|
| Account table | Shows PWM balance, staked, init | Add `marks` column or inline info row |
| F5 Burn-mark form | Functional modal, fields: marks amount, beneficiary, purpose | Add read-only `Current marks: N` header inside form |

---

## Error string reference (to be verified in Slice 3)

`pwmd` rejection path for insufficient marks should produce a JSON error body or an HTTP 400 with a string containing `InsufficientMarks` (or the node's canonical variant). Slice 1 captures the exact string in a constant/helper for reuse in CLI/TUI user messages.

---

## Files in scope (Slice 1)

- `crates/pwm-tui/src/models.rs` — add `marks: u128` to `AcctRow`
- `crates/pwm-tui/src/account_view.rs` — parse `x["marks"]` when building `AcctRow`; add marks column render
- `crates/pwm-tui/src/burn_form.rs` — add `marks_available: u128` field; render it
- `crates/pwm-tui/src/tui_loop.rs` — pass marks into burn form on F5 open
- `crates/pwm-cli/src/cmd_tx.rs` — fetch current marks via `/v1/accounts/{id}` before submit; print pre/post
- `crates/pwm-cli/src/rpc_helpers.rs` — add `fetch_marks(c, rpc, from) -> u128` helper
- `crates/pwm-cli/src/tests/mod.rs` — add negative unit test for InsufficientMarks path
