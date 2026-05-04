# Sprint 14 — Slice 17 logger contract review

## Verdict
`approve with nits`

## Recommended implementation (pwmd)
- TTY color only when `console-color=auto` and console is TTY.
- Non-TTY: plain console, full log stream to file sink.
- File rotation by size with retention cap.
- Configurable template path with placeholders (`{date}`, `{time}`, `{log_name}`, ...).

## Proposed config surface
- `--log-name` / `PWM_LOG_NAME`
- `--log-dir` / `PWM_LOG_DIR`
- `--log-file-template` / `PWM_LOG_FILE_TEMPLATE`
- `--log-file` (`on|off|required`) / `PWM_LOG_FILE`
- `--log-console-color` (`auto|always|never`) / `PWM_LOG_CONSOLE_COLOR`
- `--log-rotate-size-mb` / `PWM_LOG_ROTATE_SIZE_MB`
- `--log-rotate-max-files` / `PWM_LOG_ROTATE_MAX_FILES`

## Safety rules
- Template expands only to relative paths within log root.
- Reject `..`, absolute paths, drive prefixes.
- Bounds for rotation settings with startup validation.

## Nits
- Уточнить product default для `--log-file` (`on` vs `required`) по profile.
- Явно зафиксировать, что значит “full logs” относительно `RUST_LOG` фильтра.
