# V5-3 Slice1 Review: pure lazy marks + inflation fns

## 1. Scope recap

- Ticket: `20260524-v5-s3-slice1-pure-fns-review`
- Parent: `20260524-v5-sprint3-lazy-marks-inflation`
- Claimed coding commit: `b28e02f`
- Claimed scope:
  - `crates/pwm-core/src/marks.rs`
  - `crates/pwm-core/src/lib.rs`
  - Unit tests for `marks_*` and `inflation_*`
- Checklist anchor: `docs/plans/mvp_v5.md#sprint-v5-3-lazy-marks-engine--float-inflation`
- RFCs reviewed:
  - `docs/rfc/12-claim-maturity-and-state-model.md`
  - `docs/rfc/19-float-inflation.md`

## 2. Requirements fit

- `compute_lazy_marks` matches RFC 0012 v2 core behavior:
  - Uses `delta_hours = delta_blocks / blocks_per_hour`.
  - Uses whole-stake units via `staked_pwm_raw / PWM_RAW_SCALE`.
  - Uses integer ceiling saturation budget (`ceil_div_u128(remaining, per_hour)`).
  - Final cap is clamped via `min(u32::MAX, ...)`.
- 1M PWM saturation evidence is present and passing:
  - Test `marks_1m_pwm_reaches_u32_max_with_ceil_satur_hours` passes.
- `compute_block_reward` matches RFC 0019:
  - `season_coeff_ppm == 0` fallback to `block_reward` is implemented.
  - Otherwise uses deterministic integer formula with saturating multiply before division.
- Slice boundary respected:
  - Commit file list includes only `marks.rs` and `lib.rs`.
  - No `state.rs` / `chain.rs` changes in reviewed commit.

## 3. Style and module shape

- Positive:
  - New source module has concise public function names.
  - `lib.rs` module/export wiring is minimal and aligned with current crate structure.
- Findings (naming policy):
  - `crates/pwm-core/src/marks.rs:87` test fn `marks_1m_pwm_reaches_u32_max_with_ceil_satur_hours` has 10 segments (limit for tests is 5).
  - `crates/pwm-core/src/marks.rs:120` test fn `inflation_neutral_ppm_uses_base_emission` has 6 segments (limit 5).
  - `crates/pwm-core/src/marks.rs:129` test fn `inflation_zero_ppm_falls_back_to_block_reward` has 8 segments (limit 5).
  - `crates/pwm-core/src/marks.rs:139` test fn `inflation_saturating_mul_does_not_overflow` has 6 segments (limit 5).
  - Severity: medium (systemic naming-policy mismatch in newly added tests).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

- No new IO, networking, filesystem, or external trust-boundary handling in this slice.
- Defensive arithmetic is used where expected:
  - saturating multiply in marks and reward paths,
  - explicit early returns for zero-rate / zero-stake / zero blocks-per-hour guards.
- No new panic-prone `unwrap` usage was introduced in the reviewed code.

## 5. Tests

- Independent verification run:
  - `cargo test -p pwm-core marks_ --lib` PASS
  - `cargo test -p pwm-core inflation_ --lib` PASS
  - `cargo check --workspace` PASS
- Coverage for this slice is adequate for formula-level behavior and saturation edge cases.
- Missing for later slices (not a blocker for slice1 itself): integration assertions in state-touch/chain-seal paths belong to slice2/3.

## 6. Verdict

- **REQUEST_CHANGES**
- Priority items:
  - Rename the four new test functions in `crates/pwm-core/src/marks.rs` to satisfy test naming cap (<= 5 `snake_case` segments).

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PARTIAL
artifacts:
  - docs/reviews/20260524-v5-s3-slice1-pure-fns-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 12000
  confidence: low
```

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-s3-slice1-pure-fns-review.md'
git add 'tasks/20260524-v5-s3-slice1-pure-fns-review.json'
git commit -m 'docs(v5-3): slice1 review report and traceability'
```

Verdict: REQUEST_CHANGES (formula semantics pass; naming policy violations in new tests must be fixed).