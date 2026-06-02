# ADR 0007: Domain lease parameter governance

## Status

**Accepted as V5 spec-only contract.** This ADR defines governance boundaries for future `domain_lo > 0` leases. Runtime auction and lease enforcement are out of scope for V5.

## Context

Corporate and sector domains need a way to allocate scarce `domain_lo > 0` namespaces without treating them as permanent property. The roadmap expects high demand in some clusters, especially IT and adjacent infrastructure sectors. Governance must keep parameters adjustable while protecting active participants from retroactive rule changes.

## Decision

`domain_lo = 0` remains the root/generic corporate registration slot inside a domain cluster. Leased namespaces use `domain_lo > 0`.

Domain lease governance controls parameters, not individual private balances. Parameter changes are bounded by protocol rules and validator voting.

## Lease Parameters

The governed parameter set is:

```text
DomainLeaseParams {
  min_rent_pwm: u128
  grace_period_blocks: u64
  auction_duration_blocks: u64
  renewal_window_blocks: u64
  max_annual_adjustment_ppm: u64
}
```

Recommended V5 constraints:

- `min_rent_pwm` is the minimum lease price or renewal floor.
- `grace_period_blocks` defines how long an expired lease can be renewed before release.
- `auction_duration_blocks` defines the minimum duration for contested allocation.
- `renewal_window_blocks` defines how early a current tenant can renew.
- `max_annual_adjustment_ppm` caps governance changes to avoid sudden rent shocks.

Exact numeric production values belong in genesis/config documents, not this ADR.

## Governance Process

Validator governance may update future lease parameters if all conditions hold:

- the update is within `max_annual_adjustment_ppm` or another accepted protocol bound;
- the update has a deterministic activation height;
- the update does not alter terms of already active leases;
- the update is auditable in chain state, genesis, or a governance transcript accepted by the protocol;
- parameter changes are cluster-scoped where possible instead of global by default.

Existing active leases keep their accepted terms until renewal or expiry. New terms apply to future auctions and renewals after the activation height.

## No Burn Principle

Lease payments are not mark burns and must not be represented as `BurnMarkTx`.

Rationale:

- marks are an anti-spam/attention resource, not rent;
- domain leases are infrastructure privileges and should be accounted separately;
- burning marks for leases would mix utility throttling with namespace governance.

Future runtime may route lease payments to treasury, validator rewards, or another accepted account, but that distribution requires a separate implementation RFC.

## Out-of-Scope

- Runtime auction implementation.
- Lease payment settlement.
- Slashing or dispute process.
- Exact sector/IT subcluster allocation.
- Transferability or secondary markets for leases.

## References

- [CONCEPT_ROADMAP: MVP V5 domain distribution and leases](../CONCEPT_ROADMAP.md)
- [RFC 0007: Transaction & State Model](../rfc/7-tx-and-state-model.md)
- [MVP v5 plan](../plans/mvp_v5.md)
