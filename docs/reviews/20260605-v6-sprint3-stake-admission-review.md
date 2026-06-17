# Review: V6-3 stake-gated validator admission

**Ticket:** `20260605-v6-sprint3-stake-admission-coding`  
**Branch / worktree:** `v6/20260605-v6-sprint3-stake-admission` @ `P:/opt/docker/PWM-cryptocurrency-worktrees/v6-sprint3-stake-admission`  
**Spec:** `docs/rfc/addenda/v6-rfc4-validators-stake-admission.md`, `docs/plans/mvp_v6.md` (V6-3)  
**Reviewer:** `pwm-review`  
**Date:** 2026-06-05

## 1. Scope recap

Slice **V6-3** (`mvp_v6.md`): enforce stake-gated **active** validator admission at epoch boundaries in `pwm-core` `Chain::seal`, keep full **registered** set in `GenCfg.vals`, and cover unit tests for:

- below-threshold validator excluded from proposer rotation;
- at-threshold validator included;
- mid-epoch stake change effective only after epoch rollover.

**Touched (uncommitted):**

| File | Role |
|------|------|
| `crates/pwm-core/src/chain.rs` | `recompute_active_idxs`, `pick_prod_idx`, `is_epoch_boundary`; `boot` seeds active set; `seal` epoch rollover + active-only proposer |
| `crates/pwm-core/src/genesis.rs` | `dev_net()` sets `min_validator_stake: 0` (V5-like devnet liveness) |
| `issues-report.md` | Documents devnet bootstrap / liveness trade-off |

Out of scope (expected): `pwmd` cluster path, RFC16 failover, snapshot replay of admission — deferred to later sprints.

## 2. Requirements fit

**Aligned with RFC4 V6 addendum:**

- **Active set derivation:** `recompute_active_idxs` walks `cfg.vals.set` by registered index, reads `staked_pwm_raw` on the bound validator account, includes index when `staked_pwm_raw >= min_validator_stake`. Registered set is not mutated.
- **Epoch boundary:** On seal, when `height % epoch_length_blocks == 0` (and `epoch_length_blocks > 0`), `epoch_counter` increments and `active_validator_indices` is recomputed before proposer pick.
- **Seal / proposer:** `pick_prod_idx` rotates over `active_validator_indices` only; `prod_idx` in the header remains a **registered** index (value from the active list), consistent with existing `prod_acct` / `prod_pk` lookup.
- **Empty active set:** `pick_prod_idx` returns `"no active validators for current epoch"` → `SealAbort`; matches RFC halt for devnet.
- **Mid-epoch stability:** `stake_change_rollover_only` shows val1 still proposes at h2 after stake zeroed post-h1; active set shrinks to `[0]` only after h3 boundary (`epoch_counter == 1`).
- **Bootstrap / devnet:** `dev_net()` `min_validator_stake = 0` recovers V5-like “all registered active” per RFC §7; stake gating exercised in dedicated tests with explicit threshold/stake setup. `issues-report.md` records the rationale.

**Minor spec/traceability notes (non-blocking):**

- RFC §4 requires boundary at `h > 0`; `is_epoch_boundary` omits an explicit `height > 0` guard. Current `seal` path only ever uses `height >= 1`, so behavior matches intent; an explicit guard would improve auditability.
- RFC text resolves stake “for pubkey”; implementation uses `row.acct` (existing validator binding). Equivalent under genesis invariants.
- Recompute at boundary uses **pre-tx** cloned state (before `apply_tx_with_ctx` in that block). RFC does not normatively fix ordering; this is consistent with “mid-epoch changes apply at next boundary” and is acceptable for V6.

**No gaps** against the stated V6-3 acceptance line (“below threshold excluded at epoch boundary; seal uses active set only”).

## 3. Style and module shape

- New helpers `recompute_active_idxs`, `pick_prod_idx`, `is_epoch_boundary` are small, private, and colocated in `chain.rs` — appropriate for slice scope.
- `chain.rs` retains `//!` module banner; English error strings.
- **`python scripts/check_entity_name_segments.py`** on diff paths: **no violations** (`prod_max: 4`, `test_max: 5`).
- Test name `prod_rotation_uses_vals_len` is now misleading (rotation goes through active set with `min_validator_stake = 0`); rename to something like `prod_rotation_two_active` would match behavior — **low nit**.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

- **No new panics** on hot path; empty active set surfaces as `Result`/`SealAbort`, not `unwrap`.
- **Admission filter** silently skips validators whose account is missing from state (`filter_map` + `?`). Genesis `boot` asserts validator accounts exist; runtime account deletion is out of V6 scope — acceptable with low residual risk.
- **DoS:** Recompute is O(registered validators) per boundary — bounded by static genesis set; no new unbounded structures.
- **Trust boundaries:** Slice is in-memory `pwm-core` only; no RPC/file-path changes.

## 5. Tests

**Present and passing** (reviewer ran `cargo test -p pwm-core -- stake_below stake_at_min stake_change_rollover prod_rotation` — 4/4 OK):

| Test | Coverage |
|------|----------|
| `stake_below_min_excluded` | val1 at 99 & threshold 100 → only val0 proposes |
| `stake_at_min_included` | both at 100 → rotation 0, 1 |
| `stake_change_rollover_only` | mid-epoch stake drop; active set updates at h3; `epoch_counter` |
| `prod_rotation_uses_vals_len` | regression with `min_validator_stake = 0` |

**Gaps (nits, not slice blockers):**

- No dedicated test that **all** validators below threshold → seal error (`no active validators…`). Behavior exists in `pick_prod_idx` but is untested.
- No test for `epoch_length_blocks = 1` boundary firing every block (implicitly exercised in below/at-min tests but not asserted on `epoch_counter`).

`pwm-testing` should run broader `pwm-core` / workspace regression on commit.

## 6. Verdict

**Approve with nits.**

Prioritized follow-ups for `pwm-coding` (optional, same or next slice):

1. **Low:** Add unit test `empty_active_set_seal_fails` (all stakes below min → `seal` Err).
2. **Low:** Rename `prod_rotation_uses_vals_len` to reflect active-set rotation.
3. **Low:** Add `height > 0` to `is_epoch_boundary` for RFC-traceable parity.

None require product/protocol owner decision.

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260605-v6-sprint3-stake-admission-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 28000
  confidence: low
```

**Verdict:** `APPROVE_WITH_NITS` — V6-3 stake admission enforcement in `Chain::seal` matches RFC4 and sprint acceptance; tests pass; minor test/traceability nits only.
