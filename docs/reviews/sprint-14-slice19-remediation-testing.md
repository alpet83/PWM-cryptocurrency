# Sprint 14 Slice 19 — remediation testing report

Date: 2026-04-29  
Repo: `P:/opt/docker/PWM-cryptocurrency`

## Verdict

`PASS` for all requested remediation checks.

## Scope checked

1. Reproduce with explicit `--data-file` and verify snapshot file is created.
2. Confirm autosnapshot after `height > 100` writes/updates snapshot file.
3. Verify no regression in `/v1/status` reporting.

## Evidence

### 1) Explicit `--data-file` now creates file

Run command:

```powershell
cargo run -p pwmd --bin pwmd -- --listen 127.0.0.1:3042 --state-root ./tmp/slice19-rem-test --data-file ./tmp/slice19-rem-test/pwm-data.json --network-id slice19-net --domain-hi 0x2C --cluster-id slice19-cluster --node-id slice19-node
```

Checks:

```powershell
Test-Path "tmp/slice19-rem-test/pwm-data.json"
Get-Item "tmp/slice19-rem-test/pwm-data.json" | Select-Object FullName,Length,LastWriteTime
```

Observed:
- `Test-Path` => `True`
- file exists at `tmp/slice19-rem-test/pwm-data.json`
- initial write observed early in runtime (`LastWriteTime` populated, non-zero file size).

Result: `PASS`.

### 2) Autosnapshot after `>100` blocks writes/updates file

Runtime evidence from `pwmd` logs:
- `sealed height=100`
- `autosnapshot checkpoint hit interval=100 height=100`
- `sealed height=101`

Post-check:

```powershell
Get-Item "tmp/slice19-rem-test/pwm-data.json" | Select-Object Length,LastWriteTime
```

Observed:
- file still exists;
- `Length` increased (snapshot content grew with chain);
- `LastWriteTime` advanced after checkpoint window (file updated).

Result: `PASS`.

### 3) `/v1/status` regression check

Command:

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:3042/v1/status" | ConvertTo-Json -Depth 5
```

Observed (both before and after `height > 100`):
- `phase: "ready"`
- `ready: true`
- `snapshot_file: "./tmp/slice19-rem-test/pwm-data.json"`
- no `snapshot_error` field/value reported.

Result: `PASS`.

## Exact commands used

```powershell
cargo check -p pwmd
Remove-Item -Recurse -Force "tmp/slice19-rem-test" -ErrorAction SilentlyContinue; New-Item -ItemType Directory -Path "tmp/slice19-rem-test" | Out-Null; cargo run -p pwmd --bin pwmd -- --listen 127.0.0.1:3042 --state-root ./tmp/slice19-rem-test --data-file ./tmp/slice19-rem-test/pwm-data.json --network-id slice19-net --domain-hi 0x2C --cluster-id slice19-cluster --node-id slice19-node
Start-Sleep -Seconds 7; Test-Path "tmp/slice19-rem-test/pwm-data.json"; (Get-Item "tmp/slice19-rem-test/pwm-data.json" -ErrorAction SilentlyContinue | Select-Object FullName,Length,LastWriteTime | Format-List | Out-String); Invoke-RestMethod -Uri "http://127.0.0.1:3042/v1/status" | ConvertTo-Json -Depth 5
Test-Path "tmp/slice19-rem-test/pwm-data.json"; (Get-Item "tmp/slice19-rem-test/pwm-data.json" | Select-Object Length,LastWriteTime | Format-List | Out-String); Invoke-RestMethod -Uri "http://127.0.0.1:3042/v1/status" | ConvertTo-Json -Depth 5
Get-Process pwmd -ErrorAction SilentlyContinue | Stop-Process -Force; Get-Process pwmd -ErrorAction SilentlyContinue | Select-Object Id,ProcessName
```

## Cleanup

- cleaned: `yes`
- killed: `pwmd` process after verification.
