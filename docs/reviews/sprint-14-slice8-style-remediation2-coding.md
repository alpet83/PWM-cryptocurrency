# Sprint 14 Slice8 Style Remediation 2 (coding)

Date: 2026-04-28

Final rename pass for long production symbols in `crates/pwm-cli/src/bruteforce.rs`:

- `brute_force_domain_flags_with_progress_from_index` -> `brute_force_from_index`
- `brute_force_domain_flags_with_progress_and_match_policy` -> `brute_force_with_policy`
- `brute_force_domain_flags_with_progress_from_index_and_match_policy` -> `brute_force_index_policy`

Notes:

- Behavior kept unchanged; only symbol names and call sites were updated.
- Added concise Rust doc comments to the renamed public functions to keep semantic nuance.
- Updated usages/imports in `crates/pwm-cli/src/main.rs` and `bruteforce` tests.

Verification:

- `cargo fmt` passed.
- `cargo test -p pwm-cli` passed (`125 passed; 0 failed`).
