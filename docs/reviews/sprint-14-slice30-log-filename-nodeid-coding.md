# Sprint 14 - Slice 30 - coding report

## Scope completed

- Added `{node_id}` support in `pwmd` log filename template expansion.
- Switched default template to `{date}/{log_name}-{node_id}-{time}.log` (effective path with default `--log-dir logs` becomes `logs/{date}/{log_name}-{node_id}-{time}.log`).
- Wired `node_id` source to effective runtime identity (resolved `--node-id` / alias/neutral identity result).
- Added filesystem-safe sanitization for `{node_id}` and fallback when unavailable.
- Updated operator docs (`LOGGING_STYLE.md`, `pwmd.md`) and CLI help text.

## Implementation details

### Code

- `crates/pwmd/src/logging.rs`
  - `init_logging(...)` now accepts `runtime_node_id: Option<&str>`.
  - `expand_log_template_path(...)` now accepts runtime node id and expands `{node_id}`.
  - Added `sanitize_template_token(...)`:
    - allows only `[A-Za-z0-9._-]`,
    - replaces other chars with `_`,
    - uses `node-unknown` for empty/whitespace input.

- `crates/pwmd/src/main.rs`
  - Resolved runtime identity before logger init.
  - Passed `Some(&identity.node_id)` into `init_logging(...)` (including fallback console-only logger path).
  - Updated CLI help for `--log-file-template` placeholders and default value.

- `crates/pwmd/src/config.rs`
  - Updated `LoggingConfig::default().file_template` to `{date}/{log_name}-{node_id}-{time}.log`.
  - Updated defaults test name/assertion to slice30 template.

### Docs

- `docs/LOGGING_STYLE.md`
  - Default file pattern updated to include `{node_id}`.
  - Added note that `{node_id}` comes from runtime identity and is sanitized.

- `docs/pwmd.md`
  - Updated `--log-file-template` default and placeholder list.
  - Added explicit sanitization/fallback contract for `{node_id}`.

## Tests/checks run

- `cargo fmt` - PASS
- `cargo test -p pwmd logging::tests::template_expands_subdir_placeholders -- --nocapture` - PASS
- `cargo test -p pwmd logging::tests::template_expands_node_id_placeholder_with_sanitization -- --nocapture` - PASS
- `cargo test -p pwmd logging::tests::template_uses_node_id_fallback_when_unavailable -- --nocapture` - PASS
- `cargo test -p pwmd config::tests::logging_defaults_match_slice30_template -- --nocapture` - PASS
- `cargo check -p pwmd` - PASS

## CQDS index refresh

- Enqueued background index rebuild via `cq_files_ctl` (`project_id=5`, `background=true`).
- Result: `enqueue=duplicate` (index job already present/active in maint pool).

## Follow-ups

- `pwm-testing`: run broader regression around logging startup modes (`on/off/required`) with `{node_id}` in templates for runtime CLI scenarios.
- `pwm-review`: validate cross-platform filename safety policy for non-ASCII `node_id` (current behavior replaces non-allowed chars with `_`).
