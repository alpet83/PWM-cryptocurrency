# Sprint 14 — Slice 18 — Remediation 5 (testing)

## Scope

Repository: `P:/opt/docker/PWM-cryptocurrency`

Validated mini-fix requirements:
1. progress CR lines are excluded from file sink robustly;
2. `~UT` placeholder works with UTC `HH:MM:SS.mmm`;
3. no regressions in `pwmd` logging tests.

## Commands run

```powershell
cargo test -p pwmd logging::tests::file_sink_skips_progress_lines_for_cr_variants -- --nocapture
cargo test -p pwmd logging::tests::template_expands_ut_placeholder -- --nocapture
cargo test -p pwmd logging::tests -- --nocapture
cargo test -p pwmd config::tests::logging_bounds_reject_invalid_values -- --nocapture
cargo test -p pwmd config::tests::logging_defaults_match_slice18_template -- --nocapture
```

## Results by requirement

### 1) Progress CR lines excluded from file sink robustly
- **PASS**.
- Focused test passed: `logging::tests::file_sink_skips_progress_lines_for_cr_variants`.
- Test explicitly covers CR variants in file sink input: `\r`, `\r\n`, and `\r` with trailing whitespace/newline; regular non-progress line remains in file, progress `%d` line is absent.

### 2) `~UT` placeholder resolves to UTC `HH:MM:SS.mmm`
- **PASS**.
- Focused test passed: `logging::tests::template_expands_ut_placeholder`.
- Assertions confirm `~UT` is replaced, output format length matches `00:00:00.000`, and separators are at expected positions (`:` `:` `.`), consistent with `HH:MM:SS.mmm`.

### 3) No regressions in `pwmd` logging tests
- **PASS**.
- Regression run passed: `cargo test -p pwmd logging::tests -- --nocapture`.
- Result: `18 passed; 0 failed` in `logging::tests` module.
- Additional logging-related config checks also passed:
  - `config::tests::logging_bounds_reject_invalid_values`
  - `config::tests::logging_defaults_match_slice18_template`

## Verdict

**VERDICT: PASS.**  
Remediation 5 validation is successful: CR-progress filtering is robust, `~UT` placeholder behavior matches UTC millisecond format contract, and `pwmd` logging test suite shows no regressions.
