# Sprint 14 Slice 27 - Testing

Date: 2026-04-29

## Verdict

PASS.

The wallet-level `active_account_id_hex` requirement is removed for the tested CLI/TUI paths. v3 wallets without the field load, new v3 writes omit it, and old files that still contain it do not let that marker override runtime sender selection.

## Coverage Added

- `crates/pwm-cli/src/main.rs`
  - `tests::tx_signer_uses_deterministic_v3_default_without_active_marker`
  - Covers v3 wallet signing without `active_account_id_hex`: CLI derives the signer from `master_seed_hex + derivation_index`, verifies the derived account id, and uses the deterministic default account `(derivation_index, id_hex)` when no explicit selector exists.
- `crates/pwm-tui/src/main.rs`
  - `tests::load_wallet_identity_ignores_legacy_active_marker_for_runtime_sender`
  - Covers a v3 wallet that still has legacy `active_account_id_hex`: TUI load uses the deterministic wallet account, but F6 sender still follows the selected Owner row.

## Required Checks

1. v3 wallet without `active_account_id_hex` loads in CLI and TUI: PASS.
   - CLI: `wallet::tests::load_wallet_yaml_parses_schema_v3_plaintext_minimal`.
   - TUI: `tests::load_wallet_identity_accepts_v3_without_active_account`.
2. TUI no longer fails on missing `active_account_id_hex`, and Owner selection remains sender source: PASS.
   - Covered by `tests::load_wallet_identity_accepts_v3_without_active_account`, `tests::f6_wallet_mode_uses_selected_owner_row_as_sender`, and the new legacy-active runtime sender test.
3. New v3 wallet saves omit `active_account_id_hex`: PASS.
   - Covered by `wallet::tests::save_wallet_v3_new_creates_parent_directories`.
4. CLI signing derives/verifies from master seed plus account derivation metadata and uses deterministic default safely: PASS.
   - Covered by the new `tx_signer_uses_deterministic_v3_default_without_active_marker`.
5. Existing wallets with old active field still load, but field is not authoritative for TUI runtime sender: PASS.
   - CLI: `wallet::tests::load_wallet_yaml_ignores_v3_legacy_active_account`.
   - TUI: new legacy-active runtime sender test.
6. `tmp/genesis.yaml` regression: PASS.
   - `tmp/genesis.yaml` is schema v3 encrypted and omits `active_account_id_hex`.
   - `PWM_WALLET_PASSPHRASE=1234 cargo run -p pwm-cli -- wallet show --wallet tmp/genesis.yaml` loaded it successfully and selected deterministic account `derivation_index 105053`.
   - `PWM_WALLET_PASSPHRASE=1234 cargo run -p pwm-cli -- wallet account list --wallet tmp/genesis.yaml` marked the same CY account `*`; the older user-edited shape should now load because the loader derives selection from `accounts[]` metadata instead of requiring a wallet-level active marker.

## Commands Run

- `cargo fmt -- --check` - PASS.
- `cargo test -p pwm-core load_wallet_read_header_supports_schema_v3_without_active_account` - PASS, 1 passed.
- `cargo test -p pwm-cli tx_signer_uses_deterministic_v3_default_without_active_marker` - PASS, 1 passed.
- `cargo test -p pwm-cli load_wallet_yaml_ignores_v3_legacy_active_account` - PASS, 1 passed.
- `cargo test -p pwm-cli save_wallet_v3_new_creates_parent_directories` - PASS, 1 passed.
- `cargo test -p pwm-tui load_wallet_identity_accepts_v3_without_active_account` - PASS, 1 passed.
- `cargo test -p pwm-tui load_wallet_identity_ignores_legacy_active_marker_for_runtime_sender` - PASS, 1 passed.
- `cargo test -p pwm-tui f6_wallet_mode_uses_selected_owner_row_as_sender` - PASS, 1 passed.
- `cargo test -p pwm-tui cy_selected_while_db_active_signs_cy_not_db` - PASS, 1 passed.
- `PWM_WALLET_PASSPHRASE=1234 cargo run -p pwm-cli -- wallet show --wallet tmp/genesis.yaml` - PASS.
- `PWM_WALLET_PASSPHRASE=1234 cargo run -p pwm-cli -- wallet account list --wallet tmp/genesis.yaml` - PASS.
- `cargo test -p pwm-cli` - PASS, 139 passed.
- `cargo test -p pwm-tui` - PASS, 81 passed.

Cleanup: no `pwmd` or `pwm-tui` processes were left running after testing.

## Notes

- No TUI framebuffer assertions were attempted; per TUI testing policy, behavior was covered through pure helper/unit tests instead of alternate-screen stdout capture.
- `docs/MVP-checklist.md` was not updated in this slice.
