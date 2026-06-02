# Review: V5 audit WARN fixes

**Date:** 2026-05-28  
**Agent:** pwm-review  
**Ticket:** `20260528-v5-audit-warn-fixes-review`  
**Scope window:** `85d19bb..6592300`  
**Reviewed commit:** `6592300 fix(v5-audit): resolve MVP audit WARN-001..005`  
**Related HIGH commits:** `22051bb`, `c6ffefe`  

---

## 1. Scope recap

This review covers the integrated coding fixes for the MVP V5 audit findings from `docs/reviews/20260528-v5-mvp-rust-code-audit-review.md`:

- `WARN-001`: lazy marks touch consistency on `ClaimIPv4Batch`, `Export`, `Import`.
- `WARN-002`: direct-seal cancellation/idempotency contract in `pwmd` tx handler.
- `WARN-003`: reduce `TxSignerSource` public raw signing-key exposure.
- `WARN-004`: reject duplicate `ipv4_claim_phases` in genesis parse.
- `WARN-005`: remove retired `ClaimTx` submission path from TUI F5 flow.

The review also sanity-checks that earlier HIGH fixes remain consistent:

- `HIGH-001` / `22051bb`: deferred reversible policies can be deactivated at/after auto-activation height.
- `HIGH-002` / `c6ffefe`: `claim-ipv4-batch` fails closed for signing material and uses explicit dev opt-in.

Changed Rust files in `85d19bb..6592300`:

- `crates/pwm-core/src/state.rs`
- `crates/pwmd/src/api/handlers_tx.rs`
- `crates/pwmd/src/snapshot/genesis.rs`
- `crates/pwm-cli/src/signer.rs`
- `crates/pwm-cli/src/bin/claim_ipv4_batch.rs`
- `crates/pwm-tui/src/lib.rs`
- `crates/pwm-tui/src/tui_loop.rs`
- `crates/pwm-tui/src/tx_submit.rs`
- `crates/pwm-tui/src/marks_display.rs`

---

## 2. Requirements fit

| Finding | Closure evidence | Tests / checks | Result |
|---|---|---|---|
| `WARN-001` marks touch consistency | `crates/pwm-core/src/state.rs:294`, `crates/pwm-core/src/state.rs:312`, `crates/pwm-core/src/state.rs:381`, `crates/pwm-core/src/state.rs:394` call `touch_state_acct`; helper comment at `crates/pwm-core/src/state.rs:535` documents invariant | `cargo test -p pwm-core --lib claim_ipv4_touch_marks`; `cargo test -p pwm-core --lib export_touch_marks`; `cargo test -p pwm-core --lib import_touch_marks` | PASS |
| `WARN-002` direct-seal cancellation contract | Comment at `crates/pwmd/src/api/handlers_tx.rs:88` documents not-fully-cancel-safe contract and retry/idempotency expectation | Review-only doc/code comment; no behavior change required by ticket | PASS |
| `WARN-003` signer API surface | `TxSignerSource` fields are `pub(crate)` at `crates/pwm-cli/src/signer.rs:14`; narrow accessors at `crates/pwm-cli/src/signer.rs:21`; `claim-ipv4-batch` uses accessors at `crates/pwm-cli/src/bin/claim_ipv4_batch.rs:102` | `cargo check -p pwm-cli --bin pwm --bin claim-ipv4-batch` | PASS |
| `WARN-004` duplicate genesis phases | `BTreeSet` duplicate check at `crates/pwmd/src/snapshot/genesis.rs:237`; duplicate error includes field path and phase value | `cargo test -p pwmd --lib gen_ipv4_phases` | PASS |
| `WARN-005` retired ClaimTx UI | `submit_claim` export removed from `crates/pwm-tui/src/lib.rs:75`; function removed from `crates/pwm-tui/src/tx_submit.rs`; F5 flow builds burn form directly at `crates/pwm-tui/src/tui_loop.rs:600`; test at `crates/pwm-tui/src/tui_loop.rs:1543` | `cargo test -p pwm-tui --lib f5_retired_claim_no_submit`; `cargo check -p pwm-tui` | PASS |

Acceptance criteria are satisfied:

- Per-WARN evidence is present with file/line/test mapping.
- HIGH-001 remains consistent: deactivation now removes matching deferred rows regardless of activation height (`crates/pwm-core/src/state.rs:589`), with tests for before/at/after height.
- HIGH-002 remains consistent with WARN-003: `claim-ipv4-batch` requires explicit claimant material and explicit registry material/dev opt-in, while `TxSignerSource` no longer exposes public fields.
- No product code was edited by this review; only this report and ticket state are review outputs.

---

## 3. Style and module shape

- Ran `python scripts/check_entity_name_segments.py` over the changed focus files; no naming violations.
- `touch_state_acct` is intentionally small and documents the account-mutating invariant without broad refactor.
- `TxSignerSource` accessors are clear and minimal; the raw `SigningKey` remains consumable only through a deliberate `into_signing_key` move for harness use.
- The TUI F5 cleanup removes dead claim branches rather than layering more compatibility code over the retired path.

### Wire JSON / u128

No peer wire or RFC wire contract was changed by this WARN-fix slice.

Relevant reviewed surfaces:

- `state.rs` marks-touch additions do not change transaction JSON encoding.
- `handlers_tx.rs` adds cancellation-contract comments only.
- `signer.rs` and `claim_ipv4_batch.rs` adjust key-source access and argument validation only; `TxBody::ClaimIPv4Batch` JSON shape is unchanged.
- `genesis.rs` duplicate phase validation changes accepted/rejected genesis payloads but does not introduce new `u128` wire encoding.

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

---

## 4. Safety

- No `unsafe` was introduced in the reviewed diff.
- `claim-ipv4-batch` now fails closed when signer inputs are absent, removing deterministic fallback seeds. Registry key reuse requires explicit `--dev-registry-is-claimant` opt-in.
- `TxSignerSource` raw fields are no longer public; downstream code cannot accidentally access `sk` as a struct field.
- Direct-seal branch is still not fully cancellation-safe by design, but the contract is now explicit and points to nonce/export-id replay checks as the retry boundary. This satisfies the ticket's documentation-only remediation; durable cancellation robustness remains a future design task.
- Duplicate IPv4 phase IDs now fail at genesis parse time, preventing order-dependent first-match behavior in core state.

---

## 5. Tests

Executed checks:

```text
python scripts/check_entity_name_segments.py crates/pwm-core/src/state.rs crates/pwmd/src/api/handlers_tx.rs crates/pwm-cli/src/signer.rs crates/pwm-cli/src/bin/claim_ipv4_batch.rs crates/pwmd/src/snapshot/genesis.rs crates/pwm-tui/src/tui_loop.rs crates/pwm-tui/src/tx_submit.rs
cargo test -p pwm-core --lib claim_ipv4_touch_marks
cargo test -p pwm-core --lib export_touch_marks
cargo test -p pwm-core --lib import_touch_marks
cargo test -p pwm-core --lib policy_deferred_deact
cargo test -p pwmd --lib gen_ipv4_phases
cargo test -p pwm-cli --bin claim-ipv4-batch claim_keys
cargo test -p pwm-tui --lib f5_retired_claim_no_submit
cargo check -p pwm-cli --bin pwm --bin claim-ipv4-batch
cargo check -p pwm-tui
```

Results:

- Naming policy: PASS, no violations.
- `claim_ipv4_touch_marks`: PASS.
- `export_touch_marks`: PASS.
- `import_touch_marks`: PASS.
- `policy_deferred_deact*`: PASS, 3 tests.
- `gen_ipv4_phases*`: PASS, 2 tests.
- `claim_keys*`: PASS, 3 tests.
- `f5_retired_claim_no_submit`: PASS.
- `cargo check -p pwm-cli --bin pwm --bin claim-ipv4-batch`: PASS.
- `cargo check -p pwm-tui`: PASS.

Note: one initial attempt used multiple cargo test filters in a single command, which Cargo rejects (`unexpected argument 'export_touch_marks'`). The targeted tests were rerun individually and passed.

---

## 6. Verdict

**APPROVE** — WARN-001..005 are closed by the reviewed changes.

No blocking regressions found. The direct-seal path remains a documented cancellation-safety limitation, not a behavior fix; that matches the ticket scope. Follow-up hardening could later turn the comment into a durable/idempotent background workflow, but it is not required for this review.

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260528-v5-audit-warn-fixes-review.md
token_usage:
  source: estimate
  input: 24000
  output: 3200
  total: 27200
  confidence: medium
```

---

## 8. Testing gate (pwm-testing)

**Result:** `PASS` (ticket `20260528-v5-audit-warn-fixes-testing`, worker `pwm-testing_65896`).

- `preflight_target_debug.ps1`, `cargo fmt --check`, all targeted unit tests, `cargo check --workspace`: PASS.
- Live smoke: `scripts/devnet_v5_operator_smoke.ps1 -Ipv4ClaimOnly -CleanState` — PASS (`PASS_EVIDENCE` slice=ipv4_claim, phase=7, balance delta 1_000_000).
- Evidence: `tmp/devnet_v5_operator_smoke_20260528_212259.md` (local, not committed).

**Pipeline:** coding (WARN×5 + HIGH×2) → review APPROVE → testing PASS — **closed**.

---

## 9. Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'P:\opt\docker\PWM-cryptocurrency'
git add 'docs/reviews/20260528-v5-audit-warn-fixes-review.md'
git add 'tasks/done/20260528-v5-audit-warn-fixes-review.json'
git commit -m 'docs(v5-audit): review warn fix closure'
```
