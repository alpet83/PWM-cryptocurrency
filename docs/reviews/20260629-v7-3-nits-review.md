# Review: V7-3 nits — fee_pwm field + test coverage (93606b5)

- date: 2026-06-29
- ticket: `20260629-v7-3-nits-review`
- coding_ticket: `20260629-v7-3-nits`
- prior review: `docs/reviews/20260629-v7-3-tui-conservation-pending-review.md` (`91826f1`)
- commit: `93606b55714c091ccfa767f0f9a226a05941f347`
- scope: `api/types.rs`, `api/handlers_account.rs`, `tests/http_status.rs`, `pwm-tui/tui_loop.rs`

## 1. Scope recap

Closes three nits from V7-3 parent review:

| Nit | Delivery |
|-----|----------|
| NIT-1 | `pending_cons_api_shape` HTTP test |
| NIT-2 | `fee_pwm: String` on `PendingConservationOut` |
| NIT-3 | `conservation_pending_txt` extractor + `conservation_pending_line` unit test |

`pwmd` version `0.1.78` → `0.1.79`. Additive only; no wire/snapshot/tx path changes.

## 2. Requirements fit

| Focus area | Verdict | Evidence |
|------------|---------|----------|
| 1. `pending_cons_api_shape` cross-account isolation | **PASS** | Seeds row for `sender` only (`http_status.rs:342-354`); asserts A has 1 row with full shape (`:369-378`); B absent or empty (`:391-396`) |
| 2. `fee_pwm` mapping | **PASS** | `row.fee_pwm.to_string()` (`handlers_account.rs:23`); core field is `u64` (`state.rs:131`); test expects `"4"` (`http_status.rs:375`) |
| 3. TUI formatter + unit test | **PASS** | `conservation_pending_txt` (`tui_loop.rs:930-939`); test builds 2 rows, asserts min height 7 (`:1681-1700`); mirrors `marks_hour_left` / `marks_hour_hint_gate` pattern |
| 4. Version `0.1.79` | **PASS** | New JSON field on `PendingConservationOut` — contract extension warrants patch bump |
| 5. Pre-existing PARTIAL issues | **PASS** (not introduced) | Diff scope is API types/handlers + one test + TUI formatter; no fmt churn, no seal/event path — `v1_tx_event_sealed` flake unchanged |
| 6. Build / focused tests | **UNVERIFIED** (shell I/O) | Ticket reports PASS; static review confirms test targets exist and compile by structure |

## 3. Change-by-change analysis

### NIT-1 — `pending_cons_api_shape`

```319:396:crates/pwmd/src/tests/http_status.rs
async fn pending_cons_api_shape() {
    // ... seed sender + other accounts; push PendingConservationTransfer { sender, ... fee_pwm: 4, ... }
    // GET /v1/account/{sender} -> pending.len() == 1, all fields including fee_pwm
    // GET /v1/account/{other} -> pending_conservation absent OR empty array
}
```

- Isolation logic matches handler filter `row.sender == *key`.
- `skip_serializing_if = "Vec::is_empty"` on `AcctOut` explains `is_none()` branch for account B — correct.
- **Minor gap (non-blocking):** test uses single-account endpoint only; list `/v1/accounts` not exercised (handler shares same helper).

### NIT-2 — `fee_pwm`

```469:476:crates/pwmd/src/api/types.rs
pub struct PendingConservationOut {
    pub recipient: String,
    pub amount_pwm: String,
    pub fee_pwm: String,
    // ...
}
```

```20:27:crates/pwmd/src/api/handlers_account.rs
            fee_pwm: row.fee_pwm.to_string(),
```

- Direct `u64` → decimal string; no scaling error (fee stored as `u64` in pending row, applied as `u128::from(row.fee_pwm)` on execute — consistent).
- TUI `PendingConservationRow` does not parse `fee_pwm` — acceptable; display slice does not show fee.

### NIT-3 — TUI formatter

```930:939:crates/pwm-tui/src/tui_loop.rs
fn conservation_pending_txt(row: &AcctRow) -> Option<String> {
    let next_h = row.pending_conservation.iter().map(|p| p.execute_at_height).min()?;
    Some(format!(
        "conservation pending: {} transfer(s), next execute at height {next_h}",
        row.pending_conservation.len()
    ))
}
```

- Empty vec → `min()` is `None` → whole function `None` — detail line omitted (same as pre-refactor inline logic).
- Unit test uses two rows with execute heights 9 and 7 → asserts `"height 7"` and `"2 transfer(s)"`.
- Read-only; no tx submission introduced.

## 4. Style and module shape

- `conservation_pending_txt` — 3-word `snake_case` fn name; extracted helper improves `tui_loop` readability.
- Test names descriptive and within test naming budget.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice). HTTP `fee_pwm` as decimal string matches `amount_pwm` pattern.

## 5. Safety

- No new mutation paths; tests seed state under write lock in test harness only.
- No cross-account leakage in handler or test assertions.

## 6. Tests

| test | covers |
|------|--------|
| `pending_cons_api_shape` | JSON shape + fee_pwm + A/B isolation |
| `conservation_pending_line` | formatter min-height + count |

Parent review nits **NIT-1–3 closed**. Optional follow-up: empty-row case in `conservation_pending_line` (assert `None`) — not required.

## 7. Concurrency / parallelism

Not in diff scope (spot-check only: no new shared-state surfaces observed).

## 8. BLOCKERs

None.

## 9. Nits (non-blocking)

1. **NIT-1:** Extend `pending_cons_api_shape` to also hit `GET /v1/accounts` and assert per-row isolation in list response.
2. **NIT-2:** Add `conservation_pending_txt(&empty_row).is_none()` assertion in unit test.

## 10. Verdict

**Approve** — all three parent-review nits implemented correctly; `fee_pwm` mapping sound; cross-account isolation proven in test; version bump justified. Pre-existing fmt / `v1_tx_event_sealed` issues not introduced by this commit.

## 11. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260629-v7-3-nits-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 28000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260629-v7-3-nits-review.md'
git commit -m 'docs(v7-3): nits fee_pwm and test coverage review (93606b5)'
```