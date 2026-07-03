# Review: RPC IP allowlist middleware (59e0cc0)

- **date:** 2026-07-03
- **ticket:** `20260703-rpc-ip-allowlist-review`
- **coding_ticket:** `20260703-rpc-ip-allowlist`
- **commit:** `59e0cc0e08a731b90e4db748ef14d59c98d8ae96`
- **agent:** `pwm-review` (`pwm_review`)
- **scope:** `crates/pwmd/src/rpc_allow.rs`, `crates/pwmd/src/api/router.rs`, `crates/pwmd/src/config.rs`, `crates/pwmd/src/main.rs`, `crates/pwmd/src/tests/http_status.rs`

---

## 1. Scope recap

Coding ticket adds optional RPC source-IP gating for all `/v1/*` routes: static CIDR list (`rpc_allowed_ips`) plus startup auto-enrollment window (`rpc_allowed_auto` seconds). Addresses ops risk from [`20260703-rpc-account-api-scope.md`](20260703-rpc-account-api-scope.md) (no per-IP throttle). Default config disables middleware (backward compatible). `pwmd` version bumped to `0.1.84`.

---

## 2. Focus-area verification

| # | Focus | Verdict | Evidence |
|---|-------|---------|----------|
| 1 | CIDR parsing; bare IP → /32 or /128 | **PASS** | `parse_rpc_cidr` (`rpc_allow.rs:114–134`): optional `/prefix`; missing prefix uses `max` 32 (v4) or 128 (v6). `prefix_match` handles full and partial bytes (`:144–155`). Tests: `192.168.1.0/24` allows `.44`, rejects `10.0.0.9` (`http_status.rs:23–48`). |
| 2 | `rpc_allowed_auto` is `u16` | **PASS** | `PwmdConfig.rpc_allowed_auto: u16` (`config.rs:126`); CLI `default_value_t = 0` (`main.rs:69–71`); `from_cfg(..., auto_secs: u16)` (`rpc_allow.rs:43`). No `u64` in config path. |
| 3 | Middleware only when config enables | **PASS** | `RpcAllowState::disabled()` when `cidrs.is_empty() && auto_until.is_none()` (`:61–63`). Router skips `rpc_ip_gate` layer when `disabled()` (`router.rs:78–85`). Default config empty + 0 (`config.rs:340–341`). |
| 4 | Auto-enroll sticks; no enroll after window | **PASS** | `ip_allowed`: within `auto_until`, inserts into `dynamic` `HashSet` (`:72–77`); after expiry only static CIDR + prior `dynamic` entries pass. Test `rpc_ip_auto_sticks`: enroll `10.0.0.9`, close window → still OK; fresh closed window + `10.0.0.10` → 403 (`http_status.rs:53–79`). |
| 5 | Non-allowlisted → 403; loopback not hardcoded | **PASS** | Rejection `StatusCode::FORBIDDEN` (`rpc_allow.rs:109–110`). No `is_loopback()` bypass in `rpc_allow.rs`. Loopback must match static CIDR or auto-enroll during window. Static test rejects non-CIDR IP. |
| 6 | INFO on auto-enroll, WARN on rejection | **PASS** | `info!(%src, "rpc IP auto-enrolled")` on first insert (`:75`); `warn!(%src, "rpc request rejected by IP allowlist")` (`:109`); missing `ConnectInfo` warns (`:102`). |

---

## 3. Middleware flow

```text
/v1/* request
  → rpc_ip_gate (only if !rpc_allow.disabled())
       ConnectInfo missing + gate enabled → 403 + warn
       ip_allowed(src)?
         static CIDR match → pass
         dynamic HashSet → pass
         auto window open → enroll + info + pass
         else → 403 + warn
  → handler
```

`from_cfg` wired at startup in `lifecycle.rs:2486–2489`. Operates on peer IP from `ConnectInfo` (requires `into_make_service_with_connect_info` in production).

### Wire JSON / u128

Wire JSON / u128: not applicable (HTTP admission middleware only).

---

## 4. Safety

| Risk | Assessment |
|------|------------|
| Remote RPC exposure on public bind | **Mitigated when enabled** — static + sticky dynamic allowlist. |
| Accidental lockout at deploy | **Documented** — `issues-report.md` rollout note; `rpc_allowed_auto` window provided. |
| Default behavior regression | **None** — empty config = no middleware layer. |
| Missing peer address | **Fail-closed** when gate enabled (403). |

**Residual:** Operator can configure `0.0.0.0/0` and effectively disable IP filtering — config hazard, not code bypass. Per-route exemptions none (all `/v1/*` gated equally when enabled).

---

## 5. Tests

- `rpc_ip_static_rejects` / `rpc_ip_static_allows`
- `rpc_ip_auto_sticks` (sticky enroll + post-window reject)

**Gaps (non-blocking):** no unit test for bare `192.168.1.44` (/32 default); no IPv6 CIDR test; no explicit loopback-rejected-without-list HTTP test.

---

## 6. Concurrency / parallelism

`dynamic` allowlist uses `RwLock<HashSet<IpAddr>>`: read on check, write on enroll. Short critical sections; no lock held across handler `next.run`. **No deadlock or seal-path interaction** (separate from `inner` RwLock).

---

## 7. BLOCKERs

None.

---

## 8. Nits (non-blocking)

1. **NIT-1:** Add unit tests for bare IPv4/IPv6 (`/32`/`/128` defaults) and partial-byte prefix edge cases.
2. **NIT-2:** Add HTTP test that `127.0.0.1` is 403 when not in static list and auto window closed.
3. **NIT-3:** Consider metric/counter for rejections alongside WARN logs for ops dashboards.

---

## 9. Verdict

**Approve** — CIDR parsing and matching are correct; `rpc_allowed_auto` is `u16`; middleware is conditional on non-empty static list or positive auto window; auto-enrollment semantics match ticket (sticky + no post-window enroll); rejections return 403 with WARN, enrollments log INFO. Default-off preserves backward compatibility.

---

## 10. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260703-rpc-ip-allowlist-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 11000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260703-rpc-ip-allowlist-review.md'
git commit -m 'docs(v7): RPC IP allowlist middleware review (59e0cc0)'
```