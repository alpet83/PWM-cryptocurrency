# Sprint 14 Slice23 Testing: recipient init gate

## Verdict

PASS. No blockers found.

The recipient-init gate held for same-shard transfers, target-side imports, forged/unknown export IDs, CLI/TUI preflight, cross-shard happy path, and snapshot persistence coverage.

## Required Checks

- Same-shard transfer to a missing recipient rejects before sender debit/nonce mutation: covered by `apply_tx_transfer_rejects_missing_recipient_without_side_effects`.
- Same-shard transfer to `initialized=false` recipient rejects before sender debit/nonce mutation: covered by `apply_tx_transfer_rejects_uninitialized_recipient_without_side_effects`.
- Target-side `tx-import` to missing or `initialized=false` recipient rejects before credit/imported_set mutation: covered by `apply_tx_import_rejects_missing_destination_without_side_effects` and `apply_tx_import_rejects_uninitialized_destination_without_side_effects`.
- Unknown/forged `export_id` rejection still works: covered by `import_rejects_missing_export_provenance_without_side_effects`.
- Happy cross-shard flow succeeds when target recipient is initialized first: covered by `v1_tx_two_node_smoke_cy_to_do_with_negative_suite`.
- CLI/TUI preflight gives actionable recipient-init failures and does not hide them behind sender auto-init: covered by `recipient_preflight_blocks_missing_account`, `recipient_preflight_blocks_uninitialized_account`, and TUI `preflight` tests.
- Snapshot/restart integrity not regressed: covered by `pwmd snapshot`, including `snapshot_roundtrip_loads_after_transfer_to_initialized_recipient` and import replay/provenance snapshot tests. `pwmd restart` filter currently matches 0 tests.

## Commands Run

- `cargo test -p pwm-core recipient` -> PASS, 3 tests, 351 ms.
- `cargo test -p pwm-core import_rejects` -> PASS, 4 tests, 328 ms.
- `cargo test -p pwm-cli recipient_preflight` -> PASS, 2 tests, 2872 ms.
- `cargo test -p pwm-tui preflight` -> PASS, 5 tests, 1953 ms.
- `cargo test -p pwmd recipient` -> PASS, 11 tests, 685 ms.
- `cargo test -p pwmd v1_tx_two_node_smoke_cy_to_do_with_negative_suite` -> PASS, 1 test, 641 ms.
- `cargo test -p pwmd snapshot` -> PASS, 26 tests, 600 ms.
- `cargo test -p pwmd restart` -> PASS, 0 tests matched, 339 ms.
- `cargo check` -> PASS, 425 ms.

Harness: CQDS `cq_process_ctl` host mode, cwd `P:\opt\docker\pwm-protocol`. No hang watchdog triggered.

## Notes

- No checklist rows were changed.
- No `#[ignore]` tests were added.
- No product bugs were found in this slice.
