# Sprint 14 Slice 18 — testing validation

## Scope

Repository: `P:/opt/docker/PWM-cryptocurrency`

Validated items:
1. `LOGGING_STYLE.md` exists and matches requested palette rules.
2. Default file template resolves to `logs/{date}/{log_name}_{time}.log`.
3. Default verbosity `DEBUG` works when `RUST_LOG` is absent.
4. Startup `INFO`/errors go through logger object path.
5. `DEBUG` tx inclusion logs with balance deltas are emitted on block seal path.
6. Rotation and tty/non-tty behavior not regressed.

## Commands run (exact)

```powershell
cargo test -p pwmd logging::tests::template_expands_subdir_placeholders -- --nocapture
cargo test -p pwmd logging::tests::console_color_auto_non_tty_is_plain -- --nocapture
cargo test -p pwmd logging::tests::rotation_triggers_and_keeps_retention_cap -- --nocapture
cargo test -p pwmd logging::tests::on_mode_degrades_after_rotate_error -- --nocapture
cargo test -p pwmd logging::tests::required_mode_panics_after_rotate_error -- --nocapture
cargo test -p pwmd config::tests::logging_defaults_match_slice18_template -- --nocapture
python -c "import os,subprocess,sys,pathlib,re; env=os.environ.copy(); env.pop('RUST_LOG',None); out=pathlib.Path('tmp/slice18-runtime-stdout.log'); err=pathlib.Path('tmp/slice18-runtime-stderr.log'); out.parent.mkdir(parents=True,exist_ok=True); p=subprocess.Popen(['cargo','run','-p','pwmd','--','--listen','127.0.0.1:3044','--state-root','./tmp/slice18-runtime-state','--log-name','slice18rt'],stdout=out.open('wb'),stderr=err.open('wb'),env=env); rc=None; 
try:
 p.wait(timeout=7)
 rc=p.returncode
except subprocess.TimeoutExpired:
 p.terminate();
 try:
  p.wait(timeout=3)
 except subprocess.TimeoutExpired:
  p.kill(); p.wait()
 print('RUNTIME_EXIT', p.returncode)
 print('STDOUT_FILE', out.as_posix())
 print('STDERR_FILE', err.as_posix())"
rg --files logs
```

## Verdict by requirement

### 1) LOGGING_STYLE.md presence + palette rules
- **PASS**.
- File exists: `docs/LOGGING_STYLE.md`.
- Contains requested palette policy: numeric values bright purple, `#ERROR` bright red, `#WARN` dark red; non-TTY/`NO_COLOR` plain output policy documented.

### 2) Default template path
- **PASS**.
- Default configured template is `"{date}/{log_name}_{time}.log"` in:
  - `crates/pwmd/src/config.rs` (`LoggingConfig::default`)
  - `crates/pwmd/src/main.rs` (CLI default `--log-file-template`)
- Runtime check created file:
  - `logs/2026-04-29/slice18rt_054502.log`
  - This matches `logs/{date}/{log_name}_{time}.log` with `log_name=slice18rt`.

### 3) Default DEBUG when RUST_LOG absent
- **PASS**.
- `crates/pwmd/src/logging.rs`: `EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"))`.
- Runtime harness explicitly removed `RUST_LOG` from env and logger initialized/ran normally.

### 4) Startup INFO/errors through logger object path
- **PASS**.
- Code path uses logger object (`crate::logger().info/error(...)`) for startup phase and fatal startup errors in:
  - `crates/pwmd/src/lifecycle.rs`
  - `crates/pwmd/src/main.rs`
- Runtime output confirms startup phase lines from `pwmd::logging` logger path (e.g. `pwmd startup phase: loading_snapshot`, `ready (no snapshot file)`, `pwmd listening ...`).

### 5) DEBUG tx inclusion with balance deltas on seal path
- **PASS (code-path validated)**.
- `crates/pwmd/src/lifecycle.rs`: `spawn_seal_loop` -> successful `chain.seal(...)` -> `log_tx_debug(...)`.
- `log_tx_debug(...)` calls `logger().debug_tx(...)` per affected account.
- `crates/pwmd/src/logging.rs::NodeLogger::debug_tx` emits `tx_included` with fields:
  - `height`, `tx_kind`, `tx_id`, `addr`, `bal_before`, `bal_after`, `bal_delta`.
- In this minimal runtime session no user tx was submitted, so no live `tx_included` line was observed; implementation contract on seal path is present and wired.

### 6) Rotation + tty/non-tty
- **PASS**.
- Tests passed:
  - `logging::tests::rotation_triggers_and_keeps_retention_cap`
  - `logging::tests::on_mode_degrades_after_rotate_error`
  - `logging::tests::required_mode_panics_after_rotate_error`
  - `logging::tests::console_color_auto_non_tty_is_plain`
- Runtime file log contains plain text without ANSI escapes; file sink is configured with `.with_ansi(false)`.

## Focused test result summary

- All targeted `pwmd` logging tests above: **PASS**.
- Minimal runtime check (`cargo run -p pwmd ...` under timeout harness): **PASS** for startup/file-template checks.

## Notes / residual risk

- Item 5 was validated at implementation level and seal code path level, but not by a live RPC tx injection in this run.
- If needed, add one deterministic integration harness that submits one valid tx and asserts `tx_included` is present in generated file log.
