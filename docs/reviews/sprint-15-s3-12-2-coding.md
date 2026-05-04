# Sprint 15 S3.12.2 Coding Remediation

## Scope

- Removed steady-state reconnect/hello churn in stateful transport by adding trusted session stickiness.
- Aligned foreign account lookup semantics with authoritative trusted account-view data-plane readiness/freshness.

## Implemented changes

1. Transport stickiness and lifecycle stabilization (`crates/pwmd/src/transport.rs`)
   - Added sticky-session gate before outbound redial for a seed when a healthy trusted peer session is already live.
   - Added peer liveness touch on heartbeat and data-plane traffic to avoid false disconnect flaps.
   - Softened disconnect marking: do not force peer `Disconnected` if it was observed alive inside sticky window.

2. Trusted account-view stream readiness/freshness (`crates/pwmd/src/transport.rs`, `crates/pwmd/src/api.rs`)
   - Added `trusted_account_streams` in handshake state (`node_id -> domain_hi + last_update_ms`).
   - Updated on trusted `AccountViews` frames only (strict trust boundary preserved).
   - Added freshness window derived from heartbeat settings.

3. API lookup semantics (`crates/pwmd/src/api.rs`)
   - `ok` only when foreign account has authoritative peer view from a trusted live peer with fresh stream.
   - `not_found` only when trusted data-plane is fresh/ready for that home domain and account is absent.
   - `unavailable` when trusted path is absent or authoritative stream is not ready.
   - `stale` when trusted stream exists but is stale.

4. Regression tests (`crates/pwmd/src/lib.rs`)
   - Updated trusted foreign lookup test to provide fresh trusted stream state.
   - Added mismatch regression: trusted path live but stream not ready returns `home_lookup_status=unavailable` (not false `not_found`).

## API/build marker

- Bumped `pwmd` build/version marker: `0.1.27 -> 0.1.28` because `/v1/account/:id` and `/v1/accounts` lookup-status behavior changed.
