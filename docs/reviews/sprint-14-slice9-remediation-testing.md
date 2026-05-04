# Sprint 14 Slice 9 Remediation Testing

Date: 2026-04-28
Repository: `P:/opt/docker/PWM-cryptocurrency`

## Scope

Validated remediation items:
1. strict schema branching in `load_genesis_bundle` (no broad v2 -> legacy fallback);
2. unsupported schema and invalid `schema_version` handling;
3. docs consistency for derivation contract.

Targeted tests requested:
- `pwmd` filter: `genesis_json_`
- `pwm-cli` filter: `genesis_build_`

## Evidence

### 1) Strict schema branching in `load_genesis_bundle`

Source check (`crates/pwmd/src/snapshot.rs`):
- If `schema_version` field exists:
  - non-u64 -> error: `schema_version must be an unsigned integer when present`;
  - `2` -> parse via strict v2 parser;
  - any other value -> error: `unsupported schema_version ...`.
- Legacy parser is used only when `schema_version` is omitted.
- No v2 parse failure fallback to legacy branch.

Test evidence (`cargo test -p pwmd genesis_json_`):
- `genesis_json_v2_does_not_fallback_to_legacy` passed.

Verdict: **PASS**.

### 2) Unsupported schema and invalid `schema_version` handling

Source check (`crates/pwmd/src/snapshot.rs`):
- explicit reject for unsupported versions (`!= 2` when present);
- explicit reject for invalid type (non-unsigned integer) for `schema_version`.

Test evidence (`cargo test -p pwmd genesis_json_`):
- `genesis_json_rejects_unsupported_schema_version` passed.

Note:
- Requested targeted filter confirms unsupported-version behavior.
- Non-integer `schema_version` is covered by explicit code path; no dedicated `genesis_json_` test with this exact malformed payload was executed in this run.

Verdict: **PASS** (with minor residual test gap for malformed-type case in targeted suite).

### 3) Docs consistency for derivation contract

Checked docs:
- `docs/pwmd.md` bootstrap section:
  - v2: `SLIP-0010 m/0'/<der_idx>`;
  - legacy: `SLIP-0010 m/0'/0'`.
- `docs/GENESIS_BLOCK.md` validator key roles/checklist:
  - same v2 vs legacy derivation contract wording.

Verdict: **PASS** (documents aligned).

## Targeted test run results

### Command 1
`cargo test -p pwmd genesis_json_`

Result:
- `6 passed; 0 failed; 0 ignored` (plus secondary bin target with `0 tests`).
- Key tests in scope passed:
  - `genesis_json_v2_does_not_fallback_to_legacy`
  - `genesis_json_rejects_unsupported_schema_version`
  - `genesis_json_v2_rejects_invalid_hex`
  - `genesis_json_v2_rejects_seed_row_len_mismatch`
  - `genesis_json_v2_roundtrip_hex_fields`
  - `genesis_json_roundtrip_dev_seed`

### Command 2
`cargo test -p pwm-cli genesis_build_`

Result:
- `2 passed; 0 failed; 0 ignored`.
- Tests:
  - `genesis_build_cli_parses_required_flags`
  - `genesis_build_generates_rows_from_wallet_accounts`

## Final verdict

Slice9 remediation validation status: **PASS**.

All three requested areas are validated as remediated.  
No regressions observed in requested targeted test suites.
