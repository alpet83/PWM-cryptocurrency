# V5-3 Slice3 Review: compute_block_reward in Chain::seal

## 1. Scope recap

- Ticket: 20260524-v5-s3-slice3-chain-seal-review
- Parent: 20260524-v5-sprint3-lazy-marks-inflation
- Reviewed coding commit: 02ea946
- Claimed scope in commit file list:
  - crates/pwm-core/src/chain.rs
  - crates/pwm-core/src/genesis.rs
- Checklist anchor: docs/plans/mvp_v5.md#sprint-v5-3-lazy-marks-engine--float-inflation
- RFC anchors reviewed:
  - docs/rfc/19-float-inflation.md
  - docs/rfc/12-claim-maturity-and-state-model.md

## 2. Requirements fit

- V5/non-legacy seal path now uses compute_block_reward as primary source:
  - Chain::seal imports marks::compute_block_reward.
  - For non-legacy policy: rew = compute_block_reward(&self.cfg, height).
- season_coeff_ppm == 0 fallback remains preserved through compute_block_reward implementation (verified by existing inflation tests).
- Legacy path is explicit and preserved:
  - if cfg.is_legacy_policy() then reward_producer(cfg.block_reward) is unchanged.
- Slice boundary with slice2 is respected:
  - Commit touches only chain.rs and genesis.rs; no state.rs changes.
- Seal integration test added and passing:
  - chain::tests::policy_v2_uses_float_reward validates expected reward amount under V2 path.

## 3. Style and module shape

- Naming policy check on touched files reports no violations.
- Change is minimal and localized to reward-source wiring and one integration test.
- No new module-shape regressions observed.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

- Arithmetic path remains deterministic and integer-only.
- Avoids duplicated seasonal formula in seal path by reusing compute_block_reward.
- No new external trust boundary or deserialization surface introduced.

## 5. Tests

- Independent review validation:
  - cargo test -p pwm-core policy_v2_uses_float_reward --lib PASS
  - cargo test -p pwm-core inflation_ --lib PASS
  - cargo check --workspace PASS
- Existing inflation helper tests confirm neutral/zero-ppm/saturation behavior remains valid.

## 6. Verdict

- APPROVE

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/20260524-v5-s3-slice3-chain-seal-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 15000
  confidence: low
```

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-s3-slice3-chain-seal-review.md'
git add 'tasks/20260524-v5-s3-slice3-chain-seal-review.json'
git commit -m 'docs(v5-3): slice3 chain-seal review and traceability'
```

Verdict: APPROVE.