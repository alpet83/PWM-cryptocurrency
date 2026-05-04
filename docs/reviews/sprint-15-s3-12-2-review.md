# Sprint 15 S3.12.2 Root-Cause Review

## Verdict
`request changes`

## Blockers confirmed

1. Stateful transport still falls into periodic re-hello/reconnect churn (~2s).
2. `peer_relay_health=ok` can coexist with `home_lookup_status=not_found` for foreign lookup, because control-plane liveness and authoritative data-plane readiness are not aligned.

## Root cause summary

- Session lifecycle in `crates/pwmd/src/transport.rs` keeps reconnecting without strong stickiness to an already healthy trusted session.
- Trusted health derives from live peer records, but foreign authoritative lookup depends on `peer_account_views` cache population.
- Cache updates and health signal can diverge, producing long-lived `not_found` while status still reports healthy trusted peers.

## Required remediation

1. Add stronger trusted session stickiness / anti-churn logic in `transport`.
2. Split/align health semantics with data-plane readiness:
   - track freshness of trusted account-view stream per domain/peer;
   - avoid emitting `not_found` when data-plane is not actually ready.
3. Preserve strict trust boundary (trusted peers only) and explicit unknown/unavailable semantics.

## Test gaps to close

- steady-state no-churn regression test over multiple heartbeat windows;
- regression proving foreign lookup becomes `home_lookup_status=ok` with stable trusted path;
- regression for health/data-plane mismatch case to ensure explicit `unavailable` (not false certainty).
