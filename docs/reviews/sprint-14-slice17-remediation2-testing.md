# Sprint 14 — Slice 17 Remediation2 Testing

Date: 2026-04-29  
Repository: `P:/opt/docker/PWM-cryptocurrency`  
Mode: focused retest after deadlock remediation

## Verdict
`approve`

## Scope verified
1. No lock-order deadlock in status + tx/finalize/seal flows (focused checks).
2. Previous logger rotation remediation still passes.
3. TUI style rename (`inter_shard_cli_route_message` -> `shard_cli_hint`) does not regress behavior/tests.

## Commands and results

1) Deadlock-focused (`status + tx`)
- Command: `cargo test -p pwmd tests::v1_status_and_tx_do_not_deadlock_with_snapshot_persist -- --exact --nocapture`
- Result: **PASS**
- Duration: ~6.81s
- Evidence: `1 passed; 0 failed`; concurrent `/v1/status` + `/v1/tx` timeout-guarded smoke remained green.

2) Finalize flow (post-remediation regression)
- Command: `cargo test -p pwmd tests::v1_roaming_intent_finalize_sets_relayed_and_is_idempotent -- --exact --nocapture`
- Result: **PASS**
- Duration: ~4.32s
- Evidence: finalize semantics/idempotency test green.

3) Finalize lifecycle trace path
- Command: `cargo test -p pwmd tests::v1_flow_recent_includes_roaming_finalize_lifecycle_events -- --exact --nocapture`
- Result: **PASS**
- Duration: ~5.60s
- Evidence: lifecycle events around finalize remain consistent.

4) Sync seal path (`tx` export/import -> head advance)
- Command: `cargo test -p pwmd tests::v1_tx_http_export_import_advances_head_height_via_sync_seal -- --exact --nocapture`
- Result: **PASS**
- Duration: ~7.27s
- Evidence: sync-seal progression remains stable.

5) Logger rotation remediation pack
- Command: `cargo test -p pwmd logging::tests:: -- --nocapture`
- Result: **PASS**
- Duration: ~5.01s
- Evidence: `9 passed; 0 failed`, including:
  - `rotate_error_does_not_truncate_active_log`
  - `on_mode_degrades_after_rotate_error`
  - `required_mode_panics_after_rotate_error`
  - `rotation_triggers_and_keeps_retention_cap`

6) TUI style-rename behavior guard
- Command: `cargo test -p pwm-tui tests::inter_shard_cli_route_message_mentions_export_import_steps -- --exact --nocapture`
- Result: **PASS**
- Duration: ~4.75s
- Evidence: message behavior tied to renamed helper remains green (`1 passed; 0 failed`).

## Notes
- Hang watchdog: **not triggered** (all commands completed normally within timeout windows).
- Process cleanup: **cleaned: yes** (no persistent background daemons spawned in this run).
