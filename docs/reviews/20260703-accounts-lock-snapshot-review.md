# Review: `/v1/accounts` read-lock snapshot fix (9905187)

- **date:** 2026-07-03
- **ticket:** `20260703-accounts-lock-snapshot-review`
- **coding_ticket:** `20260703-accounts-lock-snapshot`
- **commit:** `9905187e8e3dd9cae60345b96bd225fe941b8ef1`
- **agent:** `pwm-review` (`pwm_review`)
- **scope:** `crates/pwmd/src/api/handlers_account.rs` — `v1_accounts` refactor

---

## 1. Scope recap

Coding ticket fixed seal-starvation risk: `v1_accounts` previously held `inner.read()` across `foreign_home_lookup_state(...).await` in the per-account loop. Tokio `RwLock` is write-preferring; prolonged read guards block the seal path’s `write()`. Fix: snapshot owned data under read guard, drop guard, then run async foreign lookups.

---

## 2. Focus-area verification

| # | Focus | Verdict | Evidence |
|---|-------|---------|----------|
| 1 | `inner.read()` dropped before any `.await` in `v1_accounts` | **PASS** | Read guard scoped to block `handlers_account.rs:60–63`; loop with `foreign_home_lookup_state(...).await` at `:65–89` runs after guard drop. `account_snapshots` is synchronous (`:40–53`). |
| 2 | Snapshot captures all `AcctListOut` fields | **PASS** | `AccountSnapshot` holds `id`, `account` (full `Account` clone), `peer_view` (`Option<PeerAccountView>`), `pending` (`Vec<PendingConservationOut>`). `acct_out_for_runtime` needs only `Account` + `peer_view` + `home_lookup_state`; `pending_conservation` assigned at `:87`. |
| 3 | `foreign_home_lookup_state` still correct post-lock | **PASS** | Same predicate `home_hi == cluster_domain_hi` (`:68–69`); foreign branch passes `&a`, `home_hi`, `peer_view.is_some()`, `source_node_id`, `now_ms` (`:71–77`) — identical inputs to pre-refactor logic. |
| 4 | Local-home accounts skip unnecessary async | **PASS** | Local accounts use `HomeLookupState::Ok` directly (`:68–69`); no `await` on that branch. |

---

## 3. Correctness notes

**Lock lifetime**

```60:63:crates/pwmd/src/api/handlers_account.rs
    let snapshots = {
        let g = a.inner.read().await;
        account_snapshots(&g)
    };
```

Guard dropped at end of block before loop — satisfies ticket requirement.

**Snapshot helper**

```40:53:crates/pwmd/src/api/handlers_account.rs
fn account_snapshots(inner: &Inner) -> Vec<AccountSnapshot> {
    inner
        .chain
        .st
        .accounts
        .iter()
        .map(|(id, account)| AccountSnapshot {
            id: *id,
            account: account.clone(),
            peer_view: inner.peer_account_views.get(id).cloned(),
            pending: pending_conservation_out(&inner.chain.st, id),
        })
        .collect()
}
```

Clones `BTreeMap` accounts and peer views — consistent point-in-time snapshot under one read guard.

**Staleness trade-off:** Foreign lookup runs on snapshot data; peer view / balances may age during multi-account `await` loop. Acceptable for list RPC (same as prior behavior except lock duration).

### Wire JSON / u128

Wire JSON / u128: not applicable (handler refactor only; no wire schema change).

---

## 4. Safety

| Risk | Assessment |
|------|------------|
| Seal starvation via `/v1/accounts` | **Mitigated** for list endpoint — read lock no longer spans handshake/transport `await`. |
| Missing fields in response | **None** — snapshot preserves prior field coverage. |
| Deadlock | **None introduced** — shorter read hold time reduces contention only. |

---

## 5. Tests

No new automated test for lock ordering. Coding ticket verified `cargo check -p pwmd`. **Gap (nit):** no regression test asserting `v1_accounts` does not hold `inner` read guard across foreign lookup (would need instrumentation or doc test).

---

## 6. Concurrency / parallelism

- **Before:** O(accounts) × `foreign_home_lookup_state` latency under one read guard → seal `write()` blocked.
- **After:** Read guard ≈ clone cost only; async work outside guard. Seal path can acquire `write()` between foreign lookups.
- **Residual:** `v1_account` (`:102–119`) still holds `inner.read()` across `foreign_home_lookup_state(...).await` — **same class of bug, out of coding ticket scope**.

---

## 7. BLOCKERs

None.

---

## 8. Nits (non-blocking)

1. **NIT-1:** Apply same snapshot pattern to `v1_account` (`handlers_account.rs:93–130`) for consistency and single-account DoS parity.
2. **NIT-2:** Consider `Vec::with_capacity` in `account_snapshots` from `inner.chain.st.accounts.len()` (micro-optimization).
3. **NIT-3:** Document snapshot staleness semantics for foreign accounts in API comments if clients assume strict freshness.

---

## 9. Verdict

**Approve** — `v1_accounts` correctly snapshots account, peer view, and `pending_conservation` under a short-lived read guard and performs all `foreign_home_lookup_state` awaits after lock release. Local accounts avoid async lookup. Fix is complete for the stated scope.

---

## 10. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260703-accounts-lock-snapshot-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 12000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260703-accounts-lock-snapshot-review.md'
git commit -m 'docs(v7): v1_accounts read-lock snapshot fix review (9905187)'
```