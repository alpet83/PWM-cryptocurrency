# Review: snapshot-before-await in `v1_account` (823ea36)

- **date:** 2026-07-03
- **ticket:** `20260703-account-single-lock-review`
- **coding_ticket:** `20260703-account-single-lock`
- **commit:** `823ea36aaad4742c006285a9c2bb959a7f5c2984`
- **agent:** `pwm-review` (`pwm_review`)
- **scope:** `crates/pwmd/src/api/handlers_account.rs` — `v1_account`, `account_snapshot`

---

## 1. Scope recap

Coding ticket closes the residual finding from [`20260703-accounts-lock-snapshot-review.md`](20260703-accounts-lock-snapshot-review.md) §6: `GET /v1/account/:id` previously held `inner.read()` across `foreign_home_lookup_state(...).await`, blocking seal `write()` on Tokio’s write-preferring `RwLock`. Fix mirrors `v1_accounts`: snapshot under a short read guard, drop guard, then async foreign lookup.

---

## 2. Focus-area verification

| # | Focus | Verdict | Evidence |
|---|-------|---------|----------|
| 1 | `inner.read()` dropped before any `.await` in `v1_account` | **PASS** | Read guard scoped to block `handlers_account.rs:110–113`; `foreign_home_lookup_state(...).await` at `:121–128` runs after guard drop. Only pre-snapshot `.await` is `ensure_ready` (`:107`), which does not hold `inner.read()`. |
| 2 | Snapshot captures Account, PeerView, pending_conservation | **PASS** | `account_snapshot` (`:55–63`) clones `Account`, `peer_view` from `peer_account_views`, and `pending` via `pending_conservation_out`. Output assigns `out.pending_conservation = item.pending` (`:137`). `acct_out_for_runtime` receives `&item.account` and `peer_view` (`:130–136`). |
| 3 | `foreign_home_lookup_state` correct after lock drop | **PASS** | Same predicate `home_hi == cluster_domain_hi` (`:118–119`); foreign branch passes `&a`, `home_hi`, `peer_view.is_some()`, `source_node_id`, `now_ms` (`:121–127`) — identical to `v1_accounts` loop (`:76–88`). |
| 4 | 404 + local path without unnecessary async | **PASS** | Missing account: `account_snapshot` returns `None` → `NOT_FOUND` (`:114`). Local home: `HomeLookupState::Ok` branch (`:118–119`) — no `await`. Invalid id still `BAD_REQUEST` via `parse_id` (`:108–109`). |

---

## 3. Correctness notes

**Lock lifetime**

```110:113:crates/pwmd/src/api/handlers_account.rs
    let item = {
        let g = a.inner.read().await;
        account_snapshot(&g, key)
    }
```

Guard dropped at block end before `home_hi` / foreign lookup logic.

**Shared snapshot helper**

```55:63:crates/pwmd/src/api/handlers_account.rs
fn account_snapshot(inner: &Inner, id: AccountId) -> Option<AccountSnapshot> {
    let account = inner.chain.st.get(&id)?.clone();
    Some(AccountSnapshot {
        id,
        account,
        peer_view: inner.peer_account_views.get(&id).cloned(),
        pending: pending_conservation_out(&inner.chain.st, &id),
    })
}
```

Parallels `account_snapshots` field coverage for a single key; point-in-time under one read guard.

**Staleness trade-off:** Same as list endpoint — foreign lookup uses snapshot peer view; acceptable for read RPC.

### Wire JSON / u128

Wire JSON / u128: not applicable (handler concurrency refactor only).

---

## 4. Safety

| Risk | Assessment |
|------|------------|
| Seal starvation via `/v1/account/:id` | **Mitigated** — read lock no longer spans handshake/transport `await`. |
| Missing response fields | **None** — snapshot preserves prior `AcctOut` coverage including `pending_conservation`. |
| Wrong 404 semantics | **Preserved** — lookup under read guard, 404 before any foreign async work. |

---

## 5. Tests

No new lock-ordering test. Existing `http_status.rs` coverage exercises `v1_account` for local/foreign balance shape (`:394–431`) and `pending_conservation` (`:472–509`). Coding ticket ran `cargo check -p pwmd` per companion log.

**Gap (nit):** no explicit `GET /v1/account/:missing` → 404 test; no instrumentation test for lock span.

---

## 6. Concurrency / parallelism

- **Before (9905187 era):** `v1_accounts` fixed; `v1_account` still held read guard across `foreign_home_lookup_state` → single-account query could starve seal.
- **After:** Both list and single-account paths snapshot synchronously, then await outside `inner.read()`. Seal `write()` can proceed during foreign handshake work.
- **Hazards introduced:** None. `foreign_home_lookup_state` still uses `handshake_read_traced` / `transport_config.read()` independently of account snapshot lock.

---

## 7. BLOCKERs

None.

---

## 8. Nits (non-blocking)

1. **NIT-1:** Add HTTP test for unknown account id → 404 on `/v1/account/:id`.
2. **NIT-2:** Consider deduplicating `account_snapshot` body via `account_snapshots` filter (micro-DRY; current split is clear).
3. **NIT-3:** Update [`20260703-rpc-account-api-scope.md`](20260703-rpc-account-api-scope.md) attack-surface row for `v1_account` lock contention (still listed as residual risk).

---

## 9. Verdict

**Approve** — `v1_account` correctly snapshots account, peer view, and `pending_conservation` under a short-lived read guard; `foreign_home_lookup_state` runs only after lock release; local accounts skip async lookup; not-found returns 404. Matches `v1_accounts` pattern and ticket acceptance criteria.

---

## 10. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260703-account-single-lock-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 10000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260703-account-single-lock-review.md'
git commit -m 'docs(v7): v1_account snapshot-before-await review (823ea36)'
```