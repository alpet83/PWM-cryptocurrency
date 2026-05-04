# Sprint 14 Slice 28 Coding Review

## Scope

- Removed the stale wallet active/default marker from TUI Owner rows; runtime cursor selection is now represented by the table highlight only.
- Changed TUI wallet signing to derive selected owner keys from the wallet master seed before falling back to root `signing_key_hex`, so selected account metadata drives signing.
- Added a regression for derivation index `105053` with a non-zero low byte and stale root signing key material.

## Verification

- Passed: `cargo fmt`
- Passed: `cargo test -p pwm-tui`
- Skipped: targeted CLI/wallet tests; only TUI signing flow changed, and CLI derivation convention was left untouched.

