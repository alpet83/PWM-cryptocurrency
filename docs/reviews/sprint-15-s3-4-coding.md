# Sprint 15 S3.4 coding report

Date: 2026-04-29

## Scope

- Implemented one-window peer relay foundation in `pwmd`: source RPC finalizes newly created cross-shard roaming intents and attempts to deliver handoff provenance to a matching configured seed peer.
- Added source-side relay for foreign `IMPORT` submissions: CLI/TUI can submit the signed import tx to the source node, which relays it to the target peer selected by target `domain_hi`.
- Kept manual fallback commands (`finalize`, `tx-handoff-register`, `tx-import`) intact.
- Added safe genesis-fetch foundation as status-only stub; no silent genesis replacement is performed.

## Files touched

- `crates/pwmd/src/relay.rs`
- `crates/pwmd/src/api.rs`
- `crates/pwmd/src/transport.rs`
- `crates/pwmd/src/state.rs`
- `crates/pwmd/src/bootstrap.rs`
- `crates/pwmd/src/lifecycle.rs`
- `crates/pwmd/src/config.rs`
- `crates/pwmd/src/main.rs`
- `crates/pwmd/src/lib.rs`
- `crates/pwmd/Cargo.toml`
- `crates/pwm-cli/src/main.rs`
- `issues-report.md`

## Behavior

- `/v1/status` now exposes minimal relay/peer diagnostics: `cluster_domain_hi`, seed/live peer counts, peer relay health, next reconnect timestamp, stuck exported/relayed counters, and genesis-fetch stub fields.
- Real transport default retry max is now 60 seconds, and unhealthy seeded transport emits WARN logs with next reconnect timing.
- `pwm-cli tx-send` runs export readiness before creating roaming intent, matching the fail-closed backend contract.
- Peer relay selects target by `cluster_domain_hi` reported by seed peer `/v1/status`.

## Limits

- Target import still requires a valid signed `IMPORT` transaction. The source node can relay it, but it does not forge or bypass wallet signatures.
- Relay delivery uses configured seed peer HTTP `/v1/*` endpoints as the bounded S15-S3.4 transport foundation.
- The genesis fetch path is intentionally status-only; unsafe local genesis replacement is not implemented.

## Remediation: review blockers

- `/v1/export-provenance` no longer trusts a self-attested handoff key. The handoff signature still verifies the payload, but registration now also requires the source node identity/key to match trusted peer state learned through accepted `NodeHello`; peer relay posts `NodeHello` before provenance delivery.
- `/v1/export-provenance` now uses the same genesis guard gate as user tx paths, so a blocked genesis guard fails closed before mutating target provenance state.
- Source roaming intent state is not persisted as `relayed` until target provenance delivery succeeds. Relay failures keep the intent `exported`, record `last_error`, and emit `relay_error:export_provenance` for retry/manual fallback visibility.
- Manual fallback remains available through `finalize` + `tx-handoff-register` + `tx-import`; target registration now requires trusted peer context rather than accepting arbitrary self-attested handoff material.

## Commands

- `cargo fmt`
- `cargo check -p pwmd -p pwm-cli`
- `cargo test -p pwmd foreign_import_detects_target_domain`
- `cargo test -p pwmd v1_export_provenance_rejects_self_attested_handoff`
- `cargo test -p pwmd v1_export_provenance_obeys_genesis_guard`
- `cargo test -p pwmd v1_roaming_intent_no_seed_stays_exported_with_relay_error`
- `cargo test -p pwmd v1_roaming_intent_finalize_keeps_exported_without_seed`
- `cargo test -p pwmd v1_status_reports_alias_state_namespace_for_shard`
- `cargo test -p pwmd real_transport_tick_respects_retry_backoff_on_connect_timeout`
- `cargo test -p pwm-cli tx_send_cross_domain`

## Remediation 2: provenance trust boundary

- Remaining trust-boundary blocker fixed: accepted inbound/dev `NodeHello` no longer populates provenance trust roots.
- `/v1/export-provenance` now accepts handoff provenance only when the source node identity matches relay-trusted peer state learned from a configured outbound `--transport-peer-seed` context. Test-only trust setup is kept behind `cfg(test)`.
- Manual fallback remains available, but direct target-side `tx-handoff-register` requires that the target already trusts the source peer through configured peer connectivity; arbitrary forged hello + handoff registration is rejected.
- Bumped `pwmd` build/version marker: `0.1.22 -> 0.1.23` because `/v1/export-provenance` provenance trust validation changed.
- Added focused tests for forged inbound `NodeHello` + handoff rejection, configured/trusted peer acceptance, and existing genesis guard / relay-state behavior.
