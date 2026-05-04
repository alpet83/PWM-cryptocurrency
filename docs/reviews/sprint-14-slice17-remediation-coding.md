# Sprint 14 — Slice 17 remediation coding note

## Scope
- Remediated logger rotation IO-error handling in `crates/pwmd/src/logging.rs`.
- Kept change set narrow to file-logger behavior and related docs/tests.

## Implemented fixes
- Rotation now treats retention operations (`remove_file`, `rename`) as strict IO steps; errors are no longer ignored.
- Prevented silent truncation risk after failed rotate: active file is not truncated if rotate step fails.
- Mode-specific behavior is explicit:
  - `required`: fail hard on runtime file-sink write/rotate/flush failures.
  - `on`: deterministic degrade to console-only with one-time warning when file sink fails.
- Startup path remains mode-aware:
  - `required`: file sink setup error fails startup.
  - `on`: setup error degrades to console-only with warning.

## FS error-path tests
- Added deterministic fault-injection hooks in logger tests for `rename` failure.
- Added tests:
  - failed rotate does not truncate active log;
  - `on` mode degrades deterministically after rotate failure;
  - `required` mode fails hard (panic) after rotate failure.

## Notes
- Retention contract is now fail-fast under IO errors instead of best-effort mutation.
- No API surface changes; remediation is internal to logging behavior.
