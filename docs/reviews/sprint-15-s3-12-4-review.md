# Sprint 15 S3.12.4 Review

## Findings

### High: live FAIL remains; fix is conceptually partial, not complete

S3.12.4 fixes a real subcase in `crates/pwmd/src/transport.rs`: outbound heartbeat read now treats trusted `AccountViews`, `CrossShardFacts`, and peer `Heartbeat` as progress instead of requiring `HeartbeatAck` as the next frame. That direction is reasonable.

But the live CY/DO logs still show the same acceptance blocker: after successful handshake/session open, the remote inbound side closes almost immediately with `wire_read_failed`, then the outbound side closes at the first heartbeat cycle with `heartbeat_read_failed`.

Evidence from `tmp/s15-s3-12-4-testing/alt-node1.out.log` and `alt-node2.out.log`:
- CY outbound opens to DO at `18:34:51.524`; DO inbound closes the matching socket at `18:34:51.524` with `wire_read_failed`.
- CY outbound then closes at `18:34:53.044` with `heartbeat_read_failed`.
- The same mirrored pattern repeats on DO outbound / CY inbound.

This means the remaining failure is likely earlier than the S3.12.4 fix: the peer expected to answer heartbeat is already gone. The implementation should not be accepted as complete.

### High: most likely root cause is reciprocal-session asymmetry / early inbound close, not sticky map

The repeated sequence strongly points to a race/asymmetry between reciprocal dialers and inbound/outbound session roles:

- both nodes dial each other;
- each node also accepts an inbound socket from the peer;
- inbound sockets are accepted and opened, but close immediately with `wire_read_failed`;
- outbound sockets then fail when waiting for heartbeat progress.

`mark_seed_peer_node` and `has_sticky_trusted_session` are useful for avoiding timer redial after a healthy outbound seed session, but the smoke shows the session is not healthy long enough. The sticky map is probably not the first-order cause.

The review could not prove the exact low-level read error because current close logs collapse the actual `read_wire_msg` error into `wire_read_failed` / `heartbeat_read_failed`. This hides whether the failure is EOF, reset, invalid frame length, decode error, or payload read timeout.

### Medium: inbound path is not tested symmetrically enough

The focused tests pass because they do not reproduce the live sequence with enough fidelity.

Notable gaps in `crates/pwmd/src/lib.rs`:
- `stateful_transport_data_frames_keep_heartbeat_session_alive` uses a custom fake seed that sends `AccountViews` instead of `HeartbeatAck`; it validates the new outbound progress rule, but not the real peer listener / inbound responder loop.
- `stateful_transport_keeps_long_lived_bidirectional_session_stable` uses reciprocal seeds, but observes only a short window and mainly asserts `app_a`; it does not assert both sides have zero `wire_read_failed` / `heartbeat_read_failed` after multiple production-default heartbeat intervals.
- No test asserts that an inbound accepted session survives long enough to read outbound `CrossShardFacts` / `AccountViews`, respond to outbound `Heartbeat`, and avoid immediate `wire_read_failed`.

### Medium: diagnostics are still too lossy for this failure

`record_peer_close` logs reason/detail, but the detail is a coarse label. For this slice, the important missing evidence is the exact underlying error string from `read_wire_msg`.

Current logs say:
- `reason=protocol_error detail=wire_read_failed`
- `reason=protocol_error detail=heartbeat_read_failed`

They do not say whether the underlying error was `wire_read_len_failed`, `wire_invalid_frame_len`, `wire_decode_failed`, connection reset, or EOF. That makes the next root-cause step slower than necessary.

### Low: style is acceptable

No blocking production identifier style issue found in the S3.12.4 additions. New non-test names such as `trusted_account_streams`, `sticky_session_window_ms`, `has_sticky_trusted_session`, `mark_trusted_peer_live`, and `DRAIN_TIMEOUT_CAP_MS` fit the <=4-word local rule.

## Scope Recap

Claimed S3.12.4 scope was to fix peer-session churn root cause around `protocol_error` / `heartbeat_read_failed`, preserve trust boundary/genesis guard/foreign account semantics, avoid S3.13 federation work, and validate no steady reconnect churn in live CY/DO.

Implementation touched:
- `crates/pwmd/src/transport.rs`
- `crates/pwmd/src/lib.rs`
- `issues-report.md`

Reviewed artifacts:
- `tasks/20260430-s15-slice3-12-4-peer-protocol-churn-rootcause-fix.json`
- `docs/reviews/sprint-15-s3-12-4-coding.md`
- `docs/reviews/sprint-15-s3-12-4-testing.md`
- `tmp/s15-s3-12-4-testing/alt-node1.out.log`
- `tmp/s15-s3-12-4-testing/alt-node2.out.log`
- prior S3.12.3 review/testing artifacts

## Requirements Fit

Partial only.

Met:
- focused checks pass;
- data-plane frames can now count as outbound heartbeat progress;
- successful outbound hello records seed-to-node mapping;
- trust boundary is not obviously widened: inbound processing still uses untrusted provenance.

Not met:
- live CY/DO still has steady reconnect/session churn;
- live session is not long-lived;
- `heartbeat_read_failed` still happens after successful open;
- foreign lookup positive path was not validated because live trusted stream is unstable.

## Safety

No obvious crypto/trust-boundary regression found in this slice. Inbound `AccountViews` / `CrossShardFacts` remain untrusted and ignored for provenance-sensitive merge.

Main safety risk is operational: the live loop still churns and may present misleading `live_peer_count` / transient trust state if callers sample during sticky windows. The current diagnostics also classify some likely EOF/reset cases as `protocol_error`, which can mislead triage.

## Tests

Automated focused checks passing is not sufficient evidence for acceptance because the live smoke failed.

Required test coverage for S3.12.5 should include either:
- a real two-node in-process stateful transport test with reciprocal seeds, production-default heartbeat interval/timeout, and assertions on both nodes after several heartbeat cycles; or
- a peer-only micro-node harness that runs only listener + outbound seed loops and captures exact frame/error sequencing.

The test must assert:
- no repeated `wire_read_failed`;
- no repeated `heartbeat_read_failed`;
- both sides keep a stable trusted outbound path;
- inbound responder reads frames and sends `HeartbeatAck`;
- `AccountViews` freshness reaches `home_lookup_status=ok`.

## Recommendation: S3.12.5 Diagnostic Slice

I do not think there is enough evidence to claim one concrete code root cause. The credible failure area is reciprocal stateful peer session handling, especially the immediate inbound close. Next slice should isolate that before broad rewrites.

S3.12.5 should build a peer-only micro-node harness with:
- two minimal nodes, no ledger/API/TUI noise;
- each node has `peer_listen`, reciprocal `peer_seeds`, identity, genesis guard, and the current stateful transport loops;
- per-connection IDs in logs: role `inbound|outbound`, local/remote addr, remote node id, frame type sent/read, and exact read/write error string;
- production-default heartbeat values and a short accelerated variant;
- deterministic acceptance checks: stable for at least 5 heartbeat intervals, zero reconnect churn, zero inbound `wire_read_failed`, zero outbound `heartbeat_read_failed`.

Expected output:
- if current code is wrong, the harness should reproduce inbound immediate close and reveal the exact underlying read error;
- if the full app environment is the trigger, the harness should stay stable, proving the issue is outside pure peering.

If the harness immediately identifies the concrete error, S3.12.5 can narrow the fix to that specific role/frame sequencing issue.

## Verdict

request changes

Live acceptance failed and the code should not be accepted as complete for S3.12.4.

## Participation / Token Estimate

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/sprint-15-s3-12-4-review.md
  - tasks/20260430-s15-slice3-12-4-peer-protocol-churn-rootcause-fix.json
  - docs/reviews/sprint-15-s3-12-4-coding.md
  - docs/reviews/sprint-15-s3-12-4-testing.md
  - tmp/s15-s3-12-4-testing/alt-node1.out.log
  - tmp/s15-s3-12-4-testing/alt-node2.out.log
token_usage:
  source: estimate
  input: 52000
  output: 6200
  total: 58200
  confidence: medium
```
