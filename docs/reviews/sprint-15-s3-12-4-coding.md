# Sprint 15 S3.12.4 Coding

## Summary

Verdict: **PASS**.

Fixed the trusted outbound peer session loop so normal data-plane frames (`AccountViews`, `CrossShardFacts`, or peer `Heartbeat`) count as heartbeat progress. This prevents a healthy stream from being closed as `heartbeat_read_failed` only because an explicit `HeartbeatAck` was not the next frame read.

## Changes

- `crates/pwmd/src/transport.rs`
  - Added real seed-to-node tracking after successful outbound hello so sticky trusted-session checks can use real sessions, not only test-injected state.
  - Updated the trusted heartbeat read loop to keep the session alive when valid peer data arrives before/without the ack frame.
- `crates/pwmd/src/lib.rs`
  - Added `stateful_transport_data_frames_keep_heartbeat_session_alive`, covering the normal data-plane progress case and fresh trusted `AccountViews` stream preservation.

## Quick Checks

- `cargo fmt` - PASS.
- `CARGO_TARGET_DIR='C:/Users/Alexander/AppData/Local/Temp/pwm-target' cargo test -p pwmd stateful_transport_data_frames_keep_heartbeat_session_alive -- --nocapture` - PASS.
- `CARGO_TARGET_DIR='C:/Users/Alexander/AppData/Local/Temp/pwm-target' cargo test -p pwmd stateful_transport_ -- --nocapture` - PASS (`8 passed`).
- `CARGO_TARGET_DIR='C:/Users/Alexander/AppData/Local/Temp/pwm-target' cargo check -p pwmd` - PASS.

## API / Version Marker

No `pwmd` public API response contract, endpoint validation behavior, field format, or error-code mapping changed. Build/version marker bump was not required.

## Risks / Follow-up

- Full live CY<->DO smoke was not run in this coding slice; hand off to `pwm-testing`.
- The fix intentionally does not add S3.13 federation table behavior or broaden inbound trust. Foreign account lookup still requires a fresh trusted `AccountViews` stream for `ok`; path loss still becomes unavailable/stale through the existing freshness window.

## Optimization Note

The change keeps the large transport module stable and adds only one small helper for seed-node bookkeeping. Further decomposition candidate: extract peer wire session handling from `transport.rs` after S3.12 stabilizes.
