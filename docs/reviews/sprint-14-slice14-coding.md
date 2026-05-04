# Sprint 14 - Slice 14 coding report

## Scope delivered

- Genesis naming refactor: `rows` -> `accounts` in runtime config structs, v4 genesis JSON schema, CLI output, loader, and active docs.
- Removed TUI footer hotkey hint for `F7` and removed direct `F7` handler from runtime UI loop.
- Kept behavior intact except requested naming and hotkey-hint removal.

## Runtime/code changes

- `pwm-core`:
  - `GenCfg.rows` renamed to `GenCfg.accounts`.
  - `FundingCfg.rows` renamed to `FundingCfg.accounts`.
  - Boot/runtime invariants and tests updated to `funding.accounts`.
- `pwmd`:
  - Genesis v4 loader now expects `gen_cfg.funding.accounts`.
  - Snapshot canonical field renamed `genesis_rows` -> `genesis_accounts`.
  - Snapshot validation/migration tests updated for new canonical field name.
- `pwm-cli`:
  - Genesis v4 builder emits `gen_cfg.funding.accounts`.
  - Runtime print changed from `genesis_rows` to `genesis_accounts`.
  - Focused genesis-build tests updated.
- `pwm-tui`:
  - Footer no longer advertises `F7 inter-shard->CLI`.
  - `KeyCode::F(7)` modal branch removed.
  - Cross-domain submit error still includes CLI fallback route text, but not tied to `F7`.

## Docs updated

- `docs/GENESIS_BLOCK.md`
- `docs/MVP-checklist.md`
- `docs/pwm-cli.md`
- `docs/pwmd.md`
- `docs/pwm-tui.md`

All updated docs now use `funding.accounts` / `genesis_accounts` for active contracts.

## Focused validation

- `cargo fmt`
- `cargo test -p pwm-core chain::tests::seal_empty_block`
- `cargo test -p pwm-core chain::tests::seal_allows_one_val_many_funding`
- `cargo test -p pwm-cli genesis_build_`
- `cargo test -p pwmd genesis_json_v4_`
- `cargo test -p pwmd snapshot_`
- `cargo test -p pwm-tui status_footer_line_rpc_offline_leads_then_poll_err`

Result: all listed commands passed.
