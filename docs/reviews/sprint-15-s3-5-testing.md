# Sprint 15 S3.5 testing: one-window relay polish/docs

Date: 2026-04-30  
Repository: `P:/opt/docker/pwm-protocol`  
Verdict: **PASS**

## Scope Checked

- Focused CLI/TUI tests after help/message/doc updates.
- Source-only one-window expectations in docs/artifacts.
- Manual fallback wording for trusted peer context.
- S15-S4 snapshot/DB boundary for the S3.5 touched-file set.

## Results

- **PASS**: focused CLI tests still pass:
  - `tests::tx_import_note_requires_initialized_target`
  - `tests::cli_help_names_raw_pwm_units`
  - `tests::recipient_preflight_blocks_missing_account`
- **PASS**: focused TUI tests still pass:
  - `tests::inter_shard_cli_route_message_mentions_export_import_steps`
  - `tests::format_submit_transfer_error_adds_inter_shard_hint_for_policy_reject`
- **PASS**: docs/artifacts describe the one-window happy path as source/native RPC only: client submits to source, and source `pwmd` reaches target through configured trusted seed peer context.
- **PASS**: manual fallback docs state that target-side `tx-handoff-register` is not open/no-seed and requires trusted source peer context before `tx-import`.
- **PASS with caveat**: the S3.5 coding report/task/touched-file set keeps S15-S4 snapshot/DB work out of scope. The broader working tree is already dirty and includes unrelated snapshot persistence changes (`crates/pwmd/src/snapshot.rs`, `docs/pwmd.md` snapshot sections), so this is not a clean-branch assertion.

## Commands Run

- `cargo test -p pwm-cli tx_import_note_requires_initialized_target` -> PASS (`1 passed`).
- `cargo test -p pwm-cli cli_help_names_raw_pwm_units` -> PASS (`1 passed`).
- `cargo test -p pwm-cli recipient_preflight_blocks_missing_account` -> PASS (`1 passed`).
- `cargo test -p pwm-tui inter_shard_cli_route_message_mentions_export_import_steps` -> PASS (`1 passed`).
- `cargo test -p pwm-tui format_submit_transfer_error_adds_inter_shard_hint_for_policy_reject` -> PASS (`1 passed`).
- `cargo fmt -- --check` -> PASS.

Note: an initial dry run with `-- --exact` matched `0` tests because the filters were not module-qualified; it was discarded and rerun without `--exact`.

## Cleanup

- Long-lived processes started: none.
- Process cleanup needed: no.
- Artifact cleanup: not run; only focused incremental cargo test/check commands were executed.

## Final Verdict

**PASS**: S15-S3.5 one-window relay polish/docs satisfy the requested validation checks. The only residual caveat is the pre-existing dirty working tree containing broader snapshot work outside the S3.5 touched-file set.
