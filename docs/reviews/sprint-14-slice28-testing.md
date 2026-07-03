# Sprint 14 Slice28 Testing

## Verdict

`PASS`

Slice28 coding is accepted for the requested automated checks. `pwm-tui` tests pass, the Owner table no longer formats a stale `*` marker, and the selected-owner signing regression covers derivation index `105053` with stale flattened root signing material.

## Required Checks

1. `cargo test -p pwm-tui`: PASS.
   - Result: 82 passed, 0 failed, 0 ignored; finished in 5.47s.
   - Harness duration: 6.6s, timeout: 120s.
2. Owner rows no longer render stale `*` active/default marker: PASS.
   - Render path now builds Owner cells with `format_acct_cell(r)` only.
   - `format_acct_cell` emits either `label | address` or `address`; it does not read `OwnedWalletAccount::is_active` and does not prefix `*`.
   - Selection remains represented by the reversed row style.
3. Selected-owner signing derives from master seed before stale root signing key fallback: PASS.
   - `signing_material_for_sender` resolves the selected owned account metadata, calls `wallet_seed_opt`, and derives with `derive_wallet_key(seed, index, domain, from)` before looking at `w.signing_key`.
   - Flattened `w.signing_key` is only used when no master seed is available.
4. Regression for index `105053` / CY-FB / stale DO root signing key: PASS.
   - `cargo test -p pwm-tui selected_default_index_105053_derives_from_seed_before_stale_key -- --nocapture`
   - Result: 1 passed, 0 failed, 81 filtered out; finished in 0.00s.
   - The test asserts a non-zero low byte for index `105053`, provides a stale flattened signing key, and verifies the derived signing identity still matches the selected account.
5. Bounded `tmp/genesis.yaml` passphrase `1234` check: PARTIAL / bounded.
   - `PWM_WALLET_PASSPHRASE=1234 cargo run -q -p pwm-cli -- wallet show --wallet tmp/genesis.yaml --unsafe-show-secrets`: PASS.
   - Confirmed fixture metadata selects `m/0/105053`, `domain_u16=11515`, `pwm1-CY/FB-...`, with stale flattened `signing_key_hex` still present.
   - A full automated TUI F6 proof against `tmp/genesis.yaml` was not feasible without a machine-readable TUI hook; per testing prompt, alternate-screen stdout is not a reliable assertion channel.

## Commands Run

- `cargo test -p pwm-tui` - PASS, 6.6s, timeout 120s.
- `cargo test -p pwm-tui selected_default_index_105053_derives_from_seed_before_stale_key -- --nocapture` - PASS, 1.0s, timeout 60s.
- `PWM_WALLET_PASSPHRASE=1234 cargo run -q -p pwm-cli -- wallet show --wallet tmp/genesis.yaml --unsafe-show-secrets` - PASS, 2.0s, timeout 60s.
- `target\debug\pwm-tui.exe --help` - PASS.
- `cargo fmt --check` - PASS, 1.1s.

## Notes

- CQDS process tooling could not be used from this subagent interface, so checks were run through the local shell in `P:\opt\docker\pwm-protocol`.
- `cargo run -q -p pwm-tui -- --help` failed once while trying to replace `target\debug\pwm-tui.exe` with `os error 5`; the existing binary help command passed immediately afterward.
- No `docs/MVP-checklist.md` rows were changed.
- Cleanup: yes; no `pwmd` or `pwm-tui` processes were left running.
