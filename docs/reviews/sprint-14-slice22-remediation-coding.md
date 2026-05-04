# Sprint 14 Slice 22 Remediation Coding

## Scope

Addressed approve-with-nits review feedback for Slice22 docs only.

## Changes

- Strengthened `docs/pwm-cli.md` `tx-import` notes with the raw unit scale: `1 PWM = 1_000_000 raw`.
- Documented the target-recipient stub/auto-init contract for `tx-import`, including that sender-side auto-init must not mask invalid import provenance.
- Normalized the Slice22 coding report test count to the later audited `134 + 73` result from the testing report.

## Validation

- Docs-only change; cargo checks/tests were not run.
- No help text sync was needed because the nit was limited to operator docs and audit trail wording.

## CQDS Index

Skipped: docs-only nit cleanup with no source or file-structure change.
