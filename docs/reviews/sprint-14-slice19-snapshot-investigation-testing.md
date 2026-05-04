# Sprint 14 Slice 19: snapshot persistence investigation (testing)

## Verdict

`pwmd` на текущем `HEAD` **не сохраняет** `pwm-data.json` даже после `height > 100`.

Root cause category: **logic bug / lifecycle not reaching save path** (не config/path и не filesystem permission).

## Reproduction on HEAD

Environment:
- repo: `P:/opt/docker/PWM-cryptocurrency`
- binary: `pwmd`
- explicit `--data-file` provided

Run:

```powershell
cargo run -p pwmd --bin pwmd -- `
  --listen 127.0.0.1:3040 `
  --state-root .\tmp\slice19-state `
  --data-file .\tmp\slice19-state\pwm-data.json `
  --network-id slice19-net `
  --domain-hi 0x2C `
  --cluster-id slice19-cluster `
  --node-id slice19-node
```

Observed runtime:
- logs show steady sealing up to at least `sealed height=116`;
- no `autosnapshot checkpoint hit interval=100 height=100` line;
- no snapshot save error line;
- `GET /v1/status` returns:
  - `phase: "ready"`
  - `snapshot_file: ".\\tmp\\slice19-state\\pwm-data.json"`
  - no `snapshot_error`.

File check:

```powershell
Test-Path P:\opt\docker\PWM-cryptocurrency\tmp\slice19-state\pwm-data.json
```

Result: `False` (file missing).

Also reproduced with absolute path:

```powershell
cargo run -p pwmd --bin pwmd -- `
  --listen 127.0.0.1:3041 `
  --state-root P:\opt\docker\PWM-cryptocurrency\tmp\slice19-abs `
  --data-file P:\opt\docker\PWM-cryptocurrency\tmp\slice19-abs\pwm-data.json `
  --network-id slice19-net `
  --domain-hi 0x2C `
  --cluster-id slice19-cluster `
  --node-id slice19-node-abs
```

After sealing (`height>=2`) file still missing.

## Genesis save and autosnapshot checks

1. Genesis/startup:
   - startup loader logs `snapshot file not found, fallback to genesis state`;
   - there is no startup/genesis save attempt.

2. Autosnapshot @100 blocks:
   - expected checkpoint log not emitted at `height=100`;
   - effective save path not executed.

## Runtime path/status/log error analysis

- Runtime status reports only configured path in init state (`snapshot_file`), not proof of successful writes.
- `snapshot_error` remains empty because save code path is not entered.
- No filesystem/write errors were observed in logs.

## Technical root cause

In `run_with` app is created via:
- `app_from_genesis_shard_identity(...)` (ранее `app_from_genesis_in_shard_with_identity`)

This constructor currently builds `App` with `data_file: None`:
- `crates/pwmd/src/bootstrap.rs` -> `app_from_genesis_shard_identity` вызывает `app_from_chain_boot(..., None, ...)`.

But save code requires `app.data_file`:
- `crates/pwmd/src/lifecycle.rs`:
  - `if let Some(path) = app.data_file.as_deref() { ... save_snapshot(...) }`

Therefore:
- startup/status can mention configured `config.data_file`,
- but periodic/runtime snapshot save never runs.

## Immediate workaround

Operator-level workaround without code change is not practical for long-running node persistence, because CLI `--data-file` does not reach runtime save path in current code path.

Temporary operational workaround:
- use manual offline export mechanism (if available in your flow) or patch node build locally before running production-like sessions.

## Code-fix recommendation

Minimal fix (recommended):
1. Ensure `App.data_file` is set from `PwmdConfig.data_file` in `run_with`.
   - e.g. after app creation:
   - `app.data_file = Some(config.data_file.clone());`

More structural fix:
1. Add/extend constructor with identity + data_file parameter and use it from `run_with`:
   - wire `config.data_file.clone()` through bootstrap path.
2. Add regression test:
   - start app via `run_with`-equivalent config path with explicit data file,
   - execute seal loop step or tx endpoint,
   - assert snapshot file appears and updates.

Optional observability hardening:
- expose `last_snapshot_save_at_height` / `last_snapshot_save_ok` in `/v1/status` to distinguish configured path vs actual write activity.
