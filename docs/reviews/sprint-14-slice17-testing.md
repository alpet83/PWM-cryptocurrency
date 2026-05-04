# Sprint 14 / Slice 17 — testing review (logger)

Date: 2026-04-29  
Scope: `crates/pwmd` logger implementation validation.

## Verdict

**PASS (slice scope validated).**  
Slice 17 logging functionality is implemented and behaves as expected at config/unit/runtime-smoke level for:
- CLI/env options wiring,
- template expansion + path safety guards,
- size rotation + max-files retention,
- TTY/non-TTY color policy logic,
- documentation alignment in `docs/pwmd.md`.

Note: full `pwmd` crate suite currently has unrelated pre-existing failures in `tx_policy` tests (see commands section).

## Check results

1. **CLI/env parsing for new logging options** — **OK**
   - `main.rs` exposes:
     - `--log-name` / `PWM_LOG_NAME`
     - `--log-dir` / `PWM_LOG_DIR`
     - `--log-file-template` / `PWM_LOG_FILE_TEMPLATE`
     - `--log-file` / `PWM_LOG_FILE`
     - `--log-console-color` / `PWM_LOG_CONSOLE_COLOR`
     - `--log-rotate-size-mb` / `PWM_LOG_ROTATE_SIZE_MB`
     - `--log-rotate-max-files` / `PWM_LOG_ROTATE_MAX_FILES`
   - Runtime parse checks:
     - invalid `--log-file` rejected by clap (exit 2),
     - invalid `PWM_LOG_CONSOLE_COLOR` rejected by clap (exit 2),
     - invalid `--log-rotate-max-files 0` rejected by `LoggingConfig::validate()` (exit 2).

2. **Template expansion and safety guards** — **OK**
   - `expand_log_template_path(...)` supports `{date}`, `{time}`, `{datetime}`, `{log_name}`, `{pid}`.
   - Guards reject:
     - absolute/rooted paths,
     - `..` path traversal,
     - empty expanded path.
   - Unit tests passed for expansion and safety checks.
   - Runtime smoke with `--log-file-template ../escape.log` showed guarded degradation to console-only.

3. **Size rotation + max-files retention behavior** — **OK**
   - `RotatingFile` enforces `max_size > 0`, `max_files > 0`.
   - Rotation policy:
     - active file -> `.1`,
     - older files shift `.N -> .N+1`,
     - oldest beyond cap removed.
   - Unit test `rotation_triggers_and_keeps_retention_cap` passed.

4. **TTY/non-TTY color policy logic (config/unit level)** — **OK**
   - `ConsoleColorMode::use_ansi` semantics:
     - `auto` -> `is_tty`,
     - `always` -> true,
     - `never` -> false.
   - Unit tests in `config.rs` and `logging.rs` passed for non-TTY safety and mode behavior.

5. **Docs alignment in `docs/pwmd.md`** — **OK**
   - Docs list matches implemented CLI/env options and default values.
   - Docs describe color policy and file rotation/template constraints consistently with code.

## Commands and results

Focused slice checks:
- `cargo test -p pwmd logging::tests:: -- --nocapture` -> **PASS** (6 passed)
- `cargo test -p pwmd config::tests:: -- --nocapture` -> **PASS** (4 passed)
- `cargo run -p pwmd -- --log-file invalid` -> **PASS (negative expected)**, clap rejects invalid enum (exit 2)
- `PWM_LOG_CONSOLE_COLOR=invalid cargo run -p pwmd -- --listen 127.0.0.1:0` -> **PASS (negative expected)**, clap rejects invalid enum (exit 2)
- `cargo run -p pwmd -- --log-rotate-max-files 0 --listen 127.0.0.1:0` -> **PASS (negative expected)**, config validation fails (exit 2)
- `cargo run -p pwmd -- --log-file-template ../escape.log --listen 127.0.0.1:0` -> **PASS (guard behavior)**, logs degraded to console-only; process then manually stopped.

Additional non-slice regression check:
- `cargo test -p pwmd -- --nocapture` -> **FAIL** (112 passed, 3 failed; unrelated to Slice 17 logging)
  - `tx_policy::tests::burn_mark_guard_allows_same_shard_beneficiary`
  - `tx_policy::tests::burn_mark_guard_rejects_policy_invalid_beneficiary`
  - `tx_policy::tests::export_guard_rejects_policy_invalid_recipient`

## Cleanup

Background `pwmd` process started during runtime smoke was terminated and verified as cleaned up.
