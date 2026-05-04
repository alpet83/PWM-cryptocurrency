# Sprint 14 Slice 2 Testing (pwm-cli wallet account list/add/use)

## Scope
- Repository: `P:/opt/docker/PWM-cryptocurrency`
- Target: `pwm-cli` wallet account commands (`list`, `add`, `use`) and nearby wallet regressions.

## What Ran
1. `cargo fmt --all -- --check`
   - Result: **PASS**
2. `cargo test -p pwm-cli` (pre-change validation)
   - Duration: ~68.9s
   - Result: **PASS**
   - Summary: `100 passed; 0 failed; 0 ignored`
3. `cargo fmt --all`
   - Result: **PASS**
4. `cargo test -p pwm-cli` (after adding regression test)
   - Duration: ~66.4s
   - Result: **PASS**
   - Summary: `101 passed; 0 failed; 0 ignored`

## Coverage Evidence (Slice 2 focus)
- **Encrypted v3 account add with `--wallet-passphrase`**
  - Added `tests::wallet_account_add_cli_parsing_with_wallet_passphrase` in `crates/pwm-cli/src/main.rs`.
  - Verifies global `--wallet-passphrase` is parsed and preserved for `wallet account add`.
- **Encrypted v3 account add behavior**
  - Confirmed existing `wallet::tests::wallet_account_add_encrypted_v3_requires_passphrase` in `crates/pwm-cli/src/wallet.rs` is passing.
  - Covers success with valid passphrase and failures for missing/wrong passphrase.
- **v3 merge-save preserves `wallet_created_at_unix_sec` and unknown keys after add/use**
  - Confirmed existing `wallet::tests::wallet_v3_account_rewrite_preserves_unknown_and_created_metadata` in `crates/pwm-cli/src/wallet.rs` is passing.
  - Covers add + use flow and checks retained metadata/unknown keys in saved YAML.
- **List prints all accounts with active marker**
  - Covered by existing `tests::wallet_account_list_line_marks_active_entry` in `crates/pwm-cli/src/main.rs`.
  - Verifies active line starts with `*` and inactive line starts with space marker.
- **Add derives from same master seed and persists**
  - Covered by existing `wallet::tests::wallet_account_add_derives_and_persists_new_account` in `crates/pwm-cli/src/wallet.rs`.
  - Verifies deterministic derivation and persisted account entry.
- **Use switches active account**
  - Covered by existing `wallet::tests::wallet_account_use_switches_active_account` (still passing).
- **v2 wallet rejected with clear error**
  - Covered by existing `wallet::tests::wallet_account_commands_reject_v2_wallet` in `crates/pwm-cli/src/wallet.rs`.
  - Asserts exact message: `wallet account commands require schema v3 wallet file`.

## Regression Checks
- Existing wallet command parsing and wallet behavior tests in `pwm-cli` remained green after changes.
- No regressions observed in `wallet account list/use/add`, `wallet backup/recover/show`, and existing `pwm-cli` test suite.

## Bugs Found
- **None** in this testing slice.

## Files Changed
- `crates/pwm-cli/src/main.rs`
- `docs/reviews/sprint-14-slice2-testing.md`
