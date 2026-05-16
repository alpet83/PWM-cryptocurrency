# Review: Runtime log control operator RPC (RFC 17)

**Ticket:** `20260516-pwmd-runtime-log-control-rpc`  
**Scope:** `handlers_operator_log.rs`, `logging.rs` (reload + `ovr_filter_spec`), `state.rs`, `lifecycle.rs` (token wiring), `router.rs`, `http_operator_log.rs`, `docs/api-v1.md`, `docs/pwmd.md`, RFC 17  
**Reviewer:** pwm-review (independent)  
**Date:** 2026-05-16

## 1) Scope recap

Implements authorized operator HTTP surface for temporary `EnvFilter` overrides with TTL and audit events, per `docs/rfc/17-runtime-log-control-rpc.md`. Aligns with ticket requirements: operator-only class, loopback or `PWM_ADMIN_TOKEN` bearer auth, bounded focus names and TTL, reload-based runtime filter without wire/consensus changes. Documentation places endpoints outside the V3 public stable freeze (`docs/api-v1.md` §3.2).

## 2) Requirements fit

| Requirement | Status |
|-------------|--------|
| Operator/debug, not public stable API | **Met** — listed under operator class; English note says not public client API. |
| Auth: no unauthenticated non-loopback control | **Met** — `403` unless loopback or valid bearer when token configured; remote without token rejected. |
| Scoped focus, not arbitrary `EnvFilter` strings | **Met** — `deny_unknown_fields` on POST body; focus whitelist + centralized `focus_targets` mapping in `logging.rs`. |
| TTL auto-restore | **Met** — timer task with revision guard; `GET` path calls `clear_if_expired`; tests use bounded polling. |
| Audit | **Met** — `pwmd::operator` events for set, cleared, expired, reject; token values not logged. |
| Preserve sink split / no wire impact | **Met** — single reload `EnvFilter` on registry; fmt layers retain peer vs non-peer writer filters; slice does not touch `PeerWireMsg` or consensus. |

**Gaps (low severity):** RFC audit list mentions operator `reason` for human context; `log_override_set` structured line does not include `reason` (response JSON does). Optional consistency nit with RFC wording.

## 3) Style and module shape

- New handler module has a short `//!` banner; router wiring is minimal.
- Focus mapping and TTL bounds live next to handler constants; reload trait in `logging.rs` is a reasonable split.
- **Naming:** pwm-testing reported `check_entity_name_segments` clean on touched paths; no production symbols observed with six-plus `snake_case` segments in this slice.
- **Micro-modularity:** No inappropriate new blob in `lib.rs` façade beyond existing `cors_for_listen` extension (methods list).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice; operator HTTP JSON uses string level/focus and numeric `ttl_seconds` only).

## 4) Safety

**No high-severity findings.**

| Topic | Severity | Notes |
|-------|----------|--------|
| Auth gate | — | Loopback bypass only when `ConnectInfo` reports loopback IP; production uses `into_make_service_with_connect_info::<SocketAddr>` (verified in `lifecycle.rs`). |
| Token handling | Low | Bearer compared with `==` (not constant-time); acceptable for many deployments; harden if threat model demands. |
| Token leakage | — | Audit logs `has_token_cfg`, not secret; responses omit token. |
| Proxy / trusted hops | Low (ops) | If HTTP is fronted so the app always sees `127.0.0.1`, loopback auth could widen exposure — operational/runbook caveat, not a code bug in isolation. |
| Filter injection | — | User never supplies raw filter directives; only whitelisted focus → fixed targets. |
| DoS via log volume | — | Mitigated by TTL cap (1..=3600) and operational expectation; matches RFC. |
| Clock skew | Low | TTL task uses monotonic sleep from wall `expires_at_ms`; large jumps could shorten effective window — acceptable for debug feature. |

## 5) Tests

**Covered:** set/get/delete on loopback; invalid focus and TTL; remote denied (no token); token allows remote; wrong bearer remote denied with override unchanged; TTL restore via polling; raw TCP loopback listener smoke with bearer.

**Residual gaps (nits):** No explicit test for “non-loopback, token configured, `Authorization` header missing” (behavior is same branch as other 403 cases). No test that `log_ctl: None` yields `503` (would require special harness without `init_logging`).

**Commands reviewed (from pwm-testing / ticket notes):** `cargo fmt --check`; `cargo check -p pwmd --lib`; `python scripts/check_entity_name_segments.py` (clean); `cargo test -p pwmd op_log_` (8/8).

## 6) Verdict

**Approve with nits** — security model matches RFC and docs; focus and TTL validation are strict; reload and TTL cancellation logic are sound; tests are substantive including wrong-token and TCP smoke.

**Nits**

1. **Owner decision (security hygiene):** optional constant-time comparison for admin bearer token (low priority).
2. **Owner decision (audit completeness):** add optional `reason` field to `log_override_set` audit line if operators rely on correlating notes in log-only workflows.
3. **Mechanical / docs (ops):** consider one sentence in `docs/pwmd.md` or runbook: reverse proxies must preserve real client IP **or** rely on token-only remote control — avoids misconfigured loopback trust.
4. **Mechanical:** CORS now allows `DELETE` on non-loopback when `PWM_CORS_ORIGINS` is set — acceptable for operator tooling; combined with bearer token, browser abuse risk is low but non-zero if tokens ever appear in frontend code (document “CLI/curl only” if desired).

## 7) Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/runtime-log-control-rpc-review-20260516.md
verdict_detail: APPROVE_WITH_NITS
token_usage:
  source: estimate
  input: 12000
  output: 3200
  total: 15200
  confidence: medium
```

## Sprint-final glossary

Not a sprint-final gate review for this slice — **GLOSSARY.md: no change** (terminology already covered by RFC and operator docs).
