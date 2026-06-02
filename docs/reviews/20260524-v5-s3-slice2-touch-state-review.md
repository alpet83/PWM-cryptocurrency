# V5-3 Slice2 Review: lazy marks touch integration

## 1. Scope recap

- Ticket: `20260524-v5-s3-slice2-touch-state-review`
- Parent: `20260524-v5-sprint3-lazy-marks-inflation`
- Reviewed coding commit: `80aeccc`
- Claimed scope verified in commit file list:
  - `crates/pwm-core/src/state.rs`
  - `crates/pwm-core/src/chain.rs`
  - `crates/pwmd/src/api/handlers_tx.rs`
  - `crates/pwmd/src/lifecycle.rs`
  - `crates/pwmd/src/snapshot/ch_http.rs`
  - `crates/pwmd/src/snapshot/io.rs`
  - `crates/pwmd/src/snapshot/repair.rs`
  - `crates/pwmd/src/tests/http_status.rs`
  - `crates/pwmd/src/transport/peer_session/mod.rs`
  - `crates/pwmd/src/transport/peer_session/sync_live.rs`
- Checklist anchor: `docs/plans/mvp_v5.md#sprint-v5-3-lazy-marks-engine--float-inflation`
- RFC anchor checked: `docs/rfc/12-claim-maturity-and-state-model.md`

## 2. Requirements fit

- Touch helper replacement is present:
  - `touch_acct_mrks(acc, inclusion_height, gen_cfg)` writes `stored_marks = compute_lazy_marks(...)` then `marks_last_block = inclusion_height`.
- Touch matrix checks pass:
  - `Transfer`: sender touched before debit; recipient touched after `require_recipient` before credit.
  - `Stake/Unstake`: account touched before stake/balance mutation.
  - `BurnMark`: account touched before marks sufficiency check/debit.
  - `PolicyTx`: target account touched before policy mutation and fee handling.
  - `Init`: `marks_last_block` set to inclusion height (height-only initialization).
- Legacy path replaced:
  - `matured_units_available` and `apply_auto_claim` removed from active path.
  - No `DEF_BLOCKS_PER_HOUR` usage in lazy marks generation path.
- GenCfg plumbing verified:
  - `apply_tx_with_ctx` and precheck wrappers now accept `&GenCfg`.
  - Core and `pwmd` call sites are updated and compile.
- Slice boundary to slice3 respected:
  - `chain.rs` only passes cfg into apply path; no `compute_block_reward` wiring in `Chain::seal`.

## 3. Style and module shape

- Naming policy check executed on all claimed touched files: no violations.
- New helper and changed signatures remain concise and consistent with module responsibilities.
- No new oversized façade blob introduced by this slice.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

- Marks update path is deterministic and pure-function based (`compute_lazy_marks`).
- No new external trust boundary or network deserialization surface introduced in reviewed logic.
- No obvious panic/overflow regressions in touched marks flow; saturating behavior remains in compute path from slice1.

## 5. Tests

- Independent checks run during review:
  - `cargo test -p pwm-core marks_ --lib` PASS
  - `cargo check --workspace` PASS
- Existing `state::tests::marks_*` and `marks::tests::marks_*` coverage confirms touch + saturation behavior remains green.

## 6. Verdict

- **APPROVE**

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/20260524-v5-s3-slice2-touch-state-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 16000
  confidence: low
```

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260524-v5-s3-slice2-touch-state-review.md'
git add 'tasks/20260524-v5-s3-slice2-touch-state-review.json'
git commit -m 'docs(v5-3): slice2 touch-state review and traceability'
```

Verdict: APPROVE.