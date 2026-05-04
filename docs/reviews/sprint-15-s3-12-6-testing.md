# Sprint 15 - S3.12.6 - Testing

Date: 2026-04-30  
Agent: pwm-testing  
Scope: verify S3.12.6 production idle-read fix in `pwmd` with focused checks + live two-node smoke.

## Participation / token estimate

- mode: local shell fallback (no CQDS process tools used in this run)
- token usage (estimate):
  - source: estimate
  - input: null
  - output: null
  - total: 8500
  - confidence: medium

## Commands and results

| Command | Duration | Result | Watchdog |
|---|---:|---|---|
| `cargo fmt --check` | 0.97s | PASS | no |
| `cargo check -p pwmd` | 0.82s | PASS | no |
| `cargo test -p pwmd peer_only_micro_node_harness_survives_idle_and_heartbeats -- --nocapture` | 0.99s | PASS (1 passed) | no |
| `cargo test -p pwmd production_ -- --nocapture` | 1.32s | PASS (3 passed) | no |
| `powershell -NoProfile -Command "./node-1.ps1"` + `powershell -NoProfile -Command "./node-2.ps1"` | 143.5s observed | FAIL (steady churn) | no |
| `Invoke-RestMethod -Uri 'http://127.0.0.1:3030/v1/account/32ecaa3884011f2c21bf09b05e835ec1df5545bebb2c6c478dcacfb70e7fc1c5'` | 0.76s | PARTIAL (`home_lookup_status=unavailable`) | no |

## Live smoke evidence (time-ordered)

Two real nodes were started (`CY` on `3130/3030`, `DO` on `3131/3031`) and observed for ~2m23s.

### Expected acceptance (not met)

- no steady reconnect/hello churn  
- no recurring `wire_read_failed` / `heartbeat_read_failed` on healthy session  
- long-lived session between peers

### Observed behavior

From `CY`-side stream:

- `[19:41:10.149]` `peer hello accepted ... peer=127.0.0.1:39456 ...`
- `[19:41:10.158]` `peer session close ... reason=protocol_error detail=wire_read_failed: wire_decode_failed: u128 is not supported`
- `[19:41:28.848]` hello from new peer port `39614`
- `[19:41:28.856]` same close reason again
- `[19:41:47.615]` hello from `39806`
- `[19:41:47.624]` same close reason again

From `DO`-side stream:

- `[19:41:19.586]` `peer hello accepted ... peer=127.0.0.1:39543 ...`
- `[19:41:19.595]` `peer session close ... reason=protocol_error detail=wire_read_failed: wire_decode_failed: u128 is not supported`
- `[19:41:57.023]` hello from `39890`
- `[19:41:57.032]` same close reason again
- `[19:42:34.424]` hello from `40334`
- `[19:42:34.432]` same close reason again

Classification: **BLOCKER (code/protocol path)**, not idle timeout/no-data.  
Close path affected: both directions (`seed` session close logs on both nodes).

## Foreign lookup check via trusted path

`CY` lookup for known `DO` account:

- endpoint: `GET /v1/account/32ecaa...c1c5` on `http://127.0.0.1:3030`
- result: `"home_lookup_status": "unavailable"`, `"local_view_only": true`
- acceptance `home_lookup_status=ok/known` was not reached due unstable peer stream.

## Checklist/ticket impact

- Ticket context used: `tasks/20260430-s15-slice3-12-6-production-idle-read-fix.json`.
- No checklist row flipped in this run (live acceptance failed).

## Cleanup

- processes cleanup:
  - attempted stop: `Get-Process pwmd,pwm-tui -ErrorAction SilentlyContinue | Stop-Process -Force`
  - verification: `Get-Process pwmd,pwm-tui -ErrorAction SilentlyContinue` (no remaining processes)
- build artifact cleanup:
  - removed `target/debug/incremental`
  - reclaimed size: `589973417` bytes (~562.64 MiB)

## Final status

Result: **PARTIAL**

- focused test/check gates: PASS
- required live two-node stability gate: FAIL (recurring protocol decode closes/churn)
