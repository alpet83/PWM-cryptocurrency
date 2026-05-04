# Sprint 15 S15-S3.7 coding report

Date: 2026-04-30

## Scope

- Separated peer transport from RPC: `pwmd` now uses a dedicated peer TCP listener (`transport.peer_listen`) and does not reuse RPC listener as peer socket in normal mode.
- Added peer listen strategy:
  - explicit via CLI/env (`--transport-peer-listen` / `PWM_PEER_LISTEN`),
  - fallback to `rpc_port + 100` when explicit value is not provided.
- Introduced stateful peer transport path:
  - long-lived TCP sessions to configured seed peers,
  - strict JSON framed protocol (`u32 length + json`) for `hello`, `hello_ack`, `heartbeat`, `heartbeat_ack`.
- Improved operations diagnostics:
  - `/v1/status` now includes `peer_listen` and `peer_session_*` counters,
  - mismatch/connect handshake failures propagate into `last_peer_error`,
  - runtime logs show dedicated peer listener startup.

## Files changed

- `crates/pwmd/src/config.rs`
- `crates/pwmd/src/main.rs`
- `crates/pwmd/src/lifecycle.rs`
- `crates/pwmd/src/transport.rs`
- `crates/pwmd/src/api.rs`
- `crates/pwmd/src/lib.rs`
- `crates/pwmd/Cargo.toml`
- `docs/pwmd.md`
- `node-1.ps1`
- `node-2.ps1`
- `docs/reviews/sprint-15-s3-7-coding.md`

## Behavior

- Real transport mode keeps persistent peer sessions instead of HTTP-style status/hello polling.
- Transport trust posture is unchanged from S15-S3.4/S3.6:
  - inbound socket hello can mark peer liveness,
  - trusted relay context still comes from validated outbound seed sessions.
- Startup fails early if transport is enabled but peer listener equals RPC listener.

## Remediation (S15-S3.7 review blocker)

- Outbound stateful seed session no longer ignores `process_incoming_peer_hello(...)` result.
- On remote hello validation error:
  - session is not marked connected/trusted,
  - `session_connected_total` / `session_trusted_total` are not incremented,
  - explicit `last_peer_error` is recorded as `remote_hello_rejected`.
- On stateful wire write/read failures during hello/heartbeat:
  - explicit `last_peer_error` is recorded (`wire_hello_*`, `wire_heartbeat_*`),
  - retry counter is incremented for failed hello path.
- Trust boundary remains strict: inbound untrusted hello (`provenance_trusted=false`) is not promoted into trusted peers.
- Added focused tests for:
  - outbound remote hello mismatch -> no trusted/connected promotion,
  - stateful wire failure -> diagnostics and counters updated.

## Remediation note (S15-S3.7 testing follow-up)

- Root cause was test expectation drift after S3.6/S3.7 wording updates in `roaming_relay_hint`.
- Narrow fix: updated only legacy assertion text in `tests::inbound_hello_does_not_mark_relay_ok`; production behavior unchanged.

## Tests / checks

- `cargo fmt` -> PASS
- `cargo check -p pwmd` -> PASS
- `cargo test -p pwmd stateful_transport_session_connects_on_dedicated_peer_socket` -> PASS
- `cargo test -p pwmd stateful_transport_reports_mismatch_diagnostic` -> PASS
- `cargo test -p pwmd peer_listen_defaults_to_rpc_plus_100` -> PASS

## Version marker

- Bumped `pwmd` marker `0.1.25 -> 0.1.26` because transport/runtime API behavior and `/v1/status` output contract changed.

## Optimization note

- Added stateful peer wiring as an additive path without removing existing fallback transport internals, reducing migration risk for adjacent slices.
- Next decomposition candidate: extract wire framing/session loop into a dedicated `peer_wire` module to keep `transport.rs` smaller as protocol surface grows.
