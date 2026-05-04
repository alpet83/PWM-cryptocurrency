# Sprint 14 — Slice 20 remediation3 (coding)

Repo: `P:/opt/docker/PWM-cryptocurrency`

## Verdict
Implemented.

## Blocking issue fixed
Target-side `IMPORT` no longer creates missing `ExportProvenance` from the import payload.

Before remediation3, an unknown `export_id` could pass the target-side prefilter when signer/account checks passed, then `State::apply_tx` registered provenance from the `Import` body and credited funds. That was a self-attested import/mint path.

## Contract
- `IMPORT` is accepted only when target state already has matching `exported_registry[export_id]`.
- Unknown `export_id` fails deterministically:
  - core: `TxError::InvalidImport`;
  - HTTP: `400 invalid import: export_id is not known`.
- Mismatched known provenance still fails with `400 invalid import: export provenance mismatch`.
- CLI `tx-import` may still auto-init the target signer account, but that does not make unknown provenance valid and does not credit funds.

## Legitimate happy path
The existing real transport e2e remains passing: `CY export -> finalize -> DO import` succeeds because the target receives legitimate export provenance before import. No target-side import self-registration is used.

If a deployment path does not deliver provenance/proof to the target, the current safe behavior is a clear import failure with funds unchanged.

## Files touched
- `crates/pwm-core/src/state.rs`: removed import-payload provenance creation; updated core regression test.
- `crates/pwmd/src/tx_policy.rs`: unknown import provenance is now a hard `400`.
- `crates/pwmd/src/lib.rs`: added/updated API regressions for unknown `export_id` on initialized signer.
- `crates/pwm-cli/src/main.rs`: added auto-init + invalid import regression.
- `crates/pwmd/src/slice20_e2e_tests.rs`: documented that e2e import depends on legitimate delivered provenance.
- `crates/pwmd/Cargo.toml`, `Cargo.lock`: bumped `pwmd` `0.1.12 -> 0.1.13` for public validation behavior.
- `issues-report.md`: recorded the self-attested import trap and follow-up proof/relay recommendation.

## Commands run
```text
cargo fmt
cargo check
cargo test -p pwm-core import_ -- --nocapture
cargo test -p pwmd v1_tx_ -- --nocapture
cargo test -p pwm-cli tx_import_ -- --nocapture
cargo test -p pwmd slice20_two_shard_e2e_flows_contract -- --nocapture
```

Result: PASS.

Note: one earlier test invocation used invalid Cargo multi-filter syntax and failed before running tests; it was rerun with valid filters.

## Optimization Note
No new broad abstraction was added. The fix reduces coupling by keeping provenance creation in export/relay paths only and making import validation a single invariant at both prefilter and state-apply layers.
