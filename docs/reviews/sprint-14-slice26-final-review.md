# Sprint 14 Slice26 Final Review

## Verdict
`approve`

## Summary
The selected-owner signing invariant is now enforced for both active and non-active wallet accounts. `signing_material_for_sender` verifies that wallet key material, domain, derivation index, and selected account id match before returning signing material.

The original failure shape (`CY` selected/active while decrypted payload key belongs to `DO/DB`) is now blocked before submit, and F6 uses the selected Owner row as the runtime sender.

## Coverage
- `cargo test -p pwm-tui` passed (`80 passed`).
- `cargo check -p pwm-tui` passed.
- Regression `active_cy_rejects_db_payload_signing_key` passed.
