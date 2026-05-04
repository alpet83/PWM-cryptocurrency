# Sprint 14 Slice 3 Testing Evidence

Date: 2026-04-28
Repository: `P:/opt/docker/PWM-cryptocurrency`
Scope: independent testing for Slice 3 (owner rows in TUI left panel, active owner highlight/index, v2 fallback behavior, no obvious secret leakage in owner rows path).

## Executed commands

Run via `user-cqds_mcp_mini` `cq_process_ctl` (`host=true`, `cwd=P:\opt\docker\PWM-cryptocurrency`):

1. `cargo test -p pwm-tui` (baseline before changes)
2. `cargo test -p pwm-tui owner_and_receivers`
3. `cargo test -p pwm-tui`
4. `cargo fmt`

## Pass/fail counts

- Baseline full run: `68 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`
- Targeted owner/receiver subset: `5 passed; 0 failed; 0 ignored; 0 measured; 64 filtered out`
- Final full run after test addition: `69 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`

## Coverage notes for Slice 3

- **v3 owned accounts in left owner panel path:** covered by `owner_and_receivers_uses_all_owned_accounts_and_marks_active_index`.
- **Active account marked/highlighted path:** covered by returned `active_owner_idx` assertions and render-path logic consuming it.
- **v2 fallback preserved:** added `owner_and_receivers_seed_fallback_keeps_v2_style_split` to assert first-account owner + remaining receivers split.
- **No obvious secret leakage in rendered owner rows data path:** owner rows are produced from `AcctRow` data (`id`, balance, label path); wallet secret field (`secret_payload_plaintext`) is not used by `owner_and_receivers` or owner-row rendering path. No leakage observed in tests/run output.

## Bugs found

- No functional regressions found in tested Slice 3 scope.

## Changed files

- `crates/pwm-tui/src/main.rs` (added one unit test)
- `docs/reviews/sprint-14-slice3-testing.md` (this evidence)
