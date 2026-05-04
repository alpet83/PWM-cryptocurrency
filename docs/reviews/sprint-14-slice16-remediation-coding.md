# Sprint 14 Slice16 remediation (coding)

Date: 2026-04-28

## Scope

- Fixed flow taxonomy inconsistency for finalize transition to relayed.
- Synced docs for route inventory and current finalize/flow contract.
- Added missing finalize edge tests requested by review.

## Implemented changes

- `crates/pwmd/src/api.rs`
  - finalize now emits `kind=roaming_status:relayed` (instead of `roaming_status:export`) when transition is applied.
- `crates/pwmd/src/lib.rs`
  - strengthened `flow/recent` test to assert concrete finalize/relayed taxonomy.
  - extended finalize snapshot-failure test with retry-after-500 idempotency check.
  - added terminal-status finalize idempotency test (`imported`, `expired`, `failed`).
- `docs/pwmd.md`
  - route inventory now includes `POST /v1/roaming-intents/:id/finalize`.
- `docs/rfc/9-crossdomain-roaming.md`
  - updated contract text for finalize endpoint and `flow/recent` event families.

## Notes

- Changes are intentionally narrow and behavior-preserving outside finalize flow taxonomy/events.
