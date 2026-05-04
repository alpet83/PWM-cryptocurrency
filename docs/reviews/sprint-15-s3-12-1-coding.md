# Sprint 15 — S3.12.1 Coding

Дата: 2026-04-30

## Scope

- Remediation slice `S15-S3.12.1` for stateful transport churn and foreign account view propagation.
- Preserve strict trusted boundaries and existing unknown/unavailable semantics in API/CLI/TUI.

## What changed

- `crates/pwmd/src/transport.rs`
  - Fixed stateful heartbeat handling to drain pending wire frames after `HeartbeatAck` with a bounded short timeout window.
  - Kept heartbeat timeout tolerance logic without relaxing trust checks.
  - Result: long-lived session no longer re-handshakes from inbox backlog under normal heartbeat cadence.

- `crates/pwmd/src/lib.rs` (tests)
  - Added `stateful_transport_keeps_long_lived_bidirectional_session_stable`.
  - Added `stateful_transport_propagates_foreign_account_view_to_lookup_ok`.
  - These cover churn regression and `home_lookup_status=ok` propagation for existing home-shard account data via trusted path.

- `issues-report.md`
  - Appended the transport inbox backlog trap and remediation note.

## Behavior notes

- Trust boundary remains strict:
  - account/fact merges still require trusted context;
  - no widening of acceptance path for untrusted peers.
- Unknown/unavailable semantics are preserved:
  - this slice only stabilizes delivery/session behavior so legitimate trusted foreign lookups can become `ok`.

## Validation plan executed

- `cargo fmt`
- `cargo test -p pwmd stateful_transport_keeps_long_lived_bidirectional_session_stable -- --nocapture`
- `cargo test -p pwmd stateful_transport_propagates_foreign_account_view_to_lookup_ok -- --nocapture`
- `cargo check -p pwmd`

## Outcome

- Stateful trusted session remains stable in reciprocal topology without reconnect churn.
- Foreign account view propagates over trusted path and lookup returns `home_lookup_status=ok` when home account exists.
