# Sprint 14 — Slice 17 coding report

## Scope completed
- Added logger integration in `pwmd` with explicit config surface for console/file sinks.
- Implemented console color policy `auto|always|never` with TTY-aware `auto`.
- Added file sink with size-based rotation and retention cap (`max files`).
- Added safe template expansion for log file paths with placeholders:
  - `{date}`, `{time}`, `{datetime}`, `{log_name}`, `{pid}`
  - supports subdirectories (e.g. `{date}/{log_name}-{time}.log`)
  - rejects absolute paths, drive prefixes, and `..` traversal.

## Config surface
- CLI/env added:
  - `--log-name` / `PWM_LOG_NAME`
  - `--log-dir` / `PWM_LOG_DIR`
  - `--log-file-template` / `PWM_LOG_FILE_TEMPLATE`
  - `--log-file` / `PWM_LOG_FILE` (`on|off|required`)
  - `--log-console-color` / `PWM_LOG_CONSOLE_COLOR` (`auto|always|never`)
  - `--log-rotate-size-mb` / `PWM_LOG_ROTATE_SIZE_MB`
  - `--log-rotate-max-files` / `PWM_LOG_ROTATE_MAX_FILES`
- `RUST_LOG` behavior preserved via `tracing_subscriber::EnvFilter::from_default_env()`.
- Startup `eprintln!` visibility preserved.

## Tests added
- Template expansion, placeholder handling, and safety checks.
- Rotation trigger and retention cap behavior.
- Mode parsing and non-TTY-safe color policy checks.
- Logging bounds validation checks.
