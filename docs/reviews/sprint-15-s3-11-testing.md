# Sprint 15 S3.11 Testing

## Verdict
`PASS` (focused gate)

## Commands
- `cargo check -p pwmd`
- `cargo check -p pwm-tui`
- `cargo test -p pwmd v1_status_exposes_cross_shard_summary`
- `cargo test -p pwm-tui preflight_selected_initialized_allows_initialized_or_missing_row`
- `cargo test -p pwm-tui preflight_known_recipient_blocks_uninitialized_known_row`

## Notes
- During gate execution, test fixtures for `Inner` required update for new `peer_account_views` field.
- After fixture remediation and formatting, focused checks/tests passed.
