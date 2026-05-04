## Sprint 14 Slice 8 Style Remediation (Coding)

- Remediated style gate by renaming long production identifiers to `<=4` words:
  - `assert_tx_recipient_in_wallet_address_book` -> `assert_tx_recipient_allowed`
  - `load_wallet_yaml_with_upgrade` -> `load_wallet_yaml_upgrade`
  - `to_wallet_yaml_with_metadata` -> `build_wallet_yaml`
  - `format_addr_bruteforce_progress_line` -> `fmt_addr_bruteforce_progress`
  - `format_addr_bruteforce_result_lines` -> `fmt_addr_bruteforce_results`
- Updated all call sites in `pwm-cli` (including tests) with behavior unchanged.
- Added short doc comments on renamed helpers where extra nuance is useful.
