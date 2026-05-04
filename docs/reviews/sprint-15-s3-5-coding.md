# Sprint 15 S3.5 coding report

Date: 2026-04-30

## Scope

- Audited the S15-S3.4 one-window relay follow-ups, especially the noted source-only `tx-import` preflight gap.
- Kept S15-S4 snapshot/DB scope untouched.
- Updated CLI/TUI wording and operator docs so the happy path is source/native node only, with target reached by `pwmd` through trusted configured seed peer context.

## Audit result

- `pwm-cli tx-import` already preflights recipient initialization on the current RPC before signing/submitting import. Because `tx-import` is the manual target-side command, this is the correct target RPC preflight and the source-only gap is no longer present.
- Cross-domain `tx-send` remains source-only by design. It cannot preflight the target recipient from the CLI window, so it keeps an explicit note that target relay/import rejects missing or uninitialized recipients.
- `tx-handoff-register` remains manual fallback, but target registration now requires trusted source peer context from configured seed connectivity. Open/no-seed registration is not documented as valid.

## Files changed

- `crates/pwm-cli/src/main.rs`
- `crates/pwm-tui/src/main.rs`
- `docs/pwmd.md`
- `docs/pwm-cli.md`
- `docs/pwm-tui.md`
- `docs/ROAMING-SAMPLE.md`
- `docs/tester-guide-cli-tui-scenarios.md`
- `docs/reviews/sprint-15-s3-5-coding.md`

## Behavior/docs change

- CLI help/notes now mention target trusted seed context for manual handoff/import.
- TUI inter-shard hints now point operators to source-side roaming-intent relay first and trusted target fallback second.
- Docs/runbooks now describe the user happy path as `client -> native/source pwmd -> trusted seed target peer`, without requiring target RPC from the user.
- Manual fallback docs now call out that target-side `tx-handoff-register` requires trusted peer context, not arbitrary self-attested handoff registration.

## Commands

- `cargo fmt` -> PASS.
- `cargo check -p pwm-cli -p pwm-tui` -> PASS.
- `cargo test -p pwm-cli tx_import_note_requires_initialized_target` -> PASS.
- `cargo test -p pwm-cli cli_help_names_raw_pwm_units` -> PASS.
- `cargo test -p pwm-cli recipient_preflight_blocks_missing_account` -> PASS.
- `cargo test -p pwm-tui inter_shard_cli_route_message_mentions_export_import_steps` -> PASS.
- `cargo test -p pwm-tui format_submit_transfer_error_adds_inter_shard_hint_for_policy_reject` -> PASS.

## Remaining limits

- One-window relay still cannot prove target recipient initialization from the CLI source-only window before submit; target relay/import remains the fail-closed authority.
- Full mesh/discovery and S15-S4 snapshot/DB work remain out of scope.
