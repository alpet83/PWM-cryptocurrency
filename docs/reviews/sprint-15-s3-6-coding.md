# Sprint 15 S3.6 coding report

Date: 2026-04-30

## Scope

- Replaced the live `--transport-peer-seed` dial path with HTTP seed probing against existing `pwmd --listen` ports: `GET /v1/status` followed by `POST /v1/peer/hello`.
- Kept the S15-S3.4 provenance boundary: inbound HTTP hello records peer liveness only; provenance trust is granted only when the local node validates the remote hello from its configured outbound seed context.
- Added transport diagnostics in WARN logs and `/v1/status.last_peer_error` for connect/timeout, HTTP status/decode, network mismatch, genesis mismatch, hello rejection, and missing remote hello.
- Exposed `network_id`, `cluster_id`, and `node_id` in `/v1/status` so two-node runbooks can verify identity alignment before judging relay health.

## Files changed

- `crates/pwmd/src/transport.rs`
- `crates/pwmd/src/api.rs`
- `crates/pwmd/src/lib.rs`
- `crates/pwmd/Cargo.toml`
- `docs/pwmd.md`
- `node-2.ps1`
- `docs/reviews/sprint-15-s3-6-coding.md`

## Behavior

- Reciprocal HTTP seed nodes with matching `network_id` and genesis now learn each other through the real transport tick and should report `live_peer_count >= 1`.
- Genesis mismatch from seed `/v1/status` now trips the existing genesis guard and records expected/received hashes in status diagnostics.
- `/v1/peer/hello` and `/v1/dev/peers` remain hidden unless the node is in dev profile or real transport mode is enabled.
- `node-2.ps1` now uses the same custom genesis bundle/passphrase style as `node-1.ps1`.

## Version marker

- Bumped `pwmd` crate marker `0.1.23 -> 0.1.24` because `/v1/status` and `/v1/peer/hello` response behavior changed.

## Commands

- `cargo fmt` -> PASS.
- `cargo test -p pwmd real_transport_tick_connects_seed_and_accepts_handshake` -> PASS.
- `cargo test -p pwmd real_transport_tick_rejects_genesis_mismatch_and_tracks_reason` -> PASS.
- `cargo test -p pwmd v1_status_exposes_genesis_guard_diagnostics` -> PASS.
- `cargo check -p pwmd` -> PASS.

## Optimization note

- Reused existing handshake validation and trusted-peer insertion paths instead of adding a second trust model.
- Remaining decomposition candidate: split HTTP seed probing helpers from `transport.rs` if the transport module grows again in S15-S4+.

## Remediation

Date: 2026-04-30

- Separated peer liveness from trusted relay readiness in `/v1/status`: `live_peer_count` remains generic peer liveness, while new `trusted_relay_peer_count` counts only live peers trusted through configured seed context.
- Changed `peer_relay_health` so inbound/dev hello alone reports `no_trusted_seed`, not `ok`; status hint now calls out the distinction explicitly.
- Added focused pwmd tests for inbound untrusted hello, network mismatch diagnostics, connect failure last error, HTTP status decode failure, and kept genesis mismatch diagnostics explicit.
- Bumped `pwmd` marker `0.1.24 -> 0.1.25` because `/v1/status` response behavior changed.
