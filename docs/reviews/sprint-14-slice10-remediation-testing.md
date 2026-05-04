# Sprint 14 Slice 10 Remediation Testing

Date: 2026-04-28
Repository: `P:/opt/docker/PWM-cryptocurrency`

## Scope

Validated remediation items:
1. docs now point to schema v3 flow and legacy helper path is explicitly obsolete;
2. `pwmd` rejects extreme `validator_keys[*].enc_seed.kdf.iters`;
3. renamed genesis derivation constant compiles and `genesis_build_*` tests pass.

Targeted tests requested:
- `pwmd`: `genesis_json_v3_rejects_extreme_kdf_iters`
- `pwm-cli`: `genesis_build_`

## Evidence

### 1) Docs: v3 flow is primary, legacy marked obsolete

Checked files and matches:
- `docs/GENESIS_BLOCK.md`
  - states `genesis-build` produces only v3 JSON (`schema_version=3`);
  - references old PowerShell helper as legacy fallback.
- `docs/pwmd.md`
  - states `load_genesis_bundle` accepts only `schema_version=3` and hard-fails legacy/v2.
- `docs/genesis_bundle_from_seed.ps1`
  - header includes `OBSOLETE SCRIPT: legacy helper kept for historical reference only`;
  - warning text says to use `pwm genesis-build` schema v3 for production.

Verdict: **PASS**.

### 2) `pwmd` rejects extreme `kdf.iters`

Source check:
- `crates/pwmd/src/snapshot.rs`: `GENESIS_KDF_ITERS_MAX: u32 = 10_000_000` and explicit fast-fail when `kdf.iters` exceeds cap.
- `crates/pwmd/src/lib.rs`: test `genesis_json_v3_rejects_extreme_kdf_iters`.

Test evidence:
- Command: `cargo test -p pwmd genesis_json_v3_rejects_extreme_kdf_iters -- --nocapture`
- Result: `1 passed; 0 failed` (secondary bin target: `0 tests`).
- Observed test: `tests::genesis_json_v3_rejects_extreme_kdf_iters ... ok`.

Verdict: **PASS**.

### 3) Renamed constant compiles and genesis build tests pass

Source check:
- `crates/pwm-cli/src/main.rs` uses renamed constant `GENESIS_DER_PATH_IDX`.
- `genesis_build` flow references this symbol in build logic and tests.

Test evidence:
- Command: `cargo test -p pwm-cli genesis_build_ -- --nocapture`
- Result: `2 passed; 0 failed`.
- Tests passed:
  - `genesis_build_cli_parses_required_flags`
  - `genesis_build_generates_rows_from_wallet_accounts`

Verdict: **PASS**.

## Final verdict

Slice10 remediation validation status: **PASS**.

All requested nits are validated:
- docs are aligned to v3 primary flow with legacy path marked obsolete;
- `pwmd` has enforced guardrail against extreme `kdf.iters` and covered test passes;
- renamed constant compiles under targeted `pwm-cli` genesis build test suite.
