# Review Report: V5-7 Slice1 Account Info Marks

## 1. Scope recap

- Ticket: `20260524-v5-s7-slice1-account-info-marks-review`
- Commit reviewed: `ebeb161`
- Claimed scope: add `pwm account-info` with account/head fetch, effective marks at head, marks output fields, and tests.
- MVP anchor: `docs/plans/mvp_v5.md#sprint-v5-7-cli-enhancements--21b-genesis-design-doc`

## 2. Requirements fit

Status: covered.

- `account-info` command added with `--account` or `--wallet` path resolution.
- Runtime flow implemented: fetch `/v1/head`, fetch `/v1/account/:id`, compute effective marks via `compute_lazy_marks` at head height.
- Output includes required fields: `marks_stored`, `marks_effective`, `marks_sat_pct`, `marks_last_block`, `staked`, `head_height`.
- Zero-stake path covered by test and passes.

## 3. Style and module shape

Status: mostly aligned; one non-blocking scope-drift note.

- Naming policy check (`scripts/check_entity_name_segments.py`) on touched files reports zero violations.
- New source module has proper `//!` banner.
- Command naming and parser shape follow existing `pwm-cli` conventions.
- Scope drift note: commit includes `crates/pwm-cli/src/cmd_tx.rs` test-only helper `parse_claim_mode_cli` under `#[cfg(test)]`. This does not change runtime behavior and is acceptable, but it is outside declared slice1 objective and should be minimized in future slices.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

Status: no blocking safety findings.

- CLI errors are propagated with contextual messages for HTTP and JSON parse failures.
- `calc_sat_pct` handles `0` safely and bounds conversion with fallback.
- No new panic-prone paths in runtime command flow were identified.

## 5. Tests

Executed:

- `cargo check -p pwm-cli` -> PASS
- `cargo test -p pwm-cli account_info` -> PASS (5 tests)
- `python scripts/check_entity_name_segments.py crates/pwm-cli/src/main.rs crates/pwm-cli/src/cli_cmd.rs crates/pwm-cli/src/cli_dispatch.rs crates/pwm-cli/src/cmd_account.rs crates/pwm-cli/src/cmd_tx.rs crates/pwm-cli/src/tests/mod.rs` -> PASS (no violations)

Coverage notes:

- Added tests validate CLI parse for account/wallet variants.
- Added tests validate effective marks computation and JSON field parsing.
- No additional gaps blocking this slice were found.

## 6. Verdict

Verdict: approve with nits.

Priority findings:

1. Low: avoid unrelated test-helper touches in non-target modules (`cmd_tx.rs`) when slice scope is account-info only.

## 7. Participation / token estimate

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260524-v5-s7-slice1-account-info-marks-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 10500, "confidence": "low" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-s7-slice1-account-info-marks-review.md'
git add 'tasks/20260524-v5-s7-slice1-account-info-marks-review.json'
git commit -m 'docs(v5-7): slice1 account-info review gate report'
```