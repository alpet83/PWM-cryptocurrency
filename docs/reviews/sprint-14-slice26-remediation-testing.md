# Sprint 14 Slice26 Remediation Testing

## Verdict
`PASS`

## Scope
- Retested the Slice26 selected-owner signing remediation against `docs/reviews/sprint-14-slice26-review.md`.
- Confirmed the active-wallet shortcut now rejects mismatched selected-account signing material before F6 submit.

## Required Checks
- `cargo test -p pwm-tui` passed: 80 passed, 0 failed, 0 ignored.
- `cargo test -p pwm-tui active_cy_rejects_db_payload_signing_key` passed: 1 passed, 79 filtered out.
- `cargo test -p pwm-tui selected_owner` passed: `f6_wallet_mode_uses_selected_owner_row_as_sender` and `signing_rejects_selected_owner_without_material`.
- `cargo test -p pwm-tui cy_selected_while_db_active_signs_cy_not_db` passed as the prior CY/DB selected-owner regression guard.
- `cargo check -p pwm-tui` passed.

## Regression Proof
`active_cy_rejects_db_payload_signing_key` constructs an identity whose selected/active account is CY while the unlocked payload `signing_key` belongs to DB. It first asserts `signing_material_for_sender(&cy, &identity)` returns an error containing `selected owner cannot be signed` and `signing key for m/0/`, then calls `f6_send_form_for_identity(...)` and requires `Err`, with the error containing `F6 send blocked` and `selected owner cannot be signed`. This proves the mismatch blocks before submit.

## Execution Notes
- Commands were run through `cq_process_ctl` host mode in `P:\opt\docker\PWM-cryptocurrency`.
- No hang watchdog triggered.
- Cleanup: no long-lived test process was started by this retest. A post-run process check showed existing `pwmd` and `pwm-tui` processes already present, so they were not killed by this testing pass.

## Checklist
- No `docs/MVP-checklist.md` rows were changed.

## Open Risks
- None found in the requested automated checks.
