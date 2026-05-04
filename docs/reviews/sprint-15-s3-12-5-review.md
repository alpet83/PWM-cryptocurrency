# Sprint 15 S3.12.5 Review

## Scope Recap

Reviewed S3.12.5 peer-only micro-node harness and prior S3.12.4 live failure evidence:

- `tasks/20260430-s15-slice3-12-5-peer-only-micronode-harness.json`
- `docs/reviews/sprint-15-s3-12-5-coding.md`
- `docs/reviews/sprint-15-s3-12-4-review.md`
- `docs/reviews/sprint-15-s3-12-4-testing.md`
- `tmp/s15-s3-12-4-testing/alt-node1.out.log`
- `tmp/s15-s3-12-4-testing/alt-node2.out.log`
- `crates/pwmd/src/transport.rs`
- `crates/pwmd/src/lib.rs`

Goal: determine whether empty/no-ready/read-timeout behavior can be misclassified as network/protocol close, and narrow the next fix without a transport rewrite.

## Requirements Fit

Partial. The harness is useful and matches the diagnostic slice: it uses the real `PeerWireMsg`, `read_wire_msg`, and `write_wire_msg`, runs reciprocal loopback peers, verifies Hello/HelloAck, data frames, heartbeats, heartbeat acks, and records idle reads.

Hypothesis result: **likely**.

Exact evidence:

- `read_wire_msg` maps elapsed length read timeout to `wire_read_len_timeout`; payload timeout similarly becomes `wire_read_payload_timeout`.
- Harness `read_diag` treats any `is_wire_timeout(err)` as `HarnessRead::Idle`, records action `idle`, and does not classify it as close.
- Harness inbound and outbound loops continue on `HarnessRead::Idle`; only `HarnessRead::Closed` becomes `wire_read_failed` / `heartbeat_read_failed`.
- Production inbound `process_inbound_socket` allows only `MAX_IDLE_TIMEOUT_STREAK = 3`; after repeated timeout it breaks with detail `wire_read_failed`.
- Production outbound `run_seed_session` can turn timeout/no-progress during heartbeat wait into detail `heartbeat_read_failed` after the idle streak threshold.

So S3.12.5 confirms that idle timeout itself is not EOF/disconnect at the wire primitive level, and that production session policy can still surface that condition as close detail. It does not prove that the S3.12.4 live logs were specifically `wire_read_len_timeout`, because those logs did not include the underlying error string and showed `reason=protocol_error`, not `wire_timeout`.

## Style

No blocking style issue in the S3.12.5 harness. The long test name is acceptable because tests may be descriptive. New harness helper names are inside `#[cfg(test)]` and do not violate the production identifier rule.

One non-blocking note: the harness is intentionally test-local and somewhat verbose, but that is justified by diagnostic output.

## Safety

No crypto or trust-boundary regression found in S3.12.5. The harness signs normal hello messages and does not change production behavior.

Operational safety gap remains in production: coarse close details hide whether live failure is timeout, EOF/reset, decode, invalid frame, or another read failure. `WouldBlock` is not explicitly treated as idle/no-data; if it ever surfaces as an IO error string through `read_exact`, `is_wire_timeout` will not catch it and production will classify it as a close/protocol path.

## Tests

Covered:

- peer-only reciprocal handshake;
- data frame exchange;
- five heartbeat intervals per outbound peer;
- at least one idle read timeout;
- zero unexpected `wire_read_failed`;
- zero unexpected `heartbeat_read_failed`.

Missing before acceptance:

- live CY/DO smoke rerun after production fix;
- production-path test that asserts repeated idle read timeout does not close a healthy session prematurely;
- exact low-level error logging/assertion for production close paths.

## Narrow Fix Scope

For next `pwm-coding`, keep scope minimal:

- Change production read-error handling in `process_inbound_socket` and `run_seed_session`, not the wire framing format.
- Treat `wire_read_len_timeout`, `wire_read_payload_timeout`, and elapsed read timeout as idle/no-data, not EOF/disconnect/protocol error.
- Consider treating explicit `WouldBlock`/`would block` read errors as idle/no-data if they can surface from Tokio in this path.
- Keep closing on true EOF/reset/write failure/decode failure/invalid frame/unexpected handshake frame.
- Preserve existing behavior for `wire_invalid_frame_len`, `wire_decode_failed`, connection reset, broken pipe, aborted connection, and handshake rejection.

Acceptance should include:

- focused harness still PASS;
- production two-node stateful transport test with several heartbeat intervals and no `wire_read_failed` / `heartbeat_read_failed`;
- live CY/DO smoke PASS with stable session and no reconnect churn;
- logs include exact low-level read error when a close still happens.

## Verdict

request changes

The harness is a good diagnostic addition, but the live production path is not fixed yet and can still convert idle/read timeout into session close labels.

## Participation / Token Estimate

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/sprint-15-s3-12-5-review.md
  - tasks/20260430-s15-slice3-12-5-peer-only-micronode-harness.json
  - docs/reviews/sprint-15-s3-12-5-coding.md
  - docs/reviews/sprint-15-s3-12-4-review.md
  - docs/reviews/sprint-15-s3-12-4-testing.md
  - tmp/s15-s3-12-4-testing/alt-node1.out.log
  - tmp/s15-s3-12-4-testing/alt-node2.out.log
  - crates/pwmd/src/transport.rs
  - crates/pwmd/src/lib.rs
token_usage:
  source: estimate
  input: 42000
  output: 3600
  total: 45600
  confidence: medium
```
