# RFC 0012: Marks maturity and lazy state model

**Status:** Active (V5-1 normative revision)
**Version:** 2.0
**Replaces:** RFC 0012 v1 claim maturity model (`ClaimTx`, `anchor_ref`, free-claim day)

## Abstract

RFC 0012 v2 replaces the explicit claim/maturity model with deterministic lazy accumulation of marks. Marks are generated only from staked whole PWM, measured by chain height, clamped at `MARKS_CAP`, and materialized when an account is touched by an existing state transition. There is no demurrage, no mark TTL, no daily free claim, and no standalone `ClaimTx` in the V5 active scope.

## Motivation

- Whitepaper tokenomics now require lazy accumulation to a fixed cap instead of per-claim materialization.
- Consensus should avoid wall-clock and anchor continuity rules when block height is sufficient.
- The state cursor should be minimal and replayable: one `marks_last_block` per account.
- Claim-specific validation paths created extra surface area without adding utility once marks can be computed lazily.

## State Model

Each account stores:

```text
Account {
  stored_marks: u32
  staked_pwm_raw: u128
  marks_last_block: u64
}
```

`marks_last_block` is the only marks maturity cursor. It records the chain height of the last account touch that applied lazy mark accumulation.

Public JSON-facing representations of V5 economic state MUST encode `staked_pwm_raw` and other `u128` quantities as decimal strings. This rule applies to public API payloads, human-readable snapshot JSON, and operator-facing JSON/config surfaces. It does not change binary state hashing or canonical signing preimages.

Fields from RFC 0012 v1 are removed from the V5 active state contract:

- `last_claim_unix_time`
- `last_claim_anchor_ref`
- `free_claim_utc_day`
- `marks_expiry_block`

Snapshot migration from older claim-state schemas MUST NOT try to infer exact block height from Unix time. If no canonical height mapping exists, migration initializes `marks_last_block = current_snapshot_height` or another explicit migration height chosen by the snapshot loader. The migration MUST NOT mint retroactive marks from ambiguous wall-clock data.

## Parameters

V5 uses the following genesis/config parameters:

```text
blocks_per_hour: u64              // default 3600
marks_per_coin_per_hour: u64      // default 1
```

**Marks balance cap (single normative symbol):**

```text
MARKS_CAP: u32                     // fixed ceiling for stored_marks and effective_marks
                                   // V5: MARKS_CAP == u32::MAX (2^32 − 1)
                                   // not a genesis/runtime knob in V5; define once per chain spec
```

Implementations MUST expose `MARKS_CAP` as one named constant (e.g. `pub const MARKS_CAP: u32 = u32::MAX` in reference code) and use it in lazy-marks formulas instead of scattering `u32::MAX`. Changing `MARKS_CAP` below `u32::MAX` would require a new chain spec / fork; `stored_marks` remains type `u32`.

`blocks_per_hour` is a deterministic chain-height conversion parameter. It is not a wall-clock oracle.

## Lazy Accumulation Formula

For an account touched at `current_height`:

```text
delta_blocks = current_height.saturating_sub(account.marks_last_block)
delta_hours = floor(delta_blocks / blocks_per_hour)
whole_pwm_staked = floor(account.staked_pwm_raw / 1_000_000)
rate = marks_per_coin_per_hour

if whole_pwm_staked == 0 or rate == 0:
    generated = 0
else:
    remaining = MARKS_CAP - stored_marks
    per_hour = whole_pwm_staked * rate
    satur_hours = ceil(remaining / per_hour)   // integer ceiling; see normative note below
    effective_hours = min(delta_hours, satur_hours)
    generated = per_hour * effective_hours
```

The resulting effective mark balance is:

```text
effective_marks = min(MARKS_CAP, stored_marks + generated)
```

Normative rules:

- Generation is staked-only. Liquid `balance_pwm` does not generate marks.
- Stake units are whole PWM: `whole_pwm_staked = floor(staked_pwm_raw / 1_000_000)`.
- The baseline rate remains `1 whole PWM * 1 hour = 1 mark`.
- `stored_marks == MARKS_CAP` is saturated; further generation is a no-op until marks are burned.
- `satur_hours` MUST use **integer ceiling** (`ceil`), not floor, when `remaining > 0` and `per_hour > 0`. Floor would yield `satur_hours = 0` whenever `remaining < per_hour`, permanently stalling below `MARKS_CAP` (e.g. 1 whole PWM staked at 1M PWM-equivalent rate stops ~967k marks short after the first saturation window).
- The final `effective_marks = min(MARKS_CAP, stored_marks + generated)` clamp absorbs the at-most-one-hour overshoot from ceiling; implementations MUST NOT rely on floor for saturation budgeting.
- Implementations MUST use checked or saturating arithmetic so the cap is reached through `satur_hours` + final clamp, not through integer overflow.

### Why `satur_hours` exists (design note)

`satur_hours` serves two purposes beyond the baseline rate (`1 whole PWM × 1 hour = 1 mark`):

1. **Correct saturation** — integer ceiling guarantees accounts can reach `MARKS_CAP` even when `remaining < per_hour` (see floor stall above).
2. **Protocol extension without widening intermediate math** — a fork or future genesis MAY raise `marks_per_coin_per_hour` well above the V5 default while keeping lazy-mark **intermediate** arithmetic in **u64**, as long as parameters stay within the bounds in [Implementation profile: u64 lazy marks](#implementation-profile-u64-lazy-marks-informative) below.

Without `satur_hours`, an implementation might compute `generated = per_hour × delta_hours` directly. A long idle period (`delta_hours` up to chain lifetime) multiplied by a raised reproduction rate can overflow u64 **before** the `MARKS_CAP` clamp — even though the economically meaningful result is still capped at `MARKS_CAP`. `satur_hours` bounds each touch to at most `remaining + per_hour` worth of generation (plus the final clamp), so hot-path math stays in native u64 on common 64-bit targets.

This is intentional: higher reproduction coefficients are a **genesis/governance knob**, not a requirement to adopt u128 in the lazy-marks fast path for every deployment.

### Implementation profile: u64 lazy marks (informative)

Implementations MAY compute lazy marks entirely in **u64** when genesis/runtime parameters satisfy:

```text
per_hour = whole_pwm_staked * marks_per_coin_per_hour
per_hour ≤ 2^64 − 1
```

With the normative `satur_hours` formula, a single touch then satisfies:

```text
generated ≤ remaining + per_hour ≤ MARKS_CAP + per_hour
```

so all intermediate products in the lazy-marks path fit in u64 whenever `per_hour` fits in u64.

**Conservative bound (21B PWM supply scale):** if a single account may stake the entire circulating whole-PWM supply `S` (design reference: 21×10⁹ whole PWM), then:

```text
marks_per_coin_per_hour ≤ floor((2^64 − 1) / S) ≈ 8.78×10⁸   (S = 21×10⁹ whole PWM)
```

For smaller concentrated stakes the allowable rate is higher (e.g. 100×10⁶ whole PWM staked permits rates up to ~1.8×10¹⁰ marks per whole PWM per hour). V5 default genesis (`marks_per_coin_per_hour = 1`) and realistic fork experiments (e.g. 10³–10⁶) remain far below these limits for whale-scale stakes.

If a deployment's genesis parameters may violate `whole_pwm_staked * marks_per_coin_per_hour ≤ 2^64 − 1`, implementations MUST either reject the config at genesis load or use wider intermediate arithmetic (e.g. u128) for lazy marks only. **`staked_pwm_raw` on `Account` remains u128** for token raw amounts; this profile applies only to the lazy-marks accumulation path.

Normative bounds review artifact: `docs/reviews/20260524-v5-marks-u64-arithmetic-bounds-review.md`.

## Touch Semantics

Lazy marks are applied when an account participates in a state transition that can affect balances, marks, or policy state:

- `INIT`: initializes `marks_last_block` to the inclusion height and starts with zero generated marks.
- `TRANSFER`: touches sender and recipient.
- `STAKE`: touches the owner before stake balance changes.
- `UNSTAKE`: touches the owner before stake balance changes.
- `BURN_MARK`: touches the owner before checking/burning marks.
- `PolicyTx`: touches the target account before policy state changes.
- Snapshot/replay loaders: preserve `marks_last_block` exactly for schema v3+ snapshots.

Touch means:

1. Compute `effective_marks` at the transition height.
2. Store `stored_marks = effective_marks`.
3. Store `marks_last_block = current_height`.
4. Apply the base transaction state mutation.

For display-only surfaces such as CLI/TUI, clients MAY compute `effective_marks` against the latest known head height without mutating state.

## Validation Semantics

The following RFC 0012 v1 invariants are retired in V5:

- anchor monotonicity;
- anchor continuity checks;
- over-claim checks;
- one free claim per UTC day;
- `CLAIM_ALL = u32::MAX` sentinel handling.

The V5 validation invariant is the saturation clamp:

- a state transition MUST NOT produce `stored_marks > MARKS_CAP`;
- zero stake or zero rate generates zero marks and MUST NOT fail;
- saturated accounts remain valid and simply generate no additional marks until burn.

## Compatibility

RFC 0011 and RFC 0013 carry V5 addenda that remove `ClaimTx` from active scope. Historical V2 documentation remains useful as migration context only.

RFC 0007 remains the transaction/state umbrella RFC, but RFC 0012 v2 is normative for the active V5 marks submodel and retires `marks_quota`/anchor-era claim fields from the active state contract.

V5 code slices MUST NOT add a new explicit claim transaction to replace retired `ClaimTx`. Any future claim-like mechanism must be a new ADR/RFC.

## Out-of-Scope

- Runtime enforcement of address flags and conservation delayed transfer.
- Domain lease auction runtime.
- Production off-chain IPv4 claim registry.
- PoS validator admission.

## References

- [RFC 0011: Burn purpose and Claim transaction schema](./11-burn-purpose-and-claim-tx.md)
- [RFC 0013: Claim policy matrix](./13-claim-policy-matrix.md)
- [RFC 0019: Float inflation](./19-float-inflation.md)
- [MVP v5 plan](../plans/mvp_v5.md)
