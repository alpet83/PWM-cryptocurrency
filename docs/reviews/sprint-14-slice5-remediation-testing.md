# Sprint 14 — Slice 5 remediation testing (`--upgrade-wallet`)

Date: 2026-04-28  
Repo: `P:/opt/docker/PWM-cryptocurrency`

## Scope

Independent validation for remediation behavior of `--upgrade-wallet`:

1. Without `--upgrade-wallet`, v2 wallet load path does not rewrite file.
2. With `--upgrade-wallet`, v2 migrates to v3 and persists.
3. CLI parsing for `--upgrade-wallet`.
4. TUI args parsing for `--upgrade-wallet`.
5. Regression sanity for `pwm-cli` and `pwm-tui`.

## Evidence

### 1) No rewrite without `--upgrade-wallet`

- Command:  
  `cargo test -p pwm-cli wallet::tests::load_wallet_yaml_v2_without_upgrade_flag_does_not_rewrite_file -- --exact`
- Result: **PASS**
- Observed test output:
  - `running 1 test`
  - `test wallet::tests::load_wallet_yaml_v2_without_upgrade_flag_does_not_rewrite_file ... ok`
  - `test result: ok. 1 passed; 0 failed`
- Contract asserted in test:
  - `before_raw == after_raw` (no on-disk rewrite)
  - on-disk schema remains `2` after load path without upgrade flag

### 2) Migration + persist with `--upgrade-wallet`

- Command:  
  `cargo test -p pwm-cli wallet::tests::load_wallet_yaml_with_upgrade_flag_migrates_encrypted_v2_to_v3 -- --exact`
- Result: **PASS**
- Observed test output:
  - `running 1 test`
  - `test wallet::tests::load_wallet_yaml_with_upgrade_flag_migrates_encrypted_v2_to_v3 ... ok`
  - `test result: ok. 1 passed; 0 failed`
- Contract asserted in test:
  - load returns schema `3`
  - migrated wallet remains decryptable with original passphrase
  - persisted on-disk schema detected as `3`

### 3) CLI parsing for `--upgrade-wallet`

- Command:  
  `cargo test -p pwm-cli tests::tx_send_cli_parses_upgrade_wallet_flag -- --exact`
- Result: **PASS**
- Observed test output:
  - `running 1 test`
  - `test tests::tx_send_cli_parses_upgrade_wallet_flag ... ok`
  - `test result: ok. 1 passed; 0 failed`
- Coverage:
  - verifies top-level CLI parser sets `cli.upgrade_wallet == true` when flag provided

### 4) TUI args parsing for `--upgrade-wallet`

- Command:  
  `cargo test -p pwm-tui tests::args_parse_upgrade_wallet_flag -- --exact`
- Result: **PASS**
- Observed test output:
  - `running 1 test`
  - `test tests::args_parse_upgrade_wallet_flag ... ok`
  - `test result: ok. 1 passed; 0 failed`
- Coverage:
  - verifies `Args::parse_from(["pwm-tui", "--upgrade-wallet"])` sets `args.upgrade_wallet == true`

### 5) Regression sanity

- Command: `cargo test -p pwm-cli`
  - Result: **PASS** (`104 passed; 0 failed`)
  - Duration: ~61.62s
- Command: `cargo test -p pwm-tui`
  - Result: **PASS** (`71 passed; 0 failed`)
  - Duration: ~6.80s

## Totals

- Targeted checks: **4 passed / 0 failed**
- Regression sanity: **175 passed / 0 failed** (`104 + 71`)
- Combined executed tests in this run: **179 passed / 0 failed**

## Bugs / regressions found

- **None observed** in this independent remediation test pass.
