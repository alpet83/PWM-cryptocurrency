# Review: operator auth gate for shutdown and bridge-reset (f91c477)

- **date:** 2026-07-03
- **ticket:** `20260703-admin-rpc-auth-review`
- **coding_ticket:** `20260703-admin-rpc-auth`
- **commit:** `f91c47704ba688fbe90ed8115555c8e186427e29`
- **agent:** `pwm-review` (`pwm_review`)
- **scope:** `handlers_shutdown.rs`, `handlers_bridge.rs`, `common.rs` (`ensure_operator_auth`), `tests/http_operator_log.rs`

---

## 1. Scope recap

Coding ticket `20260703-admin-rpc-auth` closes the **High** finding in [`20260703-rpc-account-api-scope.md`](20260703-rpc-account-api-scope.md): `POST /v1/shutdown` and `POST /v1/bridge-federation/reset` were previously unauthenticated. Both endpoints now call shared `ensure_operator_auth` (loopback **or** `Authorization: Bearer <op_token>`) before any side effects, matching `handlers_operator_log.rs` and `v1_cross_shard_backfill`.

---

## 2. Focus-area verification

| # | Focus | Verdict | Evidence |
|---|-------|---------|----------|
| 1 | `v1_shutdown` extracts `ConnectInfo<SocketAddr>` + `HeaderMap`, calls auth first | **PASS** | `handlers_shutdown.rs:128–133` — `ensure_operator_auth(&app, conn.map(\|v\| v.0), &headers)?` before `graceful_shutdown_request`. |
| 2 | `v1_bridge_federation_reset` same pattern | **PASS** | `handlers_bridge.rs:12–17` — auth before `ensure_ready` and handshake mutation. |
| 3 | Unauthenticated remote → 401/403, not 500/204 | **PASS** | `common.rs:46–49` returns `StatusCode::FORBIDDEN` (403) with stable message. Tests `admin_rpc_remote_denied` and `bridge_reset_remote_denied` assert 403 on `10.10.0.9` (`http_operator_log.rs:169–201`). No work runs on reject (no 204 leak). |
| 4 | Auth factored cleanly, not duplicated | **PASS** | Single `ensure_operator_auth` in `common.rs:26–50`; reused by operator log, backfill, shutdown, bridge reset. Handlers are one-line `?` calls — no copied bearer/loopback logic. |
| 5 | Loopback + valid token paths still work | **PASS** | Loopback: `ensure_operator_auth` returns `Ok("loopback")` when `addr.ip().is_loopback()` (`common.rs:31–32`). Token: `bearer_token` match against `app.op_token` (`:34–37`). Existing `op_log_token_allows` proves remote + bearer on shared helper (`http_operator_log.rs:205–225`). Operator log loopback tests use same `req_conn` + `127.0.0.1` pattern. |

---

## 3. Auth flow

```text
Request → Option<ConnectInfo<SocketAddr>> + HeaderMap
       → ensure_operator_auth (common.rs)
            loopback IP?  → Ok("loopback")
            Bearer == op_token? → Ok("token")
            else → Err(403, "operator endpoint requires loopback or valid bearer token")
       → handler work (shutdown persist / bridge latch clear)
```

Production serves with `into_make_service_with_connect_info::<SocketAddr>()` (`lifecycle.rs:2779`), so peer IP is available for non-loopback binds.

### Wire JSON / u128

Wire JSON / u128: not applicable (HTTP operator auth only; no peer wire or amount field changes).

---

## 4. Safety

| Risk | Assessment |
|------|------------|
| Remote unauthenticated shutdown | **Mitigated** — 403 before `graceful_shutdown_request`. |
| Remote bridge latch reset | **Mitigated** — 403 before `handshake_write_traced` mutation. |
| Missing `ConnectInfo` (fail-open) | **Fail-closed** — `remote.is_some_and(loopback)` is false when `None`; token required or 403. |
| Wrong token on remote | **Rejected** — same 403 path (`op_log_token_bad_bearer` pattern on log endpoint). |
| Auth bypass via 500 on shutdown errors | **N/A for unauth** — 403 returned before shutdown path; post-auth snapshot errors correctly map to 500. |

**Residual (out of slice):** other admin-adjacent routes (`POST /v1/offchain/batch`, lab seal) remain on separate gates per prior scope doc.

---

## 5. Tests

New regressions in `http_operator_log.rs`:

- `admin_rpc_remote_denied` — `POST /v1/shutdown` from `10.10.0.9` → 403
- `bridge_reset_remote_denied` — `POST /v1/bridge-federation/reset` from `10.10.0.9` → 403

Shared helper token/loopback coverage via existing operator-log tests.

**Gaps (non-blocking):** no dedicated loopback-success or bearer-success test for shutdown/bridge endpoints; tests live in `http_operator_log.rs` rather than admin-specific module.

---

## 6. Concurrency / parallelism

Auth check is synchronous and lock-free (reads `app.op_token` + headers only). No locks held across auth boundary. `v1_bridge_federation_reset` still takes `handshake_write_traced` write lock **after** auth — unchanged concurrency surface. **No new shared-state or `.await`-under-lock hazards introduced.**

---

## 7. BLOCKERs

None.

---

## 8. Nits (non-blocking)

1. **NIT-1:** Add `admin_rpc_token_allows` asserting remote shutdown/bridge-reset with valid bearer return 204 (mirrors `op_log_token_allows`).
2. **NIT-2:** Consider `http_admin_rpc.rs` test module for discoverability (tests currently appended to `http_operator_log.rs`).
3. **NIT-3:** Update [`20260703-rpc-account-api-scope.md`](20260703-rpc-account-api-scope.md) route table when scope doc is next revised (rows 43–44 still say “no auth” / “ensure_ready only”).

---

## 9. Verdict

**Approve** — both endpoints are gated via shared `ensure_operator_auth` before side effects; remote unauthenticated callers receive 403; loopback and bearer paths preserve prior operator-log semantics. Closes the documented unauthenticated admin RPC gap for shutdown and bridge-reset.

---

## 10. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260703-admin-rpc-auth-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 11000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260703-admin-rpc-auth-review.md'
git commit -m 'docs(v7): admin RPC auth review — shutdown and bridge-reset gates (f91c477)'
```