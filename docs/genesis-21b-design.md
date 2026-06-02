# Genesis 21B Design (V5-7)

## 1. Purpose and Scope

This document defines a design-level token allocation shape for a 21B PWM genesis program.
It is a planning artifact for Sprint V5-7 and does not change wire/runtime rules by itself.

Scope of this document:

- allocation structure for the 21B program;
- IPv4-weighted distribution model for claim tranches;
- phasing cadence for long-window distribution;
- production placeholder boundaries and follow-up RFC/ADR dependencies.

Out of scope:

- production registry operations;
- final legal/compliance processes;
- direct runtime implementation details beyond already accepted protocol primitives.

## 2. Allocation Table (21B PWM)

Target planning split:

| Bucket | Amount (PWM) | Share | Intent |
|---|---:|---:|---|
| IPv4 Claim Pool | 20,000,000,000 | 95.238% | Long-window network distribution tied to IPv4 claim windows |
| Verifier / Bootstrap Premine | 500,000,000 | 2.381% | Initial operational bootstrap, verification, launch safety buffer |
| Team / Operations Reserve | 400,000,000 | 1.905% | Core operations, maintenance runway, ecosystem support |
| Devnet / Test Faucet Reserve | 100,000,000 | 0.476% | Devnet onboarding, demos, integration testing |
| **Total** | **21,000,000,000** | **100%** |  |

Notes:

- the IPv4 claim pool is intentionally dominant to keep distribution externalized and multi-phase;
- premine and reserves are explicitly bounded and kept small relative to the claim pool;
- final operational policies (custody, disclosure, unlock governance) are to be defined in follow-up governance artifacts.

## 3. IPv4-Weighted Formula

The distribution model follows a tiered, compressed weighting strategy so that large prefixes do not linearly dominate allocation.

Let `w(prefix)` be the claim weight unit:

- `/8`: full tier weight, `w(/8) = 256`
- `/16`: sqrt tier, `w(/16) = sqrt(256) = 16`
- `/24`: unit tier, `w(/24) = 1`

Equivalent generalized form for IPv4 prefix size `p`:

$$
w(p) = \max\left(1, \sqrt{\frac{2^{24}}{2^{p}}}\right)
$$

with practical tier clamps to `{256, 16, 1}` for `/8`, `/16`, `/24` classes used by the program.

Per-phase claim output is proportional to normalized weights:

$$
claim_i = phase\_budget \times \frac{w_i}{\sum_j w_j}
$$

where `phase_budget` is the tranche budget for the active phase.

## 4. Phasing Schedule (5 x ~4B PWM)

The IPv4 Claim Pool (20B PWM) is released in five target tranches:

| Phase | Target Budget (PWM) | Nominal Interval |
|---|---:|---|
| Phase 1 | ~4,000,000,000 | Launch window |
| Phase 2 | ~4,000,000,000 | +1 to +2 years |
| Phase 3 | ~4,000,000,000 | +1 to +2 years |
| Phase 4 | ~4,000,000,000 | +1 to +2 years |
| Phase 5 | ~4,000,000,000 | +1 to +2 years |

Design intent:

- avoid one-shot release pressure;
- preserve adaptation room across network maturity cycles;
- keep governance adjustment points predictable and sparse.

Exact dates/heights are governance-managed and must be published before each phase opens.

## 5. Production Genesis Placeholder

This section is a placeholder until the production governance package is approved.

To be finalized in production package:

- definitive addresses/accounts for non-claim buckets;
- shard fan-out topology for distribution accounts;
- registry signing and claim proof operational policy;
- disclosure and audit format for tranche accounting.

Until finalized, this document should be treated as architectural design guidance, not as a deploy-ready genesis manifest.

## 6. Protocol and Plan Cross-References

- ADR 0002 foundation boundary for IPv4 claiming and shard fan-out direction: `docs/adr/0002-ipv4-claiming-design.md`
- MVP V5 Sprint V5-7 planning anchor: `docs/plans/mvp_v5.md`
- On-chain claim primitive reference (`ClaimIPv4Batch`) introduced by V5 model slices in `pwm-core`.
