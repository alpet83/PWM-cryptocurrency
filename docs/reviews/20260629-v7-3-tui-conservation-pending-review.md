# Review: V7-3 pending_conservation AcctOut + TUI compact row (91826f1)

- date: 2026-06-29
- ticket: `20260629-v7-3-tui-conservation-pending-review`
- coding_ticket: `20260629-v7-3-tui-conservation-pending`
- commit: `91826f1a2204183db2d39869b48f0dc9439b1cb9`
- scope: `api/types.rs`, `api/common.rs`, `api/handlers_account.rs`, `pwm-tui` account panel
- type: additive read-only API + TUI display (no wire/snapshot/tx changes)

## 1. Scope recap

V7-3 exposes deferred conservation transfers to operators:

| layer | change |
|-------|--------|
| API DTO | `PendingConservationOut` + `AcctOut.pending_conservation` (`skip_serializing_if = "Vec::is_empty"`) |
| handlers | `v1_account` / `v1_accounts` populate from `State.pending_conservation` filtered by sender |
| TUI | Compact detail line: pending count + earliest `execute_at_height` |
| version | `pwmd` `0.1.77` → `0.1.78` |

## 2. Requirements fit

| Focus area | Verdict | Evidence |
|------------|---------|----------|
| 1. DTO vs `PendingConservationTransfer` | **PASS** with nit | Core (`state.rs:126-136`): `sender`, `recipient`, `amount_pwm`, `fee_pwm`, `nonce`, `enqueue_height`, `execute_at_height`, `tx_hash`. API (`types.rs:469-475`): omits `sender` (account-scoped), `fee_pwm`, `tx_hash` — appropriate for read-only operator view; TUI only needs recipient/amount/heights |
| 2. Filter / no cross-account leakage | **PASS** | `row.sender == *key` (`handlers_account.rs:19`); recipient exposed only within sender's row — correct |
| 3. `v1_accounts` includes field | **PASS** | TUI fetches `GET /v1/accounts` (`account_view.rs:223`); per-account filter at `:59` — list endpoint is required, not leakage |
| 4. TUI read-only + UX parity | **PASS** | Detail line only (`tui_loop.rs:747-757`); no tx submit path; pattern mirrors `marks_hour_left` accrual hint (`:744-746`) |
| 5. Version bump | **PASS** | Additive JSON field; `skip_if_empty` keeps legacy responses lean; `pwmd` patch bump justified |
| 6. Build / tests | **UNVERIFIED** (shell I/O error) | Ticket notes prior `cargo check -p pwmd -p pwm-tui` PASS and lib 510/511; static review only in this session |

## 3. Change-by-change analysis

### API DTO and population

```16:27:crates/pwmd/src/api/handlers_account.rs
fn pending_conservation_out(st: &CoreState, key: &[u8; 32]) -> Vec<PendingConservationOut> {
    st.pending_conservation
        .iter()
        .filter(|row| row.sender == *key)
        .map(|row| PendingConservationOut {
            recipient: hex::encode(row.recipient),
            amount_pwm: row.amount_pwm.to_string(),
            nonce: row.nonce,
            enqueue_height: row.enqueue_height,
            execute_at_height: row.execute_at_height,
        })
        .collect()
}
```

- `acct_out_for_runtime` defaults `pending_conservation: Vec::new()` (`common.rs:479`); handlers overwrite after build — clean separation.
- State invariant: at most one pending row per sender (`state.rs:471-472` `ConservationPendingExists`) — API list is 0 or 1 entry per conservation account.

### TUI parsing and display

- Parse path: `account_view.rs:276-294` maps JSON array → `PendingConservationRow` (recipient, amount, nonce, heights).
- Display: `tui_loop.rs:747-757` — `min(execute_at_height)` for "next execute" — correct when multiple rows ever appear.
- `marks_display.rs` only supplies marks math helpers; conservation display lives in `tui_loop` detail builder — acceptable split (ticket "parity" = same detail-line pattern, not same module).

### Version

- `crates/pwmd/Cargo.toml`: `0.1.78`
- `pwm-tui` parses JSON loosely (no typed `AcctOut` dep) — no caller breakage observed.
- `pwm-cli` has no `AcctOut` struct — unaffected.

## 4. Style and module shape

- Identifiers within policy (`pending_conservation_out`, `PendingConservationOut`).
- `amount_pwm` as decimal string matches existing `AcctOut` u128 field pattern.
- Handler helper keeps filter logic in one place — good micro-modularity.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice). HTTP `amount_pwm` uses `to_string()` on API boundary — consistent with other balance fields.

## 5. Safety

- Read-only under `inner.read().await` — no mutation.
- No new trust boundary; hex recipient is public ledger data for the queried account.
- No `unwrap` on error paths in touched handler code.

## 6. Tests

| area | status |
|------|--------|
| Existing `/v1/accounts` split tests | Unaffected (`http_status.rs:318-364` — no assertion on absent `pending_conservation`) |
| Conservation pending API field | **Missing** — no test seeds `pending_conservation` and asserts JSON shape |
| TUI conservation detail line | **Missing** — no unit test for detail string (marks has `marks_hour_hint_gate`) |

Gap is non-blocking for additive read-only slice.

## 7. Concurrency / parallelism

Components: seal loop writes `State.pending_conservation`; account handlers read under `RwLock` read guard.

- No new shared-mutable surfaces; read path is snapshot-consistent for the held guard duration.
- No locks across `.await` beyond existing `read().await` pattern in `v1_accounts` foreign lookup loop.

## 8. BLOCKERs

None. No cross-account data leakage found.

## 9. Nits (non-blocking)

1. **NIT-1:** Add `http_status` (or handler) test: seed `PendingConservationTransfer` for account A, assert A's JSON has row and account B has empty/absent `pending_conservation`.
2. **NIT-2:** Consider exposing `fee_pwm` in `PendingConservationOut` for operator parity with on-chain row (optional).
3. **NIT-3:** TUI unit test for conservation detail line when `pending_conservation` non-empty (mirror `marks_hour_hint_gate`).

## 10. Verdict

**Approve with nits** — additive API and TUI display correctly scoped; sender filter prevents leakage; list endpoint inclusion required for TUI `/v1/accounts` flow; version bump justified.

## 11. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260629-v7-3-tui-conservation-pending-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 35000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260629-v7-3-tui-conservation-pending-review.md'
git commit -m 'docs(v7-3): pending_conservation AcctOut and TUI review (91826f1)'
```