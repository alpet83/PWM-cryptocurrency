# Sprint 15 S3.4 testing: one-window peer relay

Date: 2026-04-29  
Repository: `P:/opt/docker/PWM-cryptocurrency`  
Verdict: **PASS**

## Final Retest After Remediation2

No production code changes were made by this retest. The trust-boundary remediation2 acceptance pack passes, including forged inbound provenance rejection, trusted seed provenance, genesis guard, no-seed relay failure state, CLI one-window create/status coverage, and a live reciprocal-seed smoke.

### PASS: handoff trust boundary and trusted provenance

- `cargo test -p pwmd v1_export_provenance -- --nocapture` passed 4 tests:
  - `v1_export_provenance_rejects_self_attested_handoff`
  - `v1_export_provenance_rejects_handoff_after_inbound_node_hello`
  - `v1_export_provenance_accepts_configured_trusted_peer`
  - `v1_export_provenance_obeys_genesis_guard`
- Forged inbound/dev `NodeHello` plus forged handoff is rejected.
- Self-attested/untrusted handoff remains `403 Forbidden` and does not mutate target provenance.
- Configured/trusted seed peer provenance is accepted.

### PASS: genesis guard

- `v1_export_provenance_obeys_genesis_guard` still verifies `503 Service Unavailable` and no target provenance mutation while `genesis_guard.blocked = true`.

### PASS: relay failure state

- `cargo test -p pwmd v1_roaming_intent_no_seed_stays_exported_with_relay_error -- --nocapture` passed.
- `cargo test -p pwmd v1_roaming_intent_finalize_keeps_exported_without_seed -- --nocapture` passed.
- No-seed relay failure leaves the intent `Exported`, records `last_error` with `no --transport-peer-seed configured`, and does not persist a false successful `Relayed` state.

### PASS: CLI one-window create/status flow

- `cargo test -p pwm-cli tx_send_cross_domain_one_window_create_and_status_flow -- --nocapture` passed.
- The CLI still creates the cross-domain roaming intent and reports status without requiring the old manual fallback path in this unit-level flow.

### PASS: live two-node reciprocal seed smoke

Live two-node run used fresh state roots on `127.0.0.1:3130` and `127.0.0.1:3131`, both with `--transport-real` and reciprocal `--transport-peer-seed`.

- Source and target `tx-init` returned `204 No Content`.
- Source-only `tx-send` returned exit code `0`, created roaming intent `f95ce99f3566807993c29da8b7ee3a33999e75bdfb85a98758aa8a234535f25d`, and reported repeated `roaming intent status: relayed`.
- Source status JSON showed `status="relayed"`, `relay_mode="peer_relay_one_window"`, and the expected peer relay hint.
- Source log contained `peer relay handoff delivered seed=127.0.0.1:3131`.
- Cleanup completed with `CLEANUP_LEFT=none`.

## Commands Run

- `cargo test -p pwmd v1_export_provenance -- --nocapture` -> PASS (`4 passed`).
- `cargo test -p pwmd v1_roaming_intent_no_seed_stays_exported_with_relay_error -- --nocapture` -> PASS.
- `cargo test -p pwmd v1_roaming_intent_finalize_keeps_exported_without_seed -- --nocapture` -> PASS.
- `cargo test -p pwm-cli tx_send_cross_domain_one_window_create_and_status_flow -- --nocapture` -> PASS.
- Live two-node reciprocal seed smoke via host `pwmd.exe`/`pwm.exe` -> PASS for peer relay handoff delivery and source intent status `relayed`.

## Final Verdict

**PASS**: remediation2 satisfies the final S15-S3.4 retest scope. The acceptance pack rejects forged provenance paths, preserves genesis/no-seed failure guards, keeps CLI create/status green, and confirms live reciprocal seed peer relay reaches `relayed` provenance handoff delivery.
