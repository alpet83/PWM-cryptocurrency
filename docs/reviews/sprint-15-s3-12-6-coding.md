# Sprint 15 S3.12.6 Coding

## Summary

Implemented a narrow production idle-read fix in `crates/pwmd/src/transport.rs`.

- `process_inbound_socket` now treats repeated wire read timeouts and explicit WouldBlock-style no-data errors as idle wait, not session close.
- `run_seed_session` no longer turns repeated idle heartbeat read windows into `heartbeat_read_failed` / reconnect churn.
- Real read/write failures still close the session, and close/reconnect details now include the low-level error string where available.

## Files Changed

- `crates/pwmd/src/transport.rs`
- `docs/reviews/sprint-15-s3-12-6-coding.md`

## Checks

- `cargo fmt` PASS
- `cargo test -p pwmd peer_only_micro_node_harness_survives_idle_and_heartbeats -- --nocapture` PASS
- `cargo test -p pwmd production_ -- --nocapture` PASS
- `cargo check -p pwmd` PASS

## API / Version Note

Public API, wire format, response contracts, endpoint validation behavior, and error-code mappings are unchanged. A `pwmd` version marker bump is not needed for this production transport behavior fix.

## Risks

- Live CY/DO smoke was intentionally not run in this coding slice; hand off to `pwm-testing`.
- The fix keeps idle sessions alive indefinitely, so future tuning should use explicit liveness policy rather than treating timeout/no-data as a network error.

## Participation / Token Estimate

```yaml
agent: pwm-coding
result: PASS
artifacts:
  - crates/pwmd/src/transport.rs
  - docs/reviews/sprint-15-s3-12-6-coding.md
commands:
  - cargo fmt
  - cargo test -p pwmd peer_only_micro_node_harness_survives_idle_and_heartbeats -- --nocapture
  - cargo test -p pwmd production_ -- --nocapture
  - cargo check -p pwmd
token_usage:
  source: estimate
  input: 30000
  output: 5200
  total: 35200
  confidence: medium
```
