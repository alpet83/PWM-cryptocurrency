# Sprint 15 S3.12.5 Coding

## What Added

- Added `peer_only_micro_node_harness_survives_idle_and_heartbeats` in `crates/pwmd/src/transport.rs`.
- The harness runs two peer-only micro-nodes in one Tokio test:
  - inbound listener/accept session for each node;
  - outbound seed session for each node;
  - `Hello` / `HelloAck`;
  - empty `CrossShardFacts` and `AccountViews` frames;
  - five reciprocal `Heartbeat` / `HeartbeatAck` intervals.
- Added per-connection diagnostic capture in the test harness: connection id, role, local/remote addr, sent/read frame label, idle read timeout, and exact low-level read/write error string.

## Isolation Value

The harness uses the existing private `PeerWireMsg`, `read_wire_msg`, and `write_wire_msg` primitives but avoids ledger/API/TUI runtime loops. It isolates the stateful peer socket protocol and reciprocal roles without starting the full node services or S3.13 federation work.

## Empty-Buffer Hypothesis

The test explicitly performs an idle socket read after handshake with a short timeout and treats `wire_read_len_timeout` as liveness wait evidence, not EOF, disconnect, or protocol error. EOF/reset and non-timeout read errors still fail with the captured low-level string.

## Quick Checks

- `cargo fmt`
- `cargo test -p pwmd peer_only_micro_node_harness_survives_idle_and_heartbeats -- --nocapture`
- `cargo check -p pwmd`

## Result

Focused harness is intended to pass deterministically with zero unexpected `wire_read_failed` and zero `heartbeat_read_failed` across at least five heartbeat intervals. If testing later sees a failure, the assertion dump includes the exact frame sequence and low-level error per connection.

No `pwmd` public API behavior changed; no version/build marker bump is needed.

## Next Step For Testing

Run this focused harness plus the prior S3.12.4 live CY/DO smoke. If the harness stays stable while live smoke fails, continue root-cause work in full-app integration around task scheduling, config, or process lifecycle rather than the peer wire primitive itself.
