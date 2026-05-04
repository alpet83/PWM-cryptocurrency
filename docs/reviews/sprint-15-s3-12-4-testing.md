# Sprint 15 S3.12.4 Testing

Verdict: **FAIL** for live smoke; focused automated checks passed.

Scope: validate the S3.12.4 peer protocol churn root-cause fix after coding PASS. No production code was changed.

## Automated Checks

- `cargo test -p pwmd stateful_transport_data_frames_keep_heartbeat_session_alive -- --nocapture` - **PASS**, 14.5s, watchdog: no.
  - `1 passed; 0 failed`.
- `cargo test -p pwmd stateful_transport_ -- --nocapture` - **PASS**, 1.8s, watchdog: no.
  - `8 passed; 0 failed`.
- `cargo check -p pwmd` - **PASS**, 4.0s, watchdog: no.
- `cargo fmt --check` - **PASS**, 1.2s, watchdog: no.

## Live CY/DO Smoke

The repo scripts were attempted first:

- `node-1.ps1` / `node-2.ps1` via `Start-Process` - **BLOCKED**, 5.6s.
- Both exited with `os error 10048` because the default RPC/peer ports were already bound:
  - `127.0.0.1:3030` / `127.0.0.1:3130`.
  - `127.0.0.1:3031` / `127.0.0.1:3131`.

A bounded alternate-port smoke was then run directly from `target/debug/pwmd.exe` with the same node identities and genesis parameters:

- CY: RPC `127.0.0.1:4030`, peer `127.0.0.1:4130`, seed `127.0.0.1:4131`.
- DO: RPC `127.0.0.1:4031`, peer `127.0.0.1:4131`, seed `127.0.0.1:4130`.
- Start command duration: 7.6s; observation window: about 55s; watchdog: no for the node run.

Result: **FAIL**.

Evidence:

```text
[18:34:51.524] #INFO: peer handshake completed seed=127.0.0.1:4131 node_id=local-node-DO-s3124 domain_hi=0x32
[18:34:51.524] #INFO: peer session open seed=127.0.0.1:4131 node_id=local-node-DO-s3124 domain_hi=0x32
[18:34:53.044] #INFO: peer session close seed=127.0.0.1:4131 node_id=local-node-DO-s3124 reason=protocol_error detail=heartbeat_read_failed
[18:34:53.048] #INFO: peer reconnect decision seed=127.0.0.1:4131 reason=protocol_error detail=heartbeat_read_failed
```

The same pattern repeated later on both sides, for example:

```text
[18:35:32.195] #INFO: peer session close seed=127.0.0.1:4130 node_id=test-node-CY-s3124 reason=protocol_error detail=heartbeat_read_failed
[18:35:32.195] #INFO: peer reconnect decision seed=127.0.0.1:4130 reason=protocol_error detail=heartbeat_read_failed
```

Assessment:

- The live peer handshake succeeds and sessions open.
- The normal live exchange still shows recurring `protocol_error detail=heartbeat_read_failed` plus reconnect decisions.
- Therefore the smoke acceptance criteria are not met: no steady reconnect/hello churn and no normal `heartbeat_read_failed` during data-plane exchange.
- Foreign account lookup through the trusted peer path was not validated because the live trusted session was not stable enough to use as positive evidence.

## Cleanup

- Stopped the two alternate smoke PIDs recorded in `tmp/s15-s3-12-4-testing/alt-node1.pid` and `alt-node2.pid`.
- Verified no `pwmd` / `pwm-tui` process remained after cleanup.
- Removed `target/debug/incremental`, approx. 550.9 MB.
- Kept smoke logs under `tmp/s15-s3-12-4-testing/` for handoff evidence.

## Participation / Token Estimate

```yaml
agent: pwm-testing
result: FAIL
artifacts:
  - docs/reviews/sprint-15-s3-12-4-testing.md
  - tmp/s15-s3-12-4-testing/node1.out.log
  - tmp/s15-s3-12-4-testing/node2.out.log
  - tmp/s15-s3-12-4-testing/alt-node1.out.log
  - tmp/s15-s3-12-4-testing/alt-node2.out.log
commands:
  - command: cargo test -p pwmd stateful_transport_data_frames_keep_heartbeat_session_alive -- --nocapture
    duration: 14.5s
    result: PASS
    watchdog: no
  - command: cargo test -p pwmd stateful_transport_ -- --nocapture
    duration: 1.8s
    result: PASS
    watchdog: no
  - command: cargo check -p pwmd
    duration: 4.0s
    result: PASS
    watchdog: no
  - command: cargo fmt --check
    duration: 1.2s
    result: PASS
    watchdog: no
  - command: node-1.ps1 / node-2.ps1 via Start-Process
    duration: 5.6s
    result: BLOCKED
    watchdog: no
    note: default ports already bound, os error 10048
  - command: alternate-port CY/DO pwmd smoke
    duration: ~55s observation
    result: FAIL
    watchdog: no
    note: repeated protocol_error detail=heartbeat_read_failed after successful handshakes
  - command: cleanup HTTP probe
    duration: 65.3s
    result: KILLED
    watchdog: yes
    note: hung during Invoke-WebRequest after smoke failure; stopped explicitly
cleanup:
  cleaned: yes
  killed:
    - pwmd pid 46092
    - pwmd pid 1057120
    - hung probe process pid 642152
  verified_remaining_pwmd_or_pwm_tui: none
  artifact_cleanup: removed target/debug/incremental, approx 550.9 MB
token_usage:
  source: estimate
  input: null
  output: null
  total: 22000
  confidence: medium
```
