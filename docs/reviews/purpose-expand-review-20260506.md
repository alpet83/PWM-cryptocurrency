# Review: purpose placeholder expansion (`{utc_time}` / `{utc_timestamp}`)

**Commit:** `d2d51a2` — `feat(burn): purpose placeholder expansion {utc_time}/{utc_timestamp} in CLI+TUI`  
**Reviewer:** `pwm-review`  
**Date:** 2026-05-06

## 1. Scope recap

Burn `purpose` strings in **pwm-cli** (`tx-burn-mark`) and **pwm-tui** gain runtime expansion of `{utc_time}` and `{utc_timestamp}` before the dedication is committed in the signed transaction. New module `crates/pwm-cli/src/purpose_expand.rs` with unit tests; CLI and TUI call sites updated; `--purpose` help text documents placeholders. Aligns with v2 burn dedication / RFC 0011 UX described in the slice brief.

## 2. Requirements fit

| Checklist item | Assessment |
|----------------|------------|
| **`expand_purpose` correctness** | **Met.** Uses `std::time::{SystemTime, UNIX_EPOCH}` only — no extra crates. Timestamp is `duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()` (integer seconds). `{utc_time}` is formatted as `DD-MM-YY HH:MM:SSZ` (day, month, two-digit year, space, time, `Z`) via `fmt_utc_time` + Gregorian `civil_from_days`. Unknown `{...}` tokens are not matched by the two fixed `replace` calls — e.g. `{foo}` is unchanged. |
| **Placement (before purpose signing)** | **Met.** **CLI:** `expand_purpose` runs at `cmd_tx.rs` immediately before `tx.set_burn_purpose_signed(&source.sk, expanded)`. **TUI:** `expand_purpose` runs immediately before `tx.set_burn_purpose_signed(&sk, purpose)`. The earlier `SignedTx::sign_body` builds an initial BurnMark tx; `set_burn_purpose_signed` replaces `burn_purpose` and **re-signs** the full message (`pwm-core` `set_burn_purpose_signed`), so the signature that is sent binds to the **expanded** string. |
| **Idempotency / double expansion** | **Met.** Expansion is a single linear pass (two `replace` calls). Output is not run through `expand_purpose` again. No double substitution unless callers explicitly expand twice (they do not). |
| **TUI helper duplication** | **Nit (maintenance).** `tx_submit.rs` duplicates `expand_purpose`, `fmt_utc_time`, and `civil_from_days` from `purpose_expand.rs` (~same logic). Acceptable short-term to avoid crate coupling; risk is **drift** if one path is fixed and the other is not. Mitigation options (out of review scope): small shared helper in `pwm-core` or a tiny internal crate; or a single-line comment pointing to `purpose_expand.rs` as canonical (TUI file has algorithm body but no “keep in sync” banner at top of the duplicated block). |
| **Help text** | **Met.** `cli_cmd.rs` documents both placeholders with the intended semantics: `{utc_time} (DD-MM-YY HH:MM:SSZ)`, `{utc_timestamp} (Unix seconds).` |
| **Naming policy** (`check_rust_fn_name_segments.py`) | **Met.** No violations on `purpose_expand.rs`, `cmd_tx.rs`, `tx_submit.rs`. |
| **Unit tests in `purpose_expand.rs`** | **Mostly met.** Covers: numeric expansion for `{utc_timestamp}` (`is_digits`), structural validation for `{utc_time}` (length 18, separators at fixed positions, digits elsewhere — effectively a regex-shaped check), passthrough for unknown `{foo}`, and a combined template split on ` at `. **Gap (low):** no fixed golden vector for calendar math (e.g. known epoch second → exact string); shape tests reduce regression risk but do not prove `civil_from_days` against known dates. Acceptable for a small std-only helper. **Note:** TUI path has **no** parallel tests — parity relies on duplicated code staying identical to CLI tests. |

## 3. Style and module shape

- New `purpose_expand.rs` has a minimal English `//!` banner; `expand_purpose` is documented.
- Production identifiers stay within the stated segment budget (checker clean).

## 4. Safety

- `duration_since(UNIX_EPOCH).unwrap_or_default()` avoids panic on clock skew edge cases; yields zero timestamp if ever invalid (pathological).
- No new trust boundaries; purpose length / validation presumably enforced elsewhere when setting burn purpose (unchanged by this review’s scope).
- Gregorian helper is standard Howard-style arithmetic; no obvious panic paths in the hot path beyond normal integer arithmetic.

## 5. Tests

- ** pwm-cli:** four focused unit tests in `purpose_expand.rs` as listed above.
- **pwm-tui:** no new tests for duplicated expansion (noted as nit).

## 6. Verdict

**PASS WITH NITS** — behavior and ordering relative to `set_burn_purpose_signed` are correct; help and naming checks pass. Nits: duplicated expansion logic in TUI (drift risk); tests assert format shape more than calendar correctness; TUI lacks direct tests.

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS-WITH-NITS
artifacts: docs/reviews/purpose-expand-review-20260506.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 3800
  confidence: low
```

## Summary table

| Area | Verdict |
|------|---------|
| Correctness & std-only timestamp | Pass |
| Order vs `set_burn_purpose_signed` | Pass |
| Double expansion | Pass |
| Help text | Pass |
| Naming script | Pass |
| TUI duplication / test gap | Nits |

**One-line verdict for orchestrator:** `PASS-WITH-NITS` — approved; consolidate or document duplicated TUI expansion; optional stronger date tests.

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/purpose-expand-review-20260506.md'
git add 'tasks/20260506-purpose-placeholders.json'
git commit -m 'docs: purpose-expand pwm-review report'
```
