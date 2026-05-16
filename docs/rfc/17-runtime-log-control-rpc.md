# RFC 17: Runtime Log Control RPC

## Status

Draft for post-V3 operator/debug slice.

## Context

`pwmd` already separates public `/v1/*`, operator routes, and dev-only routes in
`docs/api-v1.md`. Today log verbosity is chosen at process startup through
`RUST_LOG` / `EnvFilter` and remains static for the lifetime of the process.
That is inconvenient when an operator needs high-detail logs only around a short
problem window: enabling broad debug logs ahead of time creates large log files
and hides the signal in noise.

This RFC defines an authorized operator RPC surface for temporary runtime log
overrides. It is not part of the stable public `/v1/*` contract.

## Goals

- Raise or narrow `pwmd` logging detail at runtime without restart.
- Focus verbosity on a bounded subsystem such as `transport:peers`, `sync:live`,
  `seal:loop`, `snapshot`, or `api`.
- Require authorization: loopback access or an explicit admin token.
- Require TTL / auto-restore so temporary debug settings do not remain enabled.
- Emit audit events when an override is applied, refreshed, expired, or cleared.
- Keep consensus, peer wire, transaction validation, and state semantics
  unchanged.

## Non-goals

- Public stable API for external clients.
- Remote unauthenticated debug controls.
- Per-request tracing of arbitrary secrets or payload dumps.
- Runtime reconfiguration of file paths, log rotation, or peer log sink routing.
- Replacing `RUST_LOG`; startup filters remain the baseline.

## Endpoint class

All endpoints in this RFC are **operator/debug endpoints**, outside the stable
public `/v1/*` freeze.

Recommended path:

- `GET /v1/operator/log/override`
- `POST /v1/operator/log/override`
- `DELETE /v1/operator/log/override`

If the router later introduces a stricter admin prefix, these paths may move
with a documented compatibility note; they are intentionally not public API.

## Authorization

An operator log-control request is accepted only if at least one condition is
true:

- the HTTP peer address is loopback; or
- `PWM_ADMIN_TOKEN` is configured and the request sends
  `Authorization: Bearer <token>`.

If neither condition is true, return `403`.

If token support cannot be implemented in the first coding slice without broad
config churn, the minimal profile may ship loopback-only and explicitly document
remote operator control as deferred. It must not accept unauthenticated
non-loopback requests.

## Request payload

`POST /v1/operator/log/override`

```json
{
  "level": "debug",
  "focus": "transport:peers",
  "ttl_seconds": 120,
  "reason": "capture short peer reconnect window"
}
```

Fields:

- `level`: one of `trace`, `debug`, `info`, `warn`, `error`.
- `focus`: known focus name, or `all` for a broad temporary override.
- `ttl_seconds`: required positive integer. Suggested range: `1..=3600`.
- `reason`: optional short operator note for audit logs. It must not be used for
  control flow.

Unknown fields should be ignored only if serde defaults already do that locally;
otherwise reject with `400`. The first implementation should prefer a strict,
small schema.

## Focus names

Initial focus names:

| Focus | Intended targets |
|---|---|
| `transport:peers` | peer session, handshake, reconnect, peer health |
| `sync:live` | same-shard live sync and catch-up |
| `seal:loop` | proposer/seal loop, lease/quorum decisions |
| `snapshot` | snapshot load, epoch manifest, replay validation |
| `api` | HTTP handlers/router/operator RPC |
| `all` | broad override for short local captures |

The implementation may map a focus to one or more tracing target filters. The
mapping must be centralized and tested enough that typos do not silently enable
the wrong target.

## Response payloads

Successful `POST` response:

```json
{
  "active": true,
  "level": "debug",
  "focus": "transport:peers",
  "expires_at_ms": 1778920000000,
  "baseline": "RUST_LOG/default startup filter"
}
```

`GET /v1/operator/log/override` returns the same shape. If no override is
active:

```json
{
  "active": false,
  "baseline": "RUST_LOG/default startup filter"
}
```

`DELETE /v1/operator/log/override` clears any active override and returns
`204 No Content` or a small JSON status, whichever matches local handler style.

## Runtime behavior

- Startup logging remains the baseline.
- A runtime override replaces the active reload filter until it expires or is
  cleared.
- Re-posting an override replaces the previous override and resets TTL.
- Expiration restores the baseline filter.
- The current override state is kept in `pwmd` runtime state, not consensus
  state.
- Override state is not persisted across process restart.

## Audit logging

Use a dedicated target such as `pwmd::operator` and emit at least:

- `log_override_set`
- `log_override_cleared`
- `log_override_expired`
- `log_override_rejected`

Audit events should include `focus`, `level`, `ttl_seconds`, and whether auth
was loopback or token based. Do not log token values.

## Minimal implementation profile

The first coding slice should implement:

- reloadable `EnvFilter` layer with a handle stored in `App` or adjacent runtime
  state;
- loopback-only or loopback-plus-token authorization;
- `GET`, `POST`, `DELETE` endpoints;
- focus validation for the initial focus table;
- TTL restore path using a bounded background task or timer;
- focused tests for auth rejection, accepted override, reset, and TTL restore;
- docs updates in `docs/api-v1.md` and `docs/pwmd.md`.

## Test plan

- Unit/handler tests:
  - loopback request can set override;
  - non-loopback without token is rejected;
  - unknown `focus` is rejected;
  - invalid `ttl_seconds` is rejected;
  - `DELETE` clears state;
  - TTL restores baseline.
- Smoke/manual:
  - set `transport:peers` debug for a short TTL;
  - observe audit log event;
  - verify override disappears after TTL.

## Risks

- Leaving verbose logs enabled for too long: mitigated by required TTL.
- Exposing remote debug control: mitigated by loopback/token gate.
- Filter reload breaking peer/main sink split: implementation must preserve the
  existing sink routing and only change filter directives.
- Operator reason leaking sensitive text: reason is optional and documented as
  non-secret.

## Deferred

- Persistent operator policy.
- Per-node RBAC beyond a single admin token.
- UI/TUI controls for log override.
- Fine-grained payload redaction controls.
