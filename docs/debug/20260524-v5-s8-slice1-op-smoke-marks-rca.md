# V5-8 Slice1 Debug RCA: operator smoke marks/inflation path

Date: 2026-05-24
Ticket: `20260524-v5-s8-slice1-op-smoke-marks-debug`

## Executive result

- `tx-init` failure root cause is confirmed: duplicate init on genesis-funded account, which is already initialized by design.
- Harness fix (`skip tx-init when initialized=true`) works.
- Current smoke still returns PARTIAL because the pass condition requires `marks` growth, but test account is already saturated at `u32::MAX` (`4294967295`) and cannot increase further.
- Protocol path is not blocked: height advances and `marks_last_block` advances to `1` after staking.

## Evidence

1) Fresh smoke rerun with harness fix

- Command:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/devnet_v5_operator_smoke.ps1 -CleanState -SmokeSeconds 120
```

- Report: `tmp/devnet_v5_operator_smoke_20260524_191253.md`
- Observed lines in report:
  - `## tx-init skipped (genesis row already initialized per GenCfg.state0)`
  - `marks baseline: 4294967295, marks_last_block: 0`
  - multiple head advances (`0 -> 60`) and `marks_last_block=1`
  - final result: `PARTIAL (marks not advanced in window)`

2) Direct replay for `AlreadyInit`

- Node started with same genesis/wallet.
- Command:

```bash
cargo run -p pwm-cli --bin pwm -- --rpc http://127.0.0.1:3030 tx-init --wallet ./tmp/demo-genesis-wallet.yaml --index 287292 --flags 0
```

- CLI response:
  - `HTTP 409 Conflict`
  - `reject: code=E_SCHEMA_INVALID class=VALIDATION_ERROR phase=preflight tx_kind=init`
  - `msg=tx cannot apply at tip: already initialized`
  - process exit code `2`

3) Parser status

- Script parser check for `scripts/devnet_v5_operator_smoke.ps1` returns `OK`.
- Earlier parser failures were shell-quoting artifacts in `bash`, not PowerShell script syntax faults.

## Code-level confirmation

- Genesis-funded accounts are initialized at creation:
  - `crates/pwm-core/src/types.rs:351`
  - `crates/pwm-core/src/types.rs:359` (`initialized: true`)

- Harness skip logic is present:
  - `scripts/devnet_v5_operator_smoke.ps1:247`
  - fallback init call remains at `scripts/devnet_v5_operator_smoke.ps1:251`

- Lazy marks are saturating and return immediately at cap:
  - `crates/pwm-core/src/marks.rs:17`
  - `crates/pwm-core/src/marks.rs:18`

- Marks touch path updates `marks_last_block` on inclusion:
  - `crates/pwm-core/src/state.rs:525`
  - `crates/pwm-core/src/state.rs:527`

## Root-cause statement

1. `tx-init` failure is expected when the funded demo account comes from genesis state and is already initialized.
2. Post-fix smoke still reports PARTIAL because acceptance currently requires `marks > baseline`, but baseline can be `u32::MAX` and therefore cannot grow.

## Handoff to pwm-coding

Recommended product-level adjustment (harness semantics, not core protocol):

- In `devnet_v5_operator_smoke.ps1`, treat this as PASS when either condition is true within the window:
  1. `marks` increased and `marks_last_block` increased, or
  2. baseline `marks == 4294967295` and `marks_last_block` increased after stake.

This preserves current intent while handling saturation correctly.

## Commands run

```text
powershell -NoProfile -Command '& { ... Parser::ParseFile(...); ... }'
powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/devnet_v5_operator_smoke.ps1 -CleanState -SmokeSeconds 120
cargo run -p pwm-cli --bin pwm -- --rpc http://127.0.0.1:3030 tx-init --wallet ./tmp/demo-genesis-wallet.yaml --index 287292 --flags 0
```
