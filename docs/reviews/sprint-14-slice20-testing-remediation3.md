# Sprint 14 Slice20 Remediation3 Testing

Repo: `P:/opt/docker/pwm-protocol`

## Verdict
`PASS`

No blockers found. Remediation3 rejects unknown/forged import provenance without crediting funds, and the legitimate Slice20 two-shard contract still passes.

## Required Checks
- Unknown/forged `export_id` import on an initialized target signer is rejected and does not credit funds: PASS.
  - Covered by `state::tests::import_rejects_missing_export_provenance_without_side_effects`.
  - Covered at API level by `tests::v1_tx_rejects_import_unknown_export_id` and `tests::v1_tx_rejects_unknown_import_after_signer_init`.
- CLI auto-init does not mask invalid import/provenance failure: PASS.
  - Covered by `tests::tx_import_auto_init_does_not_mask_unknown_export_id`.
- Legitimate happy path covered by `slice20_two_shard_e2e_flows_contract` still passes: PASS.
- Snapshot restart integrity still passes in that contract: PASS.
  - Contract asserts restart does not log `snapshot chain mismatch`.
- Guard labels and tx commit delta expectations still pass: PASS.
  - Contract asserts `shard=CY`, `shard=DO`, no legacy `shard=A`, and transfer/export/import `tx commit delta` log entries.

## Commands Run
```text
cargo fmt
PASS, 4.287s

cargo check
PASS, 3.876s

cargo test -p pwm-core import_ -- --nocapture
PASS, 6.122s
7 passed; 0 failed; 0 ignored; 68 filtered out

cargo test -p pwmd v1_tx_ -- --nocapture
PASS, 5.519s
24 passed; 0 failed; 0 ignored; 116 filtered out

cargo test -p pwm-cli tx_import_ -- --nocapture
PASS, 4.638s
6 passed; 0 failed; 0 ignored; 128 filtered out

cargo test -p pwmd slice20_two_shard_e2e_flows_contract -- --nocapture
PASS, 19.390s
1 passed; 0 failed; 0 ignored; 139 filtered out
```

## Cleanup
`pwmd` / `pwm-tui` cleanup check: PASS (`no pwmd/pwm-tui processes`).

## Open Risks
None found in the targeted remediation3 scope.
