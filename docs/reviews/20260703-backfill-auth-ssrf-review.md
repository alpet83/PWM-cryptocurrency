# Review: operator auth and peer_base allowlist for backfill (409ada9)

- **date:** 2026-07-03
- **ticket:** `20260703-backfill-auth-ssrf-review`
- **coding_ticket:** `20260703-backfill-auth-ssrf`
- **commit:** `409ada920abf057ef17831827647596cf926d513`
- **agent:** `pwm-review` (`pwm_review`)
- **scope:** `crates/pwmd/src/api/handlers_backfill.rs`, `crates/pwmd/src/relay.rs` (`relay_http_bases`), `crates/pwmd/src/tests/http_export.rs`

---

## 1. Scope recap

Coding ticket hardens `POST /v1/cross-shard/backfill`, which previously accepted unauthenticated callers and optional caller-supplied `peer_base` URLs (SSRF / operator abuse vector flagged in [`20260703-rpc-account-api-scope.md`](20260703-rpc-account-api-scope.md) §4.4 / Fable 5 scenario E). Fix adds:

1. **`ensure_operator_auth`** at handler entry (loopback or `Bearer` `op_token`).
2. **`peer_base_from_input`** allowlist: caller URL must be bare `http`/`https` origin with literal IP host matching `relay::relay_http_bases(cfg)` exactly; domains and loopback rejected with **400**.

---

## 2. Focus-area verification

| # | Focus | Verdict | Evidence |
|---|-------|---------|----------|
| 1 | Operator auth before any peer fetch | **PASS** | `ensure_operator_auth` is first statement in `v1_cross_shard_backfill` (`handlers_backfill.rs:92`), before `pick_backfill_signer`, `backfill_peers`, and `select_peer_facts` HTTP. Test `v1_xsh_backfill_auth_denied` → 403 on remote (`http_export.rs:698–716`). |
| 2 | `peer_base` URL validation (scheme, host allowlist, 400 on bad input) | **PASS** | `peer_base_from_input` (`:319–346`): `http`/`https` only; path must be `/`; no query/fragment; host must parse as IPv4/IPv6 (domains rejected at `:355–357`); loopback IPs rejected (`:333–334`); port required; `SocketAddr` must be in `relay_bases` (`:340–343`) else 400 `"not in the configured relay peer set"`. Test `v1_xsh_backfill_peer_bad` → 400 (`:721–742`). |
| 3 | No SSRF path outside allowlist | **PASS** | Outbound URLs built only from (a) config-derived `relay_http_bases` when `peer_base` omitted (`backfill_peers` `:178–182`) or (b) validated allowlisted origin (`:176–177`). `fetch_peer_facts` / `trust_peer_status` append fixed paths `/v1/cross-shard/facts` and `/v1/status` (`:373–374`, `:400`) — no caller-controlled path. Domain hosts cannot reach DNS resolution stage. |
| 4 | Valid operator + trusted peer still works | **PASS** | `v1_xsh_backfill_once_ok` uses loopback operator auth + `relay_http_seeds` from config (no `peer_base`) → discovers/imports facts (`:505–658`). `v1_xsh_backfill_untrusted_skip` shows trust envelope still enforced post-allowlist (`:663–694`). |

---

## 3. Auth and fetch ordering

```text
ensure_operator_auth
  → ensure_user_tx_allowed / ensure_bridge_federation_ok
  → pick_backfill_signer (local read)
  → backfill_peers (sync allowlist)
  → trust_peer_status + fetch_peer_facts (HTTP)
  → import_backfill_facts
```

Unauthenticated remote callers receive **403** before `peer_base` parsing or `reqwest` client use.

### Wire JSON / u128

Wire JSON / u128: not applicable (operator HTTP backfill; no peer wire encoding changes).

---

## 4. Safety

| Risk | Assessment |
|------|------------|
| Unauthenticated backfill → SSRF / import injection | **Mitigated** — 403 without operator credentials. |
| Arbitrary `peer_base` to internal/metadata IPs | **Mitigated** — IP literal only; must match configured relay `SocketAddr`; loopback blocked. |
| DNS rebinding via hostname | **Mitigated** — domain hosts rejected before fetch. |
| Allowlisted peer with wrong network/genesis | **Mitigated** — `trust_peer_status` still validates `network_id` / `effective_genesis_hash` (`:394–433`). |
| Degraded/genesis-blocked side effects | **Preserved** — `ensure_user_tx_allowed` blocks before peer work (`v1_xsh_backfill_degraded_hold`, `v1_xsh_backfill_genblock_hold`). |

**Residual (out of slice):** `GET /v1/cross-shard/facts` remains read-only without operator auth (disclosure, not SSRF). Operator with valid credentials can still target any configured relay seed — intended operator power.

---

## 5. Tests

New/updated regressions in `http_export.rs`:

- `v1_xsh_backfill_auth_denied` — remote → 403
- `v1_xsh_backfill_peer_bad` — non-allowlisted `peer_base` → 400

Existing `v1_xsh_backfill_once_ok` / `v1_xsh_backfill_untrusted_skip` still pass operator + trust paths.

**Gaps (non-blocking):** no test passing explicit `peer_base` URL that matches a configured relay seed; no unit tests for `peer_base_from_input` edge cases (non-http scheme, domain host, path/query injection).

---

## 6. Concurrency / parallelism

Auth and allowlist checks are synchronous. HTTP peer iteration remains sequential in `select_peer_facts` — unchanged from pre-slice. No new shared-state surfaces or lock-held-across-`await` patterns introduced.

---

## 7. BLOCKERs

None.

---

## 8. Nits (non-blocking)

1. **NIT-1:** Add test with explicit `peer_base: "http://<relay_seed_ip>:port"` matching `relay_http_seeds`.
2. **NIT-2:** Add unit tests for `peer_base_from_input` (e.g. `file://`, `http://evil.example`, `http://127.0.0.1:port` with loopback caller).
3. **NIT-3:** Update rpc-account-api-scope route table row for `/v1/cross-shard/backfill` auth tier on next doc revision.

---

## 9. Verdict

**Approve** — operator auth gates backfill before outbound fetch; `peer_base` is constrained to configured relay HTTP seeds with strict origin parsing; SSRF via caller-supplied URLs is closed. Valid loopback-operator backfill against configured peers remains functional.

---

## 10. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260703-backfill-auth-ssrf-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 11000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260703-backfill-auth-ssrf-review.md'
git commit -m 'docs(v7): backfill operator auth and peer_base SSRF review (409ada9)'
```