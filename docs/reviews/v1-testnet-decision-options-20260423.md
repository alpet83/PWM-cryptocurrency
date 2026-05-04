# MVP SPEC v1 Testnet - Architecture Decision Options (No Auto-Decision)

**Date:** 2026-04-23  
**Repository:** `p:/opt/docker/PWM-cryptocurrency`  
**Primary intent:** provide clear architecture options and trade-offs for owner decision, without selecting automatically.

---

## Executive Summary (1-page style)

PWM v1 testnet should mature from v0 devnet to a multi-shard baseline while preserving the strict-upgrade spirit: keep the account-based core, keep local v0 flows stable, and add cross-shard transfer as an explicit extension (`EXPORT`/`IMPORT`) rather than a hidden semantic rewrite of `TRANSFER`.

Across the current specs and sync review, the hard center of gravity is consistent:

- v1 must support at least **two independent shards** and **coin transfer between shards**.
- The **account-based state model** from v0 remains the truth source.
- Cross-shard movement should be replay-safe and proof-based: `EXPORT -> finality proof -> IMPORT (+ optional admission)`.
- Policy for v1 baseline should be minimal but non-trivial for safety (recipient/domain constraints), with advanced controls reserved as hooks.
- Finality should be deterministic and configurable, but MVP can start with a minimal profile that can be hardened later.

The key architectural choices are not "whether to do v1" but **how strict and heavy the first cut should be** along five decision axes:

1. Ledger extension shape  
2. Export/import wire model  
3. Finality profile  
4. Policy baseline  
5. Backward-compat strategy

This document structures each axis into concrete options with pros/cons, risks, migration cost, testing complexity, and recommendation notes. It then provides 3 combined bundles (A/B/C) to help the project owner choose a coherent path quickly.

---

## Non-Negotiable Constraints

1. **Strict-upgrade principle**
   - No v1 baseline choice should force a rewrite of Layer 1 account core (`balance/staked/marks/initialized/index/flags`).
   - Existing v0 same-shard semantics (`INIT`, `TRANSFER`, `STAKE`, `UNSTAKE`, `BURN_MARK`) remain stable.

2. **Maturity target for v1 testnet**
   - At least 2 independent shards.
   - Cross-shard coin transfer operational.

3. **Explicit cross-shard path**
   - Cross-shard transfer must be explicit (`EXPORT`/`IMPORT`) and not implicit auto-routing in legacy `TRANSFER`.

4. **Deterministic safety path**
   - Replay protection (`ImportedSet` or equivalent used-export-id guard) is mandatory.
   - Finality proof must exist (minimal profile acceptable for MVP).

5. **Baseline policy safety**
   - Recipient/domain baseline restrictions in user flow are required (reject reserve/witness/unknown for regular transfer paths).

6. **No speculative protocol pivot**
   - UTXO-style tracks can be optional future optimization tracks, not v1 baseline core replacement.

---

## Decision Axes

### Axis 1 - Ledger Extension Shape

#### Option 1A: Pure Account-Core Extension (strict additive)
Keep account-based state as sole ledger core. Add roaming records and replay guards as adjunct state.

**Pros**
- Maximum alignment with strict-upgrade.
- Lowest risk of hidden semantic drift in v0 tx flows.
- Simplest migration story for wallet/RPC/CLI behavior.

**Cons**
- Less native flexibility for complex future output semantics.
- Some future optimizations may need extra adaptation layers.

**Risks**
- Extension data model can become fragmented if not carefully bounded.
- Potentially more custom logic around export commitments.

**Migration Cost**
- **Low** (mostly additive state and tx type extension).

**Testing Complexity**
- **Medium** (new cross-shard paths + replay/finality checks, but no core rewrite).

**Recommendation Note**
- Best baseline if v1 priority is time-to-stable-testnet with low protocol churn.

---

#### Option 1B: Hybrid Internal Abstraction (account externally, optional output-like internal records)
Expose account semantics externally, but introduce internal output-like records for roaming/export commitment handling.

**Pros**
- Preserves external strict-upgrade while preparing optimization hooks.
- Can improve internal consistency for proof generation/import validation.

**Cons**
- More complexity than pure additive account extension.
- Risk of abstraction leakage into external API semantics.

**Risks**
- Team may accidentally treat internal abstraction as de-facto UTXO migration.
- Harder to communicate to integrators if boundary is unclear.

**Migration Cost**
- **Medium**.

**Testing Complexity**
- **Medium-High** (must test equivalence of external behavior vs internal representation).

**Recommendation Note**
- Viable only if team is disciplined about keeping external behavior unchanged.

---

#### Option 1C: Core Pivot Toward UTXO-like Ledger
Use v1 to move ledger center from account model to UTXO-style structure.

**Pros**
- Potentially cleaner fit for some export/import constructs.
- Could simplify certain future output-level features.

**Cons**
- Conflicts with strict-upgrade baseline from current v1 sync direction.
- High breakage risk for v0 compatibility assumptions.

**Risks**
- Scope explosion and delay of v1 maturity milestone.
- Increased wallet/RPC/state migration burden.

**Migration Cost**
- **High**.

**Testing Complexity**
- **High** (retest whole transactional and state model surface).

**Recommendation Note**
- Treat only as separate future track, not MVP v1 baseline.

---

### Axis 2 - Export/Import Wire Model

#### Option 2A: Minimal Explicit Envelope
`ExportTx` includes sender/target/recipient/amount/fee/nonce; `ImportTx` carries finality certificate (+ optional admission certificate). Export ID from deterministic hash material. Replay guard via `ImportedSet`.

**Pros**
- Minimal moving parts and straightforward implementation.
- Directly aligned with current v1 roaming draft shape.
- Easy to instrument and debug in testnet.

**Cons**
- Less future-proof metadata in first version.
- May require envelope extension fields later.

**Risks**
- Inconsistent export-id derivation across nodes if canonicalization is underspecified.
- Edge-case drift if proof payload boundaries are ambiguous.

**Migration Cost**
- **Low**.

**Testing Complexity**
- **Medium** (certificate verification + deterministic export-id + replay tests).

**Recommendation Note**
- Strong default for MVP if stability and clarity are top priority.

---

#### Option 2B: Versioned Envelope from Day One
Same explicit export/import flow, but with envelope versioning and reserved extensibility fields for future admission/quarantine/compliance extensions.

**Pros**
- Better forward compatibility without wire break.
- Cleaner path for optional advanced policy layers.

**Cons**
- Slightly heavier implementation in MVP.
- Risk of over-designing unused fields.

**Risks**
- Extra optional fields can create inconsistent node handling if validation rules are not strict.
- Early complexity may slow delivery.

**Migration Cost**
- **Low-Medium**.

**Testing Complexity**
- **Medium-High** (version negotiation and unknown-field behavior tests).

**Recommendation Note**
- Good if project owner expects rapid post-v1 policy/finality iteration and wants fewer wire changes later.

---

#### Option 2C: "Transfer Auto-Routing" UX Wire (implicit cross-shard path)
Keep user-facing transfer shape and route cross-shard internally.

**Pros**
- Superficially simpler UX.

**Cons**
- Violates explicit roaming principle in current v1 direction.
- Obscures safety path and policy/finality boundaries.
- High ambiguity around failure semantics and replay handling.

**Risks**
- Hidden behavior changes in legacy `TRANSFER`.
- Hard-to-debug operational failures.

**Migration Cost**
- **Medium-High** (behavioral migration complexity).

**Testing Complexity**
- **High** (state machine ambiguity and branch coverage explosion).

**Recommendation Note**
- Not suitable for v1 baseline under strict-upgrade goals.

---

### Axis 3 - Finality Profile

#### Option 3A: Minimal Deterministic Profile (static set, configured threshold, raw/aggregated signatures accepted)
Per-shard static validator sets from genesis; finality certificate requires threshold signatures according to configurable profile.

**Pros**
- Meets v1 testnet needs with low operational complexity.
- Deterministic finality and explicit trust boundary.
- Compatible with future strengthening.

**Cons**
- Limited decentralization and governance flexibility.
- Weaker long-term assurances than advanced models.

**Risks**
- Threshold/profile misconfiguration between shards.
- Operational fragility if validator availability is poor.

**Migration Cost**
- **Low**.

**Testing Complexity**
- **Medium** (threshold and certificate verification matrix).

**Recommendation Note**
- Preferred baseline for fast, controlled v1 testnet maturity.

---

#### Option 3B: Hardened MVP Profile (stricter quorum policy + stricter certificate checks)
Still static sets, but enforce stricter quorum constraints and tighter proof validation rules from the start.

**Pros**
- Better safety posture for cross-shard trust.
- Lower chance of weak-finality operational incidents.

**Cons**
- Higher coordination and setup burden.
- Slower early iteration.

**Risks**
- Could block progress if shard validator operations are unstable.
- May increase false negatives in import acceptance during early network turbulence.

**Migration Cost**
- **Medium**.

**Testing Complexity**
- **Medium-High**.

**Recommendation Note**
- Good for teams prioritizing safety confidence over delivery speed.

---

#### Option 3C: Early Dynamic Validator Rotation & advanced consensus for MVP
Introduce dynamic validator lifecycle and more complex BFT behavior in baseline v1.

**Pros**
- Better long-term realism.

**Cons**
- Out of MVP v1 baseline scope per current documents.
- Major complexity increase in consensus and certification.

**Risks**
- Delivery delay and specification churn.
- Harder root-cause analysis in early testnet failures.

**Migration Cost**
- **High**.

**Testing Complexity**
- **Very High**.

**Recommendation Note**
- Defer to post-v1 track.

---

### Axis 4 - Policy Baseline

#### Option 4A: Minimal Mandatory Recipient/Domain Policy
Enforce baseline constraints: reject witness/reserve/unknown recipients in regular user flow; require explicit roaming for cross-domain movement; keep advanced policy optional.

**Pros**
- Meets minimum safety expectations.
- Keeps policy layer deterministic and understandable.
- Does not block v1 delivery.

**Cons**
- Limited institutional/compliance expressiveness in baseline.
- Some governance/compliance use cases postponed.

**Risks**
- If extension hooks are weakly defined, future policy upgrades may be awkward.
- Operators may overestimate security guarantees from minimal policy.

**Migration Cost**
- **Low**.

**Testing Complexity**
- **Medium** (policy-valid vs decode-valid matrix required).

**Recommendation Note**
- Best fit for v1 maturity milestone without overloading scope.

---

#### Option 4B: Minimal Baseline + Switchable Advanced Hooks (cosign/membership/admission toggles)
Keep baseline mandatory checks, but include dormant/feature-flagged advanced primitives with strict default-off behavior.

**Pros**
- Good upgrade runway.
- Supports controlled experimentation in testnet environments.
- Preserves baseline compatibility while preparing richer policy.

**Cons**
- More state/config surface in MVP.
- Requires clear governance around activation.

**Risks**
- Misconfigured flags can cause inconsistent node behavior.
- Increased combinatorial test matrix.

**Migration Cost**
- **Medium**.

**Testing Complexity**
- **Medium-High**.

**Recommendation Note**
- Strong if project owner wants early experiments without promising full advanced policy.

---

#### Option 4C: Full advanced policy mandatory for baseline
Require cosign/membership-heavy policy engine behavior from day one.

**Pros**
- Maximum policy richness immediately.

**Cons**
- Conflicts with current "advanced policy as extension" direction.
- High friction for baseline usability and implementation timeline.

**Risks**
- Over-constrained network behavior in early testnet.
- High risk of policy deadlocks/misconfig rejections.

**Migration Cost**
- **High**.

**Testing Complexity**
- **High**.

**Recommendation Note**
- Not recommended for MVP v1 baseline.

---

### Axis 5 - Backward-Compatibility Strategy

#### Option 5A: Strict Local Compatibility + Additive Cross-Shard Commands
Preserve existing local paths and semantics; add new explicit commands/endpoints/fields for roaming without changing old behavior.

**Pros**
- Lowest regression risk for existing tools/flows.
- Clean operator communication: "new capability is additive."
- Aligns with strict-upgrade spirit directly.

**Cons**
- Temporary duplication in APIs/CLI surface.
- More documentation burden for parallel old/new paths.

**Risks**
- Incomplete cross-surface consistency if CLI/RPC/wallet updates diverge.
- Legacy clients may ignore new errors around cross-shard misuse unless messaging is clear.

**Migration Cost**
- **Low-Medium**.

**Testing Complexity**
- **Medium** (compatibility regression + additive feature tests).

**Recommendation Note**
- Default compatibility posture for v1.

#### Option 5A-prime: Strict Local Compatibility + Protocol-Derived Routing (recommended)
Preserve existing local paths and semantics while keeping one API/CLI surface where routing is inferred by protocol:

- `domain_hi(sender) == domain_hi(receiver)` -> same-shard transfer path,
- `domain_hi(sender) != domain_hi(receiver)` -> explicit roaming path (`EXPORT/IMPORT`).

**Pros**
- No duplicated command surface for shard-local vs cross-shard mode.
- Keeps strict-upgrade behavior and removes manual route forcing.
- Matches protocol authority boundary (routing is node-side deterministic rule).

**Cons**
- Requires clear validation/errors so users understand why transfer becomes roaming.
- Documentation must clearly separate transfer intent from execution path.

**Risks**
- Ambiguous UX if error messages do not mention derived route logic.
- Client libraries must parse route-derived error codes consistently.

**Migration Cost**
- **Low-Medium**.

**Testing Complexity**
- **Medium** (same-shard and cross-shard branch coverage in one API surface).

**Recommendation Note**
- Preferred variant for Bundle A when owner wants minimal API duplication.

---

#### Option 5B: Compatibility with soft deprecation warnings
Keep additive strategy, but add explicit deprecation warnings for patterns expected to phase out.

**Pros**
- Improves forward migration planning.
- Gives ecosystem time to adapt.

**Cons**
- Requires careful wording and version policy discipline.
- Can confuse users if deprecation timeline is unclear.

**Risks**
- Warning fatigue in operators.
- Premature ecosystem migration pressure.

**Migration Cost**
- **Medium**.

**Testing Complexity**
- **Medium** (including warning behavior and non-breaking guarantees).

**Recommendation Note**
- Good if owner wants to shape post-v1 trajectory early.

---

#### Option 5C: Aggressive normalization of old flows into new behavior
Refactor old command semantics to map internally to new cross-shard rules quickly.

**Pros**
- Potentially cleaner long-term surface.

**Cons**
- Violates strict-upgrade expectation in spirit and likely in behavior.
- Elevated regression risk.

**Risks**
- Breakage in existing scripts/tooling.
- Hidden behavior changes difficult to audit.

**Migration Cost**
- **High**.

**Testing Complexity**
- **High**.

**Recommendation Note**
- Avoid for MVP v1.

---

## Combined Architecture Bundles

### Bundle A - Conservative Strict-Upgrade Baseline (delivery-first)
- Axis 1: **1A**
- Axis 2: **2A**
- Axis 3: **3A**
- Axis 4: **4A**
- Axis 5: **5A-prime** (or 5A if explicit split commands are still preferred)

**Profile**
- Fastest credible path to v1 maturity target.
- Lowest architectural churn.
- Best for proving two-shard operation and safe coin roaming early.

**Main Trade-off**
- Less immediate flexibility for advanced governance/policy/finality evolution.

---

### Bundle B - Balanced Baseline with Controlled Future Hooks (recommended middle path)
- Axis 1: **1A** or **1B** (prefer 1A unless team strongly needs internal abstraction)
- Axis 2: **2B**
- Axis 3: **3A** with selected hardened checks from 3B
- Axis 4: **4B**
- Axis 5: **5B**

**Profile**
- Keeps strict-upgrade baseline intact while preparing cleaner post-v1 extension runway.
- Moderately higher MVP complexity but lower risk of near-term protocol rework.

**Main Trade-off**
- Larger implementation/testing matrix than Bundle A.

---

### Bundle C - Aggressive Capability Push (not MVP-friendly)
- Axis 1: **1C**
- Axis 2: **2C** or over-extended 2B
- Axis 3: **3C**
- Axis 4: **4C**
- Axis 5: **5C**

**Profile**
- Attempts to accelerate long-term architecture in one milestone.

**Main Trade-off**
- High likelihood of delaying or destabilizing v1 testnet maturity goals.

---

## Decision Checklist (Project Owner Questions)

1. **Milestone priority:** Is the immediate objective "working two-shard transfer safely" or "preloading advanced architecture now"?
2. **Strict-upgrade tolerance:** How much behavioral change in existing v0 local flows is acceptable (if any)?
3. **Wire stability preference:** Is it worth paying MVP complexity now for versioned envelopes and reserved fields?
4. **Operational readiness:** Can shard operators sustain stricter finality settings from day one?
5. **Policy appetite:** Should v1 enforce only baseline safety, or include feature-flagged advanced policy hooks?
6. **Compatibility policy:** Should deprecation messaging begin in v1 or remain purely additive without migration pressure?
7. **Risk budget:** Which is less acceptable: delayed release (complex baseline) or technical debt (lean baseline)?
8. **Testing budget:** Is the team prepared for medium-high combinatorial testing if hooks/flags are included?
9. **Governance timing:** When should dynamic validator/governance tracks begin (immediately post-v1 vs later)?
10. **Success definition:** What concrete go/no-go criteria mark v1 as "done" for owner acceptance?

---

## Suggested First Implementation Slice After Choice

Assuming owner selects **Bundle A or B**, the first slice should prove end-to-end maturity in minimal scope:

1. **Two independent shard runtime setup**
   - Static validator set per shard.
   - Deterministic finality profile configuration and certificate emission.

2. **Explicit cross-shard transfer path**
   - Implement `ExportTx` state transition (debit + export commitment).
   - Implement finality proof packaging/verification.
   - Implement `ImportTx` transition (proof validation + credit).

3. **Replay safety core**
   - Persist `ImportedSet` (or equivalent).
   - Enforce hard rejection on duplicate import.

4. **Policy baseline gate**
   - Enforce recipient/domain baseline restrictions in user transfer flow.
   - Keep advanced policy optional/off unless explicitly chosen.

5. **Compatibility guardrails**
   - Verify unchanged semantics of local v0 tx types on same-shard path.
   - Keep wallet/CLI/RPC behavior additive for new cross-shard capabilities.

6. **Minimum acceptance tests for first slice**
   - Same-shard regression tests for v0 flows.
   - Cross-shard happy-path export/import.
   - Duplicate import rejection.
   - Invalid/weak certificate rejection.
   - Policy reject cases (witness/reserve/unknown recipient in regular flow).

---

## Practical Guidance on Choosing

- Choose **Bundle A** if the owner wants the shortest path to a stable, demonstrable v1 testnet.
- Choose **Bundle B** if the owner accepts moderate complexity now to reduce near-term rework and enable smoother post-v1 extension.
- Avoid **Bundle C** for MVP v1 baseline; treat it as a separate architectural track.

---

## Source Alignment Note

This options paper is constrained to the current repository context and sync direction, especially:
- strict-upgrade from v0 account core,
- explicit additive cross-shard flow (`EXPORT`/`IMPORT`),
- deterministic finality certificate path,
- mandatory replay protection,
- minimal policy baseline for user safety,
- deferred advanced economics/governance/policy tracks unless explicitly promoted by owner decision.
