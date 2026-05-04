# Sprint 14 Slice23 Coding: recipient init gate

## Decisions

- `Transfer` no longer creates or credits missing/uninitialized recipient stubs. Same-domain recipients must already exist and be initialized before sender balance, nonce, or fee state is mutated.
- `Import` keeps duplicate/unknown/forged `export_id` rejection first, then requires the target recipient to be initialized before sender nonce, recipient credit, or `imported_set` changes.
- CLI `tx-send` preflights same-domain recipients on the current source RPC. Cross-domain `tx-send` has no target RPC in the current one-window flow, so it prints that limitation and relies on target import rejection.
- CLI `tx-import` preflights the target recipient before sender auto-init so auto-init cannot mask recipient init failure.
- TUI blocks known missing/uninitialized recipients from the current shard view before submit and also preflights same-domain submit via `/v1/account/{to}`.
- `pwmd` package version bumped `0.1.14 -> 0.1.15` because public API validation behavior changed.

## Commands

- `cargo fmt` -> pass.
- `cargo test -p pwm-core recipient` -> pass (3 tests).
- `cargo test -p pwm-core import_rejects` -> pass (4 tests).
- `cargo test -p pwm-cli recipient_preflight` -> pass (2 tests).
- `cargo test -p pwm-tui preflight` -> pass (5 tests).
- `cargo test -p pwmd recipient` -> pass (11 tests).
- `cargo test -p pwmd v1_tx_two_node_smoke_cy_to_do_with_negative_suite` -> pass.
- `cargo check` -> pass.

## Notes

- Initial multi-filter `cargo test` commands failed because Cargo accepts only one test filter before `--`; reran with focused single filters.
- CQDS background index rebuild enqueued after code/doc edits.
