# Sprint 14 Slice 18 — remediation 4 coding

## Scope

- Re-aligned `pwmd` formatter palette to match `docs/LOGGING_STYLE.md`.
- Added file-sink guard to ignore progress lines that end with carriage return (`\r`).
- Kept console flow unchanged for progress output behavior.

## Code updates

- `crates/pwmd/src/logging.rs`
  - added message tint mapping: `INFO` -> light blue, stage-level (`TRACE`/`DEBUG`) -> yellow;
  - kept numeric highlighter bright purple and preserved it inside tinted messages;
  - added field value palette: numeric -> bright purple, string -> light green, JSON-like structures -> white;
  - added `is_progress_line` check in `RotatingGuard::write` so trailing-`\r` lines are skipped by file sink.

## Tests

- Added formatter palette checks for info/stage message tint and field-value classes.
- Added explicit file-sink regression test:
  - input line `#INFO: sealed height=%d    \r` is not persisted to file.
