# Sprint 14 — Slice32 Testing

Date: 2026-04-29
Repository: `P:/opt/docker/pwm-protocol`

## Scope validated

1. TUI receivers no longer hide own addresses from `address_book`.
2. Protocol rejects self-transfer at tx shape validation.
3. State path for self-transfer is no-side-effect reject.
4. Existing TUI suite remains green.

## Test runs

1. `cargo test -p pwm-core validate_tx_shape_rejects_self_transfer -- --nocapture`  
   - Result: **PASS**  
   - Evidence: `tx::tests::validate_tx_shape_rejects_self_transfer ... ok`  
   - Duration: ~2.49s

2. `cargo test -p pwm-core apply_tx_transfer_self_is_rejected_without_side_effects -- --nocapture`  
   - Result: **PASS**  
   - Evidence: `state::tests::apply_tx_transfer_self_is_rejected_without_side_effects ... ok`  
   - Duration: ~0.95s

3. `cargo test -p pwm-tui owner_and_receivers_keeps_owned_addresses_from_address_book -- --nocapture`  
   - Result: **PASS**  
   - Evidence: `tests::owner_and_receivers_keeps_owned_addresses_from_address_book ... ok`  
   - Duration: ~3.59s

4. `cargo test -p pwm-tui`  
   - Result: **PASS**  
   - Evidence: `test result: ok. 84 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`  
   - Duration: ~5.24s

## Verdict

**PASS** — all requested Slice32 checks succeeded, including full `pwm-tui` regression suite.
