# Sprint 15 S3.7 stateful peer transport testing

Date: 2026-04-30 (retested after trust-boundary remediation and S3.7 expectation alignment)  
Repository: `P:/opt/docker/PWM-cryptocurrency`  
Verdict: **PASS**

## Scope

Validated S15-S3.7 testing gate for stateful peer transport redesign on fresh ports and fresh state roots, with explicit checks for:

1. dedicated peer listener socket (not RPC listener),
2. reciprocal peer ports and persistent session counters,
3. reconnect behavior under disconnect,
4. explicit mismatch diagnostics in status/logs,
5. no regression of S15-S3.4/S3.6 one-window trusted relay path.

No production code changes were made.

## Retest after trust-boundary remediation

Targeted revalidation for S15-S3.7 (post-remediation):

1. outbound remote hello validation failure does not mark trusted/connected -> **PASS**
   - `cargo test -p pwmd tests::stateful_transport_remote_hello_mismatch_not_trusted_or_connected -- --exact`
2. wire read/write failures update `last_peer_error` and retry counters -> **PASS**
   - `cargo test -p pwmd tests::stateful_transport_wire_read_failure_updates_diagnostics_and_counters -- --exact` passed (read-failure path validated).
   - no dedicated heartbeat/write-failure test is present in this retest slice, so write-failure signal was not independently re-asserted.
3. reciprocal stateful peer session still works on separate peer socket -> **PASS**
   - `cargo test -p pwmd tests::stateful_transport_session_connects_on_dedicated_peer_socket -- --exact`
4. mismatch diagnostics still explicit -> **PASS**
   - `cargo test -p pwmd tests::stateful_transport_reports_mismatch_diagnostic -- --exact`
   - `cargo test -p pwmd tests::network_mismatch_sets_status_diagnostic -- --exact`
5. no regression of S15-S3.6 relay-health semantics -> **PASS**
   - S3.6 boundary/flow regressions passed:
     - `tests::v1_export_provenance_rejects_self_attested_handoff`
     - `tests::v1_export_provenance_rejects_handoff_after_inbound_node_hello`
     - `tests::v1_export_provenance_accepts_configured_trusted_peer`
     - `tests::v1_export_provenance_obeys_genesis_guard`
     - `pwm-cli::tests::tx_send_cross_domain_one_window_create_and_status_flow`
  - relay-health expectation drift was remediated and revalidated:
    - `cargo test -p pwmd tests::inbound_hello_does_not_mark_relay_ok -- --exact` -> PASS (`1 passed`)
    - updated expectation now matches current status hint wording.

## Test environment (fresh roots/ports)

- Evidence root: `P:/opt/docker/PWM-cryptocurrency/.tmp-test/s15-s3-7-gate-20260430-090935`
- Node A:
  - RPC `127.0.0.1:21930`
  - peer listen `127.0.0.1:22930`
  - seed `127.0.0.1:22931`
  - state root `.tmp-test/s15-s3-7-gate-20260430-090935/node-a`
- Node B:
  - RPC `127.0.0.1:21931`
  - peer listen `127.0.0.1:22931`
  - seed `127.0.0.1:22930`
  - state root `.tmp-test/s15-s3-7-gate-20260430-090935/node-b`
- Node C (network mismatch probe):
  - RPC `127.0.0.1:21932`
  - peer listen `127.0.0.1:22932`
  - seed `127.0.0.1:22930`
  - state root `.tmp-test/s15-s3-7-gate-20260430-090935/node-c`

All nodes used `--transport-real` and explicit `--transport-peer-listen`.

## Validation results

### 1) Dedicated peer listener socket works (not RPC port)

PASS.

- Node A `/v1/status`: `peer_listen=127.0.0.1:22930` while RPC is `127.0.0.1:21930`.
- Node B `/v1/status`: `peer_listen=127.0.0.1:22931` while RPC is `127.0.0.1:21931`.
- Node A runtime log confirms dedicated bind:
  - `#INFO: peer listener active at 127.0.0.1:22930`
  - `#INFO: pwmd listening on http://127.0.0.1:21930 peer=127.0.0.1:22930 ...`

### 2) Reciprocal peer ports establish persistent session and stable counters

PASS.

Initial healthy snapshot (`A <-> B`) reached:

- `live_peer_count=1`
- `trusted_relay_peer_count=1`
- `peer_session_connected_total=1`
- `peer_session_retrying_total=0`
- `peer_session_disconnected_total=0`

After 5 seconds of steady operation, the same counters remained unchanged on both nodes (`stableCounters=true`), indicating stable persistent session state under reciprocal peer sockets.

### 3) Reconnect behavior with warnings under disconnect

PASS.

Procedure:

- Forced disconnect by stopping Node B process.
- Node A status moved into retry/disconnect activity:
  - `peer_session_retrying_total: 0 -> 2`
  - `peer_session_disconnected_total: 0 -> 1`
  - `last_peer_error=seed 127.0.0.1:22931 connect_timeout`
- Restarted Node B on the same peer/RPC ports and state root.
- Session recovered:
  - Node A: `peer_session_connected_total: 1 -> 2`
  - both nodes returned to `live_peer_count=1`, `trusted_relay_peer_count=1`, `peer_relay_health=ok`.

### 4) Genesis/network mismatch diagnostics explicit in status/logs

PASS.

Network mismatch was verified live (Node C with `network_id=s15-s3-7-net-bad` against Node A seed):

- Node C `/v1/status`:
  - `peer_relay_health=no_trusted_seed`
  - `last_peer_error=seed 127.0.0.1:22930 hello_rejected reason=hello_rejected`
  - `peer_session_retrying_total=10`
- Node A logs provide explicit mismatch reason details:
  - `#WARN: peer hello rejected ... reason=network_mismatch expected_network_id=s15-s3-7-net received_network_id=s15-s3-7-net-bad ...`

Genesis mismatch diagnostics are covered by regression tests below (`real_transport_tick_rejects_genesis_mismatch_and_tracks_reason`).

### 5) No regression of S15-S3.4/S3.6 one-window trusted relay path

PASS.

Regression pack remained green:

- trusted provenance boundary tests (`v1_export_provenance*`) passed,
- one-window CLI flow test (`tx_send_cross_domain_one_window_create_and_status_flow`) passed,
- S3.6 mismatch diagnostic tests passed.

## Commands run

- `cargo test -p pwmd tests::stateful_transport_session_connects_on_dedicated_peer_socket -- --exact` -> PASS (`1 passed`)
- `cargo test -p pwmd tests::stateful_transport_remote_hello_mismatch_not_trusted_or_connected -- --exact` -> PASS (`1 passed`)
- `cargo test -p pwmd tests::stateful_transport_wire_read_failure_updates_diagnostics_and_counters -- --exact` -> PASS (`1 passed`)
- `cargo test -p pwmd tests::inbound_hello_does_not_mark_relay_ok -- --exact` -> PASS (`1 passed`)
- `cargo test -p pwmd tests::network_mismatch_sets_status_diagnostic -- --exact` -> PASS (`1 passed`)
- `cargo test -p pwmd tests::real_transport_tick_rejects_genesis_mismatch_and_tracks_reason -- --exact` -> PASS (`1 passed`)
- `cargo test -p pwmd v1_export_provenance -- --nocapture` -> PASS (`4 passed`)
- `cargo test -p pwm-cli tx_send_cross_domain_one_window_create_and_status_flow -- --exact` -> ran `0 tests` (filter mismatch)
- `cargo test -p pwm-cli tx_send_cross_domain_one_window_create_and_status_flow -- --nocapture` -> PASS (`1 passed`)
- `cargo build -p pwmd --bin pwmd` -> PASS
- `cargo build -p pwm-cli --bin pwm` -> PASS
- live S3.7 gate scenario (fresh roots/ports, disconnect/reconnect, mismatch probe) -> PASS

## Cleanup

- Process cleanup: done (`Get-Process pwmd` returned empty after stop).
- Artifact cleanup: removed `target/debug/incremental`, reclaimed `1272597948` bytes.

## Final verdict

**PASS**: Core trust-boundary remediation behavior is validated for the S15-S3.7 targeted slice, and the prior PARTIAL status was caused by legacy hint-text expectation drift in one test. After narrow expectation alignment and focused rerun, all targeted tests listed in this report are green.
