# Sprint 14: address generation checkup (testing)

Verdict: **PASS** for requested scenarios in `pwm-cli`.

## Scope validated

- `addr-bruteforce` keeps existing wallet and appends by default (no replacement).
- `addr-derive --wallet-out` create behavior and append path via shared wallet persist flow.
- `wallet account remove` guardrails:
  - rejects removing last account;
  - deterministically switches active account when active one is removed.
- CLI parse/help sanity for:
  - `--overwrite-wallet`;
  - `wallet account remove`;
  - `addr-derive --wallet-out`.

## Evidence (targeted tests)

Command:

`cargo test -p pwm-cli <filter>`

Executed filters (all passed):

- `addr_bruteforce_wallet_persist_appends_existing_wallet_by_default`
- `addr_derive_wallet_out_creates_new_wallet_when_missing`
- `wallet_account_remove_rejects_last_account`
- `wallet_account_remove_active_switches_to_deterministic_fallback`
- `addr_bruteforce_cli_parses_overwrite_wallet_flag`
- `wallet_account_remove_cli_parsing`
- `addr_derive_cli_parses_wallet_out`

## Evidence (CLI help sanity)

Commands (all passed, help rendered as expected):

- `cargo run -p pwm-cli -- --help`
- `cargo run -p pwm-cli -- addr-bruteforce --help` (contains `--overwrite-wallet` with append-by-default wording)
- `cargo run -p pwm-cli -- addr-derive --help` (contains `--wallet-out`)
- `cargo run -p pwm-cli -- wallet account remove --help` (contains guardrail wording about last account)

## Full suite run

Command:

`cargo test -p pwm-cli`

Result:

- **117 passed**, **0 failed**, **0 ignored**.
- Runtime: ~55s.

## Notes

- `addr-derive --wallet-out` append semantics are validated through the same shared persistence path used by generation commands (`persist_wallet_account_output`), while create mode has its dedicated test.
