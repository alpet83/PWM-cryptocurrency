# Sprint 15 S3.12.3 Testing

Verdict: **PASS** after remediation retest.

Scope: validate peer session diagnostic logging/tests for TCP connect, handshake, session close, reconnect reason, healthy-session skip verbosity, and guard against CLI/TUI unknown/unavailable regressions. No production code was changed in this gate.

## Automated Evidence

Environment: `CARGO_TARGET_DIR=F:/Temp/pwm-target-s15-s3123` (via `%TEMP%/pwm-target-s15-s3123`).

Remediation retest evidence:

- `cargo fmt --check` - **PASS**.
- `cargo check -p pwmd -p pwm-cli` - **PASS**.
- `cargo test -p pwmd stateful_transport_ -- --nocapture` - **PASS**.
  - `7 passed; 0 failed`.
  - Covered:
    - `stateful_transport_session_connects_on_dedicated_peer_socket`
    - `stateful_transport_reports_mismatch_diagnostic`
    - `stateful_transport_remote_hello_mismatch_not_trusted_or_connected`
    - `stateful_transport_wire_read_failure_updates_diagnostics_and_counters`
    - `stateful_transport_eof_after_open_records_close_reason`
    - `stateful_transport_keeps_long_lived_bidirectional_session_stable`
    - `stateful_transport_healthy_trusted_session_skips_timer_redial`
- `cargo test -p pwm-cli tx_import_auto_init_does_not_mask_unknown_export_id -- --nocapture` - **PASS**.
  - `1 passed; 0 failed`; output included the expected mock `204 No Content`.

Healthy-session skip verbosity:

- Source inspection confirms `healthy_session_skip` now updates diagnostics but logs only at `DEBUG` (`peer reconnect skipped ...`), not live `INFO`.
- The focused `stateful_transport_` nocapture run emitted no noisy INFO line for `healthy_session_skip`.

Previous gate evidence:

- `cargo test -p pwmd stateful_transport_ -- --nocapture` - **PASS**.
  - `7 passed; 0 failed`.
  - Covered:
    - `stateful_transport_session_connects_on_dedicated_peer_socket`
    - `stateful_transport_reports_mismatch_diagnostic`
    - `stateful_transport_remote_hello_mismatch_not_trusted_or_connected`
    - `stateful_transport_wire_read_failure_updates_diagnostics_and_counters`
    - `stateful_transport_eof_after_open_records_close_reason`
    - `stateful_transport_keeps_long_lived_bidirectional_session_stable`
    - `stateful_transport_healthy_trusted_session_skips_timer_redial`
- `cargo test -p pwmd trusted_foreign_lookup_without_ready_stream_returns_unavailable -- --nocapture` - **PASS** (`1 passed`).
- `cargo test -p pwm-cli account_lookup_meta_ -- --nocapture` - **PASS** (`2 passed`).
- `cargo test -p pwm-cli rpc_unavailable_error_detects -- --nocapture` - **PASS** (`2 passed`).
- `cargo test -p pwm-cli tx_path_recipient_policy_rejects_unknown_reserve_witness -- --nocapture` - **PASS** (`1 passed`).
- `cargo test -p pwm-tui status_footer_line_online_single_segment_without_red -- --nocapture` - **PASS** (`1 passed`).
- `cargo test -p pwm-tui preflight_selected_initialized_ -- --nocapture` - **PASS** (`2 passed`).

Previously blocking regression evidence:

- `cargo test -p pwm-cli tx_import_auto_init_does_not_mask_unknown_export_id -- --nocapture` - **FAIL** before remediation.
  - Failure: mock server received an unexpected `GET /v1/account/<sender>` request and `ensure_import_sender` returned:
    - `nonce fetch: error sending request for url (...)`
  - The test did not reach the final assertion that unknown `export_id` remains rejected after auto-init.
  - Retest status: **fixed**, same focused regression test now passes.

## Diagnostics Coverage

The focused `pwmd` tests and live smoke confirm that logs/dev status now expose enough context to answer:

- TCP connect established? **Yes**: `peer tcp connect started` / `peer tcp connect succeeded` with seed/local/remote.
- Handshake finalized or failed? **Yes**: `peer handshake started`, `peer handshake completed`, and failure/reject paths are covered by tests.
- Session closed intentionally or by error? **Partial/yes for error paths**: close logs and `/v1/dev/peers.transport` expose `last_session_close_reason` and `peer_close_by_reason`. The live smoke observed error closes (`protocol_error`) rather than an intentional close.
- Reconnect reason? **Yes**: logs and `/v1/dev/peers.transport` expose `peer reconnect decision`, `last_reconnect_reason`, and `reconnect_decision_by_reason`.

## Live Two-Node Smoke

Remediation retest started two temporary `pwmd.exe` nodes directly from `%TEMP%/pwm-target-s15-s3123/debug/pwmd.exe` on alternate ports (`3330/3331`, peer `3430/3431`) with temp state roots and `--log-file off`, then stopped only the two spawned PIDs. Cleanup: **yes**, spawned PIDs were gone after the run.

Retest status/dev diagnostics:

```text
node-1 peer_relay_health: ok
node-1 live_peer_count: 1
node-1 trusted_relay_peer_count: 1
node-1 transport.last_session_close_reason: protocol_error
node-1 transport.last_reconnect_reason: retry_after_close
node-1 transport.counters.peer_close_by_reason.protocol_error: 9
node-1 transport.counters.reconnect_decision_by_reason.protocol_error: 4
node-1 transport.counters.reconnect_decision_by_reason.retry_after_close: 5

node-2 peer_relay_health: ok
node-2 live_peer_count: 1
node-2 trusted_relay_peer_count: 1
node-2 transport.last_session_close_reason: protocol_error
node-2 transport.last_reconnect_reason: retry_after_close
node-2 transport.counters.peer_close_by_reason.protocol_error: 8
node-2 transport.counters.reconnect_decision_by_reason.protocol_error: 4
node-2 transport.counters.reconnect_decision_by_reason.retry_after_close: 5
```

Retest sample lines:

```text
[13:01:25.714] #INFO: peer session close seed=127.0.0.1:3431 node_id=local-node-DO reason=protocol_error detail=heartbeat_read_failed
[13:01:25.714] #INFO: peer reconnect decision seed=127.0.0.1:3431 reason=protocol_error detail=heartbeat_read_failed
[13:01:25.931] #INFO: peer reconnect decision seed=127.0.0.1:3431 reason=retry_after_close
[13:01:25.931] #INFO: peer tcp connect started seed=127.0.0.1:3431 remote=127.0.0.1:3431
[13:01:25.931] #INFO: peer tcp connect succeeded seed=127.0.0.1:3431 local=Some(127.0.0.1:35260) remote=127.0.0.1:3431
[13:01:25.932] #INFO: peer handshake started seed=127.0.0.1:3431 node_id=test-node-CY domain_hi=0x2C
[13:01:25.948] #INFO: peer handshake completed seed=127.0.0.1:3431 node_id=local-node-DO domain_hi=0x32
[13:01:25.948] #INFO: peer session open seed=127.0.0.1:3431 node_id=local-node-DO domain_hi=0x32
```

Assessment: live diagnostics are useful and bounded to real transport events. No `healthy_session_skip` INFO spam was observed. The smoke still shows repeated real `protocol_error` / `heartbeat_read_failed` churn, so that remains a runtime stability follow-up, but it is no longer a diagnostics/noise blocker for this retest.

Earlier smoke evidence:

Started `node-1.ps1` and `node-2.ps1` with temp target dir, then stopped both processes after capture. Cleanup: **yes**, no `pwmd`/`pwm-tui` processes remained after the run.

Sample node-2 lines around churn:

```text
[12:52:05.772] #INFO: peer reconnect decision seed=127.0.0.1:3130 reason=retry_after_close
[12:52:05.772] #INFO: peer tcp connect started seed=127.0.0.1:3130 remote=127.0.0.1:3130
[12:52:05.774] #INFO: peer tcp connect succeeded seed=127.0.0.1:3130 local=Some(127.0.0.1:29071) remote=127.0.0.1:3130
[12:52:05.774] #INFO: peer handshake started seed=127.0.0.1:3130 node_id=local-node-DO domain_hi=0x32
[12:52:05.791] #INFO: peer handshake completed seed=127.0.0.1:3130 node_id=test-node-CY domain_hi=0x2C
[12:52:05.791] #INFO: peer session open seed=127.0.0.1:3130 node_id=test-node-CY domain_hi=0x2C
[12:52:07.302] #INFO: peer session close seed=127.0.0.1:3130 node_id=test-node-CY reason=protocol_error detail=heartbeat_read_failed
[12:52:07.302] #INFO: peer reconnect decision seed=127.0.0.1:3130 reason=protocol_error detail=heartbeat_read_failed
```

Sample node-1 inbound lines:

```text
[12:52:05.774] #INFO: peer tcp connect succeeded seed=inbound local=Some(127.0.0.1:3130) remote=127.0.0.1:29071
[12:52:05.774] #INFO: peer handshake started seed=inbound node_id=unknown domain_hi=unknown remote=127.0.0.1:29071
[12:52:05.783] #INFO: peer handshake completed seed=inbound node_id=local-node-DO domain_hi=0x32
[12:52:05.783] #INFO: peer session open seed=inbound node_id=local-node-DO domain_hi=0x32
[12:52:05.791] #INFO: peer session close seed=127.0.0.1:29071 node_id=local-node-DO reason=protocol_error detail=wire_read_failed
```

Status/dev diagnostics sample from node-2:

```text
peer_relay_health: ok
live_peer_count: 1
trusted_relay_peer_count: 1
transport.last_session_close_reason: protocol_error
transport.last_reconnect_reason: retry_after_close
transport.counters.peer_close_by_reason.protocol_error: 11
transport.counters.reconnect_decision_by_reason.retry_after_close: 12
transport.counters.reconnect_decision_by_reason.protocol_error: 11
```

## Open Risks

- Live smoke still shows repeated real churn (`protocol_error` / `heartbeat_read_failed`) during the short run. This does not block the S3.12.3 diagnostics objective, because the churn is now diagnosable and no healthy-session-skip INFO spam was observed, but it remains a runtime stability concern for follow-up.
- `/v1/status` exposes high-level peer counts/health, while detailed close/reconnect reasons are exposed through `/v1/dev/peers` and logs.
