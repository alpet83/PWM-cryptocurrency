# Sprint 15 S3.9 Testing

## Verdict
`PASS` for focused gate.

## Commands
- `cargo fmt`
- `cargo check -p pwmd`
- `cargo test -p pwmd ledger`
- `cargo test -p pwmd v1_status_exposes_cross_shard_summary`
- `cargo test -p pwmd v1_tx_export_rollback_keeps_sender_state_on_snapshot_fail`
- `cargo test -p pwmd v1_tx_returns_500_when_snapshot_save_fails`
- `cargo test -p pwmd snapshot_roundtrip_preserves_cross_shard_summary`

## Notes
- `pwm-testing` subagent could not start because the API quota was exhausted, so the focused gate was run manually.
- The rollback regression test now proves failed snapshot save does not leave phantom export facts in `cross_shard`.
- Full two-node live peer e2e remains useful before closing the sprint, but the blocking review issues were covered by focused tests.
