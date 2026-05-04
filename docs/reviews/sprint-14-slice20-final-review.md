# Sprint 14 Slice20 Final Review

## Verdict
`block`

## Blocking Finding
Target-side import must not self-register missing export provenance from an arbitrary `Import` payload.

The current remediation2 path can accept an unknown `export_id` on the target side when signer/account checks pass, then create provenance from the import payload and credit funds. That turns a relay race into a possible self-attested import/mint path.

## Required Fix
- Import must require proven source-side export/finalized provenance, or a formally constrained proof passed through the protocol.
- Add a negative test: forged/unknown `export_id` on an initialized target signer must not credit balance.
- Keep CLI auto-init documented, but do not let auto-init mask an invalid import.

## Notes
- Same-hi routing, atomic commit, rollback, `CY/DO` guard labels, and `tx commit delta` coverage are directionally correct.
- Raw-balance UX belongs to Slice22 and does not block Slice20 by itself.
