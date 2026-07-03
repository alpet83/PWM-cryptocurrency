# Sprint 14 — Slice 17 Remediation3 Testing

Date: 2026-04-29  
Repository: `P:/opt/docker/pwm-protocol`  
Mode: focused pwmd logging retest after style-contract remediation

## Verdict
`approve`

## Scope verified
1. Custom formatter output shape matches style contract.
2. Color palette mapping works for `WARN`/`ERROR` and numeric values in TTY mode.
3. `NO_COLOR` overrides ANSI output.
4. Numeric exclusion rules for timestamp/id/hash hold.
5. Existing logging rotation/template tests still pass.

## Commands and results

1) Focused pwmd logging suite
- Command: `cargo test -p pwmd logging::tests::`
- Result: **PASS**
- Duration: ~0.77s wall time (`finished in 0.01s` test execution)
- Evidence: `14 passed; 0 failed; 0 ignored`

## Evidence by verification point

1) Formatter contract shape
- `logging::tests::formatter_plain_contract` — **PASS**

2) Palette mapping (`WARN`/`ERROR`) + numeric highlight in TTY/ANSI path
- `logging::tests::formatter_colors_warn_and_error_tags` — **PASS**
- `logging::tests::numeric_highlight_applies_to_message_and_values` — **PASS**

3) `NO_COLOR` override
- `logging::tests::no_color_disables_ansi_even_in_tty` — **PASS**

4) Numeric exclusion for timestamp/id/hash-like tokens
- `logging::tests::numeric_highlight_skips_hash_like_tokens` — **PASS**

5) Rotation/template regression guards
- `logging::tests::rotation_triggers_and_keeps_retention_cap` — **PASS**
- `logging::tests::on_mode_degrades_after_rotate_error` — **PASS**
- `logging::tests::required_mode_panics_after_rotate_error` — **PASS**
- `logging::tests::rotate_error_does_not_truncate_active_log` — **PASS**
- `logging::tests::template_expands_subdir_placeholders` — **PASS**
- `logging::tests::template_rejects_parent_dir` — **PASS**
- `logging::tests::template_rejects_absolute` — **PASS**

## Notes
- Hang watchdog: **not triggered** (single focused run completed normally).
- Process cleanup: **cleaned: yes** (no background daemons were spawned in this run).
