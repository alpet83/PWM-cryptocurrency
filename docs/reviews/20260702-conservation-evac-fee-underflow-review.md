# Review: emergency evac fee underflow fix (8fbc226)

- **date:** 2026-07-02
- **ticket:** `20260702-conservation-evac-fee-underflow-review`
- **coding_ticket:** `20260702-conservation-evac-fee-underflow-fix`
- **commit:** `8fbc22681a2c54eee87b4b077748a6026d6f76b8`
- **agent:** `pwm-review` (`pwm_review`)
- **scope:** `crates/pwm-core/src/state.rs` `TxBody::Policy` evac arm, workspace `Cargo.toml` `[profile.release]`, regression test `conservation_emergency_evac_with_fee`

---

## 1. Scope recap

Critical security fix for emergency evacuation fee underflow in the `TxBody::Policy` arm. Before the fix, fee subtraction occurred after evacuation zeroed `balance_pwm`, yielding `0 - fee` (debug panic or release wrap → supply inflation). Fix: debit fee via `checked_sub` before moving remaining balance/stake to rescue target; enable `overflow-checks = true` in workspace release profile; add `conservation_emergency_evac_with_fee` regression test.

---

## 2. Focus-area verification

| # | Focus | Verdict | Evidence |
|---|-------|---------|----------|
| 1 | Fee-first ordering before evac | **PASS** | `apply_policy_action` then `checked_sub(*fee)` (`state.rs:707–711`), then `fee_pool` credit (`:712`), then `amount = a.balance_pwm` and rescue credit (`:715–724`). Remaining balance after fee is what evacuates. |
| 2 | `checked_sub` / `Insufficient` / clean `Err` | **PASS** | `ok_or(TxError::Insufficient)?` at `:710–711` returns before `pending_conservation.retain`, rescue mutation, `fee_pool` update, or `accounts.insert` (`:733`). In-memory `a` may reflect `apply_policy_action` changes, but `self` ledger unchanged on `Err`. |
| 3 | `overflow-checks = true` placement | **PASS** | Workspace root `Cargo.toml:18–20` under `[profile.release]`; no crate-local `[profile.*]` overrides in `crates/*/Cargo.toml`. Applies to all workspace members (`pwm-core`, `pwmd`, `pwm-cli`, `pwm-tui`). |
| 4 | `conservation_emergency_evac_with_fee` | **PASS** with nit | Test at `state.rs:4009–4086`: balance 1000, fee 5, emergency redirect with rescue cosign. Asserts `owner.balance_pwm == 0`, `rescue.balance_pwm == rescue_before + 995`, `fee_pool == pool_before + 5`. **Nit:** no negative case (`fee > balance` → `Insufficient`) and no stake+fee evac assertion. |
| 5 | `conservation_emergency_cancels_pending` (fee=0) | **PASS** (static) | Test unchanged at `:4634–4702`, still uses `fee: 0`; compatible with reordered fee block (fee=0 `checked_sub` is no-op before evac). `cargo test` not re-run in reviewer environment. |
| 6 | No `pending_conservation` mutation on `Err` | **PASS** | `retain` at `:714` is after successful `checked_sub`; failed fee debit returns before any queue change. |

---

## 3. Correctness trace (happy path)

For emergency evac with balance `B`, fee `F`, stake `S`:

1. `apply_policy_action` activates policy, sets `finalized` for emergency redirect.
2. `balance_pwm ← B - F` (checked); `fee_pool += F`.
3. `pending_conservation` rows for sender removed.
4. Rescue receives `(B - F)` liquid + `S` staked; sender liquid zeroed.

Regression test confirms `B=1000`, `F=5` → rescue `+995`, sender `0`, `fee_pool +5`.

**Prior bug (confirmed by code structure):** evac moved full `B` first, then subtracted `F` from zeroed balance — matches ticket brief and `issues-report.md` entry.

---

## 4. Style and module shape

Minimal, idiomatic fix: `checked_sub` + early `?`, fee block hoisted before evac transfer. No new identifiers beyond test name `conservation_emergency_evac_with_fee` (5 segments, test budget).

### Wire JSON / u128

Wire JSON / u128: not applicable (ledger apply path only; no peer wire changes in this slice).

---

## 5. Safety

| Risk | Assessment |
|------|------------|
| Supply inflation via u128 wrap | **Mitigated** — fee debited before zeroing; release `overflow-checks = true` adds belt-and-suspenders for other arithmetic. |
| Mid-seal panic (debug) | **Mitigated** — no subtract-after-zero path. |
| Partial evac on failure | **None observed** — `Err` before any `self` mutation of accounts, `fee_pool`, or `pending_conservation`. |
| `fee_pool` `saturating_add` | Pre-existing; not introduced by this fix (info only). |

---

## 6. Tests

| Test | Role |
|------|------|
| `conservation_emergency_evac_with_fee` | **New** — fee>0 happy path, supply conservation |
| `conservation_emergency_cancels_pending` | **Regression** — fee=0 + pending cancel |
| `emergency_activation_sweep_ok` (near `:3977`) | fee=0 evac with balance+stake |
| `emergency_activation_fee_reject` | fee reject for non-emergency policy (`PolicyActivationFeeMustBeZero`) — not Insufficient on emergency |

**Gaps (non-blocking):**

- No test that `fee > balance` on emergency redirect returns `Insufficient` with unchanged `pending_conservation` and balances.
- No test that `fee > 0` with pending conservation queued verifies cancel only on success.

**Verification note:** `cargo test -p pwm-core conservation_emergency --lib` could not be executed in reviewer WSL session (shell I/O error). Coding ticket reports PASS on conservation tests.

---

## 7. Concurrency / parallelism

Single-threaded `apply_tx_impl` on seal path; no new `Arc`/async surfaces. Err path does not leave partial global mutations. **No hazards found.**

---

## 8. BLOCKERs

None.

---

## 9. Nits (non-blocking)

1. **NIT-1:** Add `conservation_emergency_evac_fee_insufficient` — enqueue pending conservation, submit emergency with `fee > balance`, assert `Insufficient`, pending row count unchanged, balances unchanged.
2. **NIT-2:** Document normative fee policy: ADR 0011 describes fee-free emergency activation; code allows `fee > 0` and now handles it correctly — clarify whether nonzero fee is intentional or should be rejected at `validate_pol_action`.

---

## 10. Verdict

**Approve** — fix correctly reorders fee debit before evacuation, propagates `Insufficient` without partial state mutation, enables workspace-wide release overflow checks, and adds a targeted regression test. Resolves critical supply-inflation / seal-panic vector. Nits are follow-up test/doc hardening only.

---

## 11. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260702-conservation-evac-fee-underflow-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 18000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260702-conservation-evac-fee-underflow-review.md'
git commit -m 'docs(v7-8): emergency evac fee underflow fix review (8fbc226)'
```