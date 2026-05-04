# Sprint 14 Slice28 Remediation Coding

## Changes
- Added an encrypted-v3 regression where the decrypted payload carries the master seed for the selected `m/0/105053` account while the flattened/root signing key belongs to a different account.
- Updated `docs/pwm-tui.md` to clarify that a missing master seed blocks non-root/multi-account derivation, while verified legacy/root-key fallback is limited to compatible root/default cases.

## Results
- `cargo fmt` passed.
- `cargo test -p pwm-tui` passed: 83 tests.

## Notes
- No commit was created.
