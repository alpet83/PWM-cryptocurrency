# Sprint 14 — Slice 20 remediation4 (coding)

Repo: `P:/opt/docker/PWM-cryptocurrency`

## Verdict
Implemented.

## What changed
- Source `finalize` now returns a portable signed `handoff` payload for exported roaming intent provenance.
- Target node exposes `POST /v1/export-provenance` to validate and register that handoff before `tx-import`.
- `pwm-cli` adds `tx-handoff-register --handoff-json <path>` for the operator handoff step.
- Slice20 e2e now uses `finalize -> tx-handoff-register -> tx-import`, not hidden target registry insertion.
- Unknown/forged `export_id` import rejection remains intact.

## Notes
- Import provenance matching now treats `target_domain` at domain-hi boundary, so a target-domain signer can import to another account in the same target shard/domain-hi. This matches the existing CLI/e2e model where the import signer and credited recipient can be different accounts.
- `pwmd` version marker bumped `0.1.13 -> 0.1.14` because the public API contract changed.

## Commands run
```text
cargo fmt
cargo check
cargo test -p pwm-core import_ -- --nocapture
cargo test -p pwmd v1_tx_ -- --nocapture
cargo test -p pwm-cli tx_import_ -- --nocapture
cargo test -p pwm-cli tx_handoff_ -- --nocapture
cargo build -p pwmd --bin pwmd -p pwm-cli --bin pwm
cargo test -p pwmd slice20_two_shard_e2e_flows_contract -- --nocapture
```

Result: PASS.

Note: one earlier e2e invocation failed because the test launches prebuilt `target/debug/pwmd` / `pwm`; after explicit binary rebuild, the Slice20 contract passed.

## Optimization Note
The change keeps handoff logic in existing `pwmd` API/CLI boundaries and avoids adding a broad relay abstraction. Remaining decomposition candidate: move roaming handoff request/response helpers from `api.rs` into a focused module if more proof formats are added.
