# Sprint 14 — Slice 11 testing (decoupled genesis v4)

## Verdict
`pass`

Slice 11 decoupled genesis redesign validated for the requested scope: focused tests across `pwm-core`, `pwmd`, and `pwm-cli` passed and match the intended v4 contract.

## Scope and checks

1. **v4 decoupled schema end-to-end: 1 validator + N funding rows**
   - Covered by `pwm-core` test `chain::tests::seal_allows_one_val_many_funding`.
   - Result: pass (`1 passed; 0 failed`).

2. **Producer rotation uses validators set length (not funding rows length)**
   - Covered by `pwm-core` test `chain::tests::prod_rotation_uses_vals_len`.
   - Result: pass (`1 passed; 0 failed`).

3. **Reward default behavior deterministic and sensible**
   - Covered by `pwm-core` test `chain::tests::reward_default_is_deterministic`.
   - Result: pass (`1 passed; 0 failed`).

4. **`pwm-cli genesis-build` emits decoupled v4 bundle**
   - Covered by `pwm-cli` tests:
     - `tests::genesis_build_generates_decoupled_v4_bundle`
     - `tests::genesis_build_cli_parses_required_flags`
   - Result: pass (`1 passed` + `1 passed`; no failures).

5. **Strict schema behavior (v4-only) and clear old-format errors**
   - Covered by `pwmd` tests:
     - `tests::genesis_json_rejects_unsupported_schema_version`
     - `tests::genesis_json_v4_roundtrip_encrypted_validator_key`
     - `tests::genesis_json_v4_rejects_wrong_passphrase`
     - `tests::genesis_json_v4_rejects_malformed_payload`
     - `tests::genesis_json_v4_rejects_path_mismatch`
     - `tests::genesis_json_v4_rejects_extreme_kdf_iters`
   - Result: all pass, with explicit v4-only rejection behavior validated.

## Command output summary

- `cargo test -p pwm-core chain::tests::seal_allows_one_val_many_funding`
  - `test ... ok`
  - `test result: ok. 1 passed; 0 failed`

- `cargo test -p pwm-core chain::tests::prod_rotation_uses_vals_len`
  - `test ... ok`
  - `test result: ok. 1 passed; 0 failed`

- `cargo test -p pwm-core chain::tests::reward_default_is_deterministic`
  - `test ... ok`
  - `test result: ok. 1 passed; 0 failed`

- `cargo test -p pwmd genesis_json_v4_roundtrip_encrypted_validator_key`
  - `test ... ok`
  - `test result: ok. 1 passed; 0 failed`

- `cargo test -p pwmd genesis_json_v4_rejects_wrong_passphrase`
  - `test ... ok`
  - `test result: ok. 1 passed; 0 failed`

- `cargo test -p pwmd genesis_json_v4_rejects_malformed_payload`
  - `test ... ok`
  - `test result: ok. 1 passed; 0 failed`

- `cargo test -p pwmd genesis_json_v4_rejects_path_mismatch`
  - `test ... ok`
  - `test result: ok. 1 passed; 0 failed`

- `cargo test -p pwmd genesis_json_rejects_unsupported_schema_version`
  - `test ... ok`
  - `test result: ok. 1 passed; 0 failed`

- `cargo test -p pwmd genesis_json_v4_rejects_extreme_kdf_iters`
  - `test ... ok`
  - `test result: ok. 1 passed; 0 failed`

- `cargo test -p pwm-cli genesis_build_generates_decoupled_v4_bundle`
  - `test ... ok`
  - `test result: ok. 1 passed; 0 failed`

- `cargo test -p pwm-cli genesis_build_cli_parses_required_flags`
  - `test ... ok`
  - `test result: ok. 1 passed; 0 failed`

## Notes

- During focused runs, Cargo reported short waits on package cache lock (`Blocking waiting for file lock on package cache`), but all test executions completed successfully.
- No additional runtime daemons/processes were left running by this validation workflow.
