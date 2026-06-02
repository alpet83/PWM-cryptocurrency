# V5-2 Slice 1 Review: GenCfg + ClaimPhaseConfig

## 1. Scope recap

Reviewed V5-2 slice1 after coding and testing PASS for the `GenCfg` / `ClaimPhaseConfig` extension in [crates/pwm-core/src/genesis.rs](../../crates/pwm-core/src/genesis.rs) and the export in [crates/pwm-core/src/lib.rs](../../crates/pwm-core/src/lib.rs).

Claimed scope from the coding/testing tickets:

- add `blocks_per_hour`, `marks_per_coin_per_hour`, `base_emission_per_block`, `season_coeff_ppm`, and `ipv4_claim_phases` to `GenCfg`;
- add `ClaimPhaseConfig`;
- keep public JSON `u128` values on decimal-string encoding;
- avoid Account / TxBody / snapshot behavior changes in this slice.

## 2. Requirements fit

The slice mostly lands the intended config surface:

- `ClaimPhaseConfig` was added and exported;
- JSON round-trip coverage was added for the new config fields;
- decimal-string JSON encoding is enforced for the new `u128` public fields;
- the slice stays inside the intended module boundary.

However it does not fully match RFC 0019 as currently written.

## 3. Style and module shape

The change is compact and localized. Test coverage is adjacent to the implementation, which is the right shape for this slice.

There is one naming inconsistency worth noting: the JSON field is `marks_per_coin_per_hour`, but the Rust field name is `marks_per_hour`. That is acceptable at runtime because serde renames it correctly, but it increases drift between code terminology and the V5 plan/RFC wording.

### Wire JSON / u128

Applicable.

This slice touches public JSON/config fields carrying large integers:

- `base_emission_per_block: u128`
- `block_reward: u128`
- `marks_coeff: u128`
- `pwm_stake_min: u128`
- `marks_stake_min: u128`
- `ClaimPhaseConfig.allocation: u128`

The implementation uses `ser_json_u128` for these public JSON/config surfaces, and tests verify decimal-string output. That part matches the V5-1 rereview contract.

`season_coeff_ppm` is not a `u128` wire hazard itself, but its type still matters for RFC conformance.

## 4. Safety

Findings:

1. Medium: [crates/pwm-core/src/genesis.rs](../../crates/pwm-core/src/genesis.rs) keeps `season_coeff_ppm` as `u128`, while [docs/rfc/19-float-inflation.md](../rfc/19-float-inflation.md) specifies `season_coeff_ppm: u64`. This is a spec/implementation mismatch in the exact slice whose acceptance criteria say GenCfg must match RFC 0019. It is unlikely to cause an immediate runtime fault, but it weakens the freeze contract before later V5 slices build on this config type.

## 5. Tests

Evidence reviewed:

- coding handoff in [tasks/done/20260524-v5-s2-slice1-gencfg.json](../../tasks/done/20260524-v5-s2-slice1-gencfg.json)
- testing handoff in [tasks/done/20260524-v5-s2-slice1-gencfg-testing.json](../../tasks/done/20260524-v5-s2-slice1-gencfg-testing.json)
- commit `c11840d`
- targeted tests reported by pwm-testing:
  - `genesis::tests::gen_cfg_json_round_trip`
  - `genesis::tests::gen_cfg_defaults_sparse_json`
  - `tx::tests::signed_tx_json_roundtrip_u128`

The automated validation is good for this slice. The remaining issue is contract alignment, not missing execution coverage.

## 6. Verdict

Request changes.

Priority:

1. Align `season_coeff_ppm` with RFC 0019 by either changing the code to `u64` or explicitly updating the RFC/plan if `u128` is intentional.

Non-blocking nit:

- consider renaming the Rust field from `marks_per_hour` to `marks_per_coin_per_hour` in a follow-up if the team wants code terminology to mirror the frozen V5 contract exactly.

## 7. Participation / token estimate

```text
agent: pwm-review
result: FAIL
artifacts: docs/reviews/20260524-v5-s2-slice1-gencfg-review.md
token_usage: { "source": "estimate", "input": 15000, "output": 1800, "total": 16800, "confidence": "medium" }
```

## 8. Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-s2-slice1-gencfg-review.md'
git add 'tasks/20260524-v5-s2-slice1-gencfg-review.json'
git commit -m 'docs(v5-2): add slice1 review gate report'
```