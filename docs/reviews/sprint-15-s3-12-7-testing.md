# Sprint 15 - S3.12.7 - Testing

Date: 2026-04-30  
Agent: pwm-testing  
Scope: verify S3.12.7 wire u128 decode compat fix in `pwmd` with focused gates and mandatory live two-node smoke (CY/DO).

## Participation / token estimate

- mode: CQDS `cq_process_ctl` (host mode)
- token usage (estimate):
  - source: estimate
  - input: null
  - output: null
  - total: 10400
  - confidence: medium

## Commands and results

| Command | Duration | Result | Watchdog |
|---|---:|---|---|
| `cargo fmt --check` | 8.33s | PASS | no |
| `cargo test -p pwmd wire_decode_ -- --nocapture` | 5.96s | PASS (3 passed) | no |
| `cargo test -p pwmd production_ -- --nocapture` | 7.13s | PASS (3 passed) | no |
| `cargo check -p pwmd` | 6.07s | PASS | no |
| `powershell -NoProfile -ExecutionPolicy Bypass -File .\node-1.ps1` + `.\node-2.ps1` | >=130s observed | PASS (stable session) | no |
| `Invoke-RestMethod -Uri 'http://127.0.0.1:3030/v1/account/32ecaa3884011f2c21bf09b05e835ec1df5545bebb2c6c478dcacfb70e7fc1c5'` | 22.49s | PASS (`home_lookup_status=ok`) | no |

## Live smoke evidence (time-ordered)

Two real nodes were started (`CY` on `3130/3030`, `DO` on `3131/3031`) and observed for more than 2 minutes.

### Session stability

- `[20:01:12.409]` `peer session open seed=inbound node_id=local-node-DO domain_hi=0x32` (CY side)
- `[20:01:12.417]` `peer session open seed=127.0.0.1:3130 node_id=test-node-CY domain_hi=0x2C` (DO side)
- `[20:02:06.078]` `peer account views merged count=1 source=local-node-DO` (CY side)
- `[20:02:04.706]` `peer account views merged count=1 source=test-node-CY` (DO side)
- `[20:03:44.131]` `sealed height=838` and continuous sealing/merge progression afterwards (DO side)

### Decode/churn acceptance

- No recurring `wire_decode_failed: u128 is not supported` found in observed node logs.
- No steady reconnect/hello churn loop observed; a single reconnect decision appeared during startup transition, then long stable streaming (`peer account views merged` cadence persisted on both nodes).

## Foreign lookup via trusted path

From CY node (`http://127.0.0.1:3030`) for foreign DO account:

- request: `GET /v1/account/32ecaa3884011f2c21bf09b05e835ec1df5545bebb2c6c478dcacfb70e7fc1c5`
- response fields:
  - `"home_lookup_status":"ok"`
  - `"authoritative_home_balance":"1000000"`
  - `"local_view_only":true`

Acceptance `home_lookup_status=ok` reached on stable peer session.

## Classification

Result: **PASS**

- focused compile/test gates: PASS
- mandatory live two-node stability + decode acceptance: PASS
- trusted foreign lookup path (`home_lookup_status=ok`): PASS

## Cleanup

- processes cleanup:
  - stopped `pwmd`/`pwm-tui` via `Get-Process ... | Stop-Process -Force`
  - post-check `Get-Process pwmd,pwm-tui -ErrorAction SilentlyContinue` returned empty output
- build artifact cleanup:
  - removed `target/debug/incremental`
  - reclaimed `656835059` bytes (~626.41 MiB)
