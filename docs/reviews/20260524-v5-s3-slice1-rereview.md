# V5-3 Slice1 — Re-review after review-fixes (`b28e02f..9086d96`)

## 1. Scope recap

- **Ticket:** `20260524-v5-s3-slice1-rereview`
- **Parent:** `20260524-v5-sprint3-lazy-marks-inflation`
- **Prior review:** `docs/reviews/20260524-v5-s3-slice1-pure-fns-review.md` (REQUEST\_CHANGES on **test naming only**; formula semantics reported PASS).
- **Review-fixes ticket:** `tasks/done/20260524-v5-s3-slice1-review-fixes.json` → coding **PASS**, commit **`9086d96`**.
- **Base slice coding:** `tasks/done/20260524-v5-s3-slice1-pure-fns.json` → **`b28e02f`**.
- **Integrated diff:** `b28e02f..9086d96`.
- **Checklist anchor:** `docs/plans/mvp_v5.md#sprint-v5-3-lazy-marks-engine--float-inflation`
- **RFC anchors:** `docs/rfc/12-claim-maturity-and-state-model.md`, `docs/rfc/19-float-inflation.md`

**Claimed product scope:** `crates/pwm-core/src/marks.rs`, `crates/pwm-core/src/lib.rs` (slice1). **Actual delta in `9086d96`:** only `marks.rs` (rename + short comment).

## 2. Requirements fit — closure of prior REQUEST\_CHANGES

Prior blocking items (four `#[test]` names over the **≤5** `snake_case` segment cap) are addressed per the mapping in the review-fixes ticket:

| Prior (violating) | Current (integrated tree) |
| --- | --- |
| `marks_1m_pwm_reaches_u32_max_with_ceil_satur_hours` | `marks_1m_pwm_ceil_cap` |
| `inflation_neutral_ppm_uses_base_emission` | `inflation_neutral_ppm_base` |
| `inflation_zero_ppm_falls_back_to_block_reward` | `inflation_zero_ppm_fallback` |
| `inflation_saturating_mul_does_not_overflow` | `inflation_sat_mul_no_ovf` |

A single-line comment was added above `marks_1m_pwm_ceil_cap` documenting the ceil / large-stake intent where the longer name carried nuance.

**Formula semantics:** `git diff b28e02f..9086d96` changes **do not alter** `compute_lazy_marks` or `compute_block_reward` bodies; semantics remain as validated on **`b28e02f`** and in the prior review.

## 3. Style and module shape

- **Production symbols** in `marks.rs`: public fns remain short (`compute_lazy_marks`, `compute_block_reward` — within production segment policy).
- **Tests:** all `#[test]` fns in `marks.rs` are within the hard cap after rename; tooling output cited under Tests.
- **Slice shape:** No new large blobs or façade churn; **`lib.rs` unchanged** across `9086d96`.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

No change from prior slice1 assessment:

- Pure arithmetic only; no IO, network, or new trust boundaries.
- Continued use of saturating multiply in reward path and saturating accumulation with `u32::MAX` clamp in marks path.
- No new panic-prone `unwrap` observed in touched code paths.

## 5. Tests

**Automation (mandatory checker):**

- `python scripts/check_entity_name_segments.py crates/pwm-core/src/marks.rs` → **`violations: []`** for the file under current policy (`prod_max: 4`, `test_max: 5`).

**Independent re-run:**

- `cargo test -p pwm-core marks_ --lib` → **PASS** (note: substring filter matches other `marks_` tests in the crate beyond this module; acceptable for sanity check).
- `cargo test -p pwm-core inflation_ --lib` → **PASS** (three tests in `marks::tests`).

**Integrated diff evidence:** **`9086d96`** touches **only** `crates/pwm-core/src/marks.rs`; no `state.rs` / `chain.rs` edits — slice2/3 boundary preserved for this delta.

### Re-validation of prior PASS items (logical, on current tree)

1. **`compute_lazy_marks`:** `satur_hours` uses integer ceiling via `ceil_div_u128(remaining, per_hour)`; effective hours capped with `min(satur_hours, delta_hours)`; final value clamped to `u32::MAX`.
2. **1M PWM cap:** `marks_1m_pwm_ceil_cap` still asserts **`u32::MAX`** under the documented stake and height configuration.
3. **`compute_block_reward`:** Matches RFC-style shape: **`season_coeff_ppm == 0`** returns **`gen_cfg.block_reward`**; otherwise `base_emission_per_block * season_coeff_ppm / 1_000_000` with saturating multiply. Tests cover neutral ppm (`1_000_000`), zero fallback, and saturating multiply toward overflow avoidance.

## 6. Verdict

- **APPROVE** — prior **REQUEST\_CHANGES** resolved (test naming + segment checker clean); **`9086d96`** is rename/comment-only relative to **`b28e02f`**; formula semantics and slice boundary unchanged.

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/20260524-v5-s3-slice1-rereview.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 8000
  confidence: low
```

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-s3-slice1-rereview.md'
git add 'tasks/20260524-v5-s3-slice1-rereview.json'
git commit -m 'docs(v5-3): slice1 rereview after naming fixes'
```

**Verdict (one-line):** APPROVE — REQUEST\_CHANGES closed; integrated diff rename-only; segment checker PASS.
