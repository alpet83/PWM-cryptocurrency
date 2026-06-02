# RFC 0019: Float inflation and seasonal block reward

**Status:** Active (V5-1 normative freeze)  
**Version:** 1.0  
**Depends on:** RFC 0007, MVP v5 plan

## Abstract

V5 replaces a fixed block reward with a deterministic floating emission formula. The block reward is derived from a base emission per block and a seasonal coefficient in parts per million (ppm). The target policy is approximately 5% annual inflation under the current stake-participation assumptions.

## Motivation

- PWM needs continuing emission for staking and operational liquidity without turning marks into a speculative asset.
- The Whitepaper calls for roughly 5% annual inflation with seasonal modulation.
- The consensus path should compute reward with pure integer arithmetic and no wall-clock side effects.

## Parameters

Genesis/config provides:

```text
base_emission_per_block: u128
season_coeff_ppm: u64       // 1_000_000 means neutral 1.0x
legacy_block_reward: u128?  // optional migration fallback for older dev genesis configs
```

Public JSON/config surfaces that expose `base_emission_per_block` or `legacy_block_reward` MUST encode these `u128` values as decimal strings, consistent with RFC 0007 fee encoding and RFC 0012 `staked_pwm_raw` guidance.

`season_coeff_ppm` is expressed in parts per million:

- `1_000_000` = 100%;
- `950_000` = 95%;
- `1_050_000` = 105%.

## Reward Formula

For each sealed block:

```text
if season_coeff_ppm == 0:
    reward = fallback_reward
else:
    reward = floor(base_emission_per_block * season_coeff_ppm / 1_000_000)
```

`fallback_reward` is:

```text
fallback_reward = legacy_block_reward.unwrap_or(base_emission_per_block)
```

The fallback exists only to avoid old devnet genesis files accidentally producing zero rewards when `season_coeff_ppm` was absent or serialized as zero. Production genesis SHOULD set `season_coeff_ppm = 1_000_000` or another explicit non-zero value.

## Annual Target

The V5 economic target is approximately 5% annual emission:

```text
annual_emission ~= circulating_or_float_supply * 0.05
base_emission_per_block ~= annual_emission / expected_blocks_per_year
```

`expected_blocks_per_year` is derived from the chain's configured block cadence. This RFC fixes the reward formula, not the one-time production calibration of `base_emission_per_block`.

## Consensus Rules

- `compute_block_reward(gen_cfg)` MUST be a pure deterministic function.
- The seal/apply path MUST use the computed reward, not a stale fixed reward field, once V5 is active.
- Integer overflow MUST be prevented by checked or saturating arithmetic before division.
- `season_coeff_ppm = 0` MUST trigger the explicit fallback above; it MUST NOT silently set production reward to zero.
- Seasonal coefficient changes are governance/config changes and must be auditable in genesis or accepted protocol state.

## Out-of-Scope

- Validator admission and stake-weighted proposer selection.
- Domain lease revenue distribution.
- Monetary policy governance beyond bounded coefficient updates.

## References

- [MVP v5 plan](../plans/mvp_v5.md)
- [Draft Whitepaper RU](../../DRAFT_WHITEPAPER-ru.md)
