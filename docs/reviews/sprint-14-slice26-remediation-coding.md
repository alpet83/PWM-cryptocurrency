# Sprint 14 Slice26 Remediation Coding

## Fix

- Tightened wallet selected-owner signing in `pwm-tui`: the active wallet shortcut now verifies the held signing key against the selected account id, derivation index, and domain before returning it.
- Kept active and non-active wallet selected-account signing on the same invariant helper: the key material plus derivation metadata must reconstruct the selected account id.
- F6 send pre-submit now reports selected-owner signing failures as `F6 send blocked: ...`, so mismatched wallet material is blocked before submit.
- Added a regression for active CY selected while the unlocked payload signing key belongs to DB; the path returns a signing/pre-submit error and does not produce DB or synthetic sender material.

## Results

- `cargo fmt` passed.
- `cargo test -p pwm-tui` passed: 80 tests.
- `cargo check -p pwm-tui` passed.

## Notes

- CQDS project 5 was selected, but its shell image did not have `cargo`; Rust commands were run in the local checkout.
