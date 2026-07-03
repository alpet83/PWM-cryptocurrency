# Review: offchain batch store caps and ensure_ready (04d69fc)

- **date:** 2026-07-03
- **ticket:** `20260703-offchain-batch-bounds-review`
- **coding_ticket:** `20260703-offchain-batch-bounds`
- **commit:** `04d69fcd8ca31c8a26d9826801c49f41c37e21bc`
- **agent:** `pwm-review` (`pwm_review`)
- **scope:** `crates/pwmd/src/api/handlers_offchain.rs`, `crates/pwmd/src/offchain.rs`, `crates/pwmd/src/tests/http_status.rs`

---

## 1. Scope recap

Coding ticket bounds previously unbounded in-memory offchain batch storage (flagged in [`20260703-rpc-account-api-scope.md`](20260703-rpc-account-api-scope.md) as unauthenticated `POST /v1/offchain/batch` without `ensure_ready`). Changes:

1. **`MAX_BATCH_ENTRIES = 4096`** — per-request entry cap → **413** when exceeded.
2. **`MAX_BATCHES = 1024`** in `OffchainStore` — evict lowest `batch_id` when full before insert.
3. **`ensure_ready`** on `POST /v1/offchain/batch` only.

---

## 2. Focus-area verification

| # | Focus | Verdict | Evidence |
|---|-------|---------|----------|
| 1 | `MAX_BATCH_ENTRIES` + 413 for oversized batches | **PASS** | `handlers_offchain.rs:11`, `:21–25` returns `StatusCode::PAYLOAD_TOO_LARGE`. Test `v1_off_batch_413` posts 4097 entries → 413 (`http_status.rs:104–117`). |
| 2 | `MAX_BATCHES` store cap + oldest `batch_id` eviction | **PASS** | `offchain.rs:14`, `:70–74` — when `batches.len() >= MAX_BATCHES`, remove `batches.keys().min()` (monotonic ids ⇒ oldest). Unit test `store_evicts_oldest` inserts 1025 batches: len stays 1024, `get(1)` none, `get(2)` some, `get(1025)` some (`:213–221`). |
| 3 | `ensure_ready` at top of `v1_off_batch` | **PASS** | First handler statement `ensure_ready(&app).await?` (`handlers_offchain.rs:17`). Test `v1_off_batch_ready_gate` with `InitState::loading` → 503 (`http_status.rs:122–138`). |
| 4 | GET endpoints unaffected (no auth, no ready gate) | **PASS** | `v1_off_batch_get` (`:40–48`) and `v1_off_proof` (`:51–79`) have no `ensure_operator_auth` or `ensure_ready`; read from `app.offchain.get` only. Router unchanged for GET routes (`router.rs:35–39`). |

---

## 3. Bounds summary

| Limit | Value | Enforcement |
|-------|-------|-------------|
| Entries per POST | 4096 | Handler pre-check → 413 |
| Batches in store | 1024 | `OffchainStore::insert` evicts min `batch_id` |
| Global HTTP body | 256 KB | Existing `DefaultBodyLimit` (`router.rs:76`) |

Worst-case memory is now **finite** (≤1024 batches × ≤4096 entries each), though still large — acceptable bounded store vs prior unbounded growth.

### Wire JSON / u128

Wire JSON / u128: not applicable (local offchain Merkle store; amounts parsed as decimal strings in handler, no peer wire change).

---

## 4. Safety

| Risk | Assessment |
|------|------------|
| Unbounded batch count DoS | **Mitigated** — 1024 cap + eviction. |
| Huge single batch DoS | **Mitigated** — 4096 entry cap + 413. |
| Offchain POST while node loading | **Mitigated** — `ensure_ready` → 503. |
| Unauthenticated GET disclosure | **Unchanged** — still no auth on GET (pre-existing; out of slice). |

---

## 5. Tests

- `v1_off_batch_413` — HTTP 413 at 4097 entries
- `v1_off_batch_ready_gate` — 503 when not ready
- `store_evicts_oldest` — unit eviction semantics

**Gaps (non-blocking):** no HTTP test for GET after POST; no integration test exercising store eviction via API; entry cap runs after axum `Json` deserialize (body still parsed for 4097-element array within global body limit).

---

## 6. Concurrency / parallelism

`OffchainStore` uses `Mutex<HashMap<...>>`; `insert` holds lock for eviction + insert — short critical section. No `.await` under store mutex. **No new async lock hazards.**

---

## 7. BLOCKERs

None.

---

## 8. Nits (non-blocking)

1. **NIT-1:** Document `MAX_BATCH_ENTRIES` / `MAX_BATCHES` in API or operator docs (cap values are reasonable but magic constants).
2. **NIT-2:** Consider rejecting oversize entry count before full JSON array materialization if large batches become a CPU concern (mitigated today by 256 KB body cap).
3. **NIT-3:** Add HTTP test that POST 1025 batches evicts oldest and GET returns 404 for evicted id.

---

## 9. Verdict

**Approve** — per-batch and store-level caps are implemented with correct eviction semantics; `ensure_ready` gates POST; GET handlers remain unauthenticated and without ready gate. Store is no longer unbounded.

---

## 10. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260703-offchain-batch-bounds-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 10000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260703-offchain-batch-bounds-review.md'
git commit -m 'docs(v7): offchain batch bounds and ensure_ready review (04d69fc)'
```