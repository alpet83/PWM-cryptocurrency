# Review: V7-4 emergency stake evacuation in apply_tx (a727fa4)

- date: 2026-06-29
- ticket: `20260629-v7-4-stake-evac-review`
- coding_ticket: `20260629-v7-4-stake-evac`
- commit: `a727fa4` (verify at review time via branch `main`)
- adr: `docs/adr/0012-emergency-stake-evacuation.md`
- scope: `crates/pwm-core/src/state.rs`, `docs/runbooks/v6-owner-stability-soak-50k.md`

## 1. Scope recap

V7-4 implements ADR 0012: on successful `routing.emergency_redirect` `ActivatePolicy`, evacuate victim **`staked_pwm_raw`** to `activation_target.balance_pwm` in the same `apply_tx` as liquid evacuation. No wire change, no new tx type. Runbook step 8 oracle updated for V7.

## 2. Requirements fit

| Focus area | Verdict | Evidence |
|------------|---------|----------|
| 1. Stake evac placement + atomicity | **PASS** | After `apply_policy_action` + `pending_conservation` clear; liquid then stake in one block (`state.rs:711-728`); single `accounts.insert(id, a)` (`:733`) — all-or-nothing within successful `apply_tx` |
| 2. `staked_pwm_raw` zeroing | **PASS** | `a.staked_pwm_raw = 0` when `stake > 0` (`:724-726`); victim `finalized` (`apply_policy_action :888-890`); `is_finalized_blocked` blocks `Unstake` (`:946-954`) |
| 3. Validator set side effects | **PASS** (same as `Unstake`) | Organic `Unstake` only adjusts account fields (`:521-533`); `active_validator_indices` recomputed at epoch boundary via `roll_epoch_if_needed` → `recompute_active_idxs` (`chain.rs:68-72`, `:40-53`) reading `staked_pwm_raw`. Emergency path matches — no separate inline helper required |
| 4. `emergency_activation_sweep_ok` | **PASS** | Liquid 1234 + stake 777 → rescue (`:4003`); victim balance/stake zero (`:4001-4002`); finalized (`:4000`) |
| 5. `emergency_activation_no_stake` | **PASS** | `staked_pwm_raw = 0` setup (`:4033`); rescue gets liquid only (`:4065`); V6 regression preserved |
| 6. Pre-existing PARTIAL issues | **PASS** (not introduced) | Diff confined to `pwm-core/state.rs` evacuation block + tests + runbook; no fmt churn, no pwmd seal/event paths |

## 3. Implementation analysis

### Ordering (ADR §Ordering)

```709:733:crates/pwm-core/src/state.rs
                let evac_target = emergency_act_target(action);
                apply_policy_action(&mut a, action, inclusion_height)?;
                if let Some(target) = evac_target {
                    self.pending_conservation.retain(|row| row.sender != id);
                    let amount = a.balance_pwm;
                    let stake = a.staked_pwm_raw;
                    if target != id {
                        let target_acc = self.accounts.get_mut(&target).expect("activation target validated");
                        if amount > 0 { /* credit liquid, zero balance */ }
                        if stake > 0 {
                            target_acc.balance_pwm = target_acc.balance_pwm.saturating_add(stake);
                            a.staked_pwm_raw = 0;
                        }
                    }
                }
                a.balance_pwm -= *fee;
                a.nonce += 1;
                self.accounts.insert(id, a);
```

Matches ADR: validate (earlier) → activate/finalize → clear pending → evac balance → evac stake → fee/nonce.

- **All-or-nothing stake:** entire `staked_pwm_raw` credited (no partial evac) — ADR non-goal respected.
- **`target != id` guard:** self-target skips evacuation; preflight `validate_pol_action` requires `activation_target == rescue_id` (`:1080-1083`) — normal path always evacuates to rescue.
- **Supply conservation:** stake moves to rescue `balance_pwm`; no mint/burn.

### Validator set

Ticket asks for "same helpers as Unstake." **Neither arm calls a validator helper at apply time** — epoch admission observes post-tx `staked_pwm_raw` on next `roll_epoch_if_needed`. Implementation is consistent with existing unstake semantics; not a blocker.

### Tests

| test | result |
|------|--------|
| `emergency_activation_sweep_ok` | **PASS** (Windows `cargo.exe`, exit 0) |
| `emergency_activation_no_stake` | **PASS** (exit 0) |

**Gap (nit):** no test that owner with validator stake ≥ `min_validator_stake` drops from `active_validator_indices` after activation + epoch roll.

### Runbook

`v6-owner-stability-soak-50k.md` step 8 (`:413-419`) documents V6 vs V7 oracle (victim `staked=0`, rescue balance includes evacuated stake; activate-after-stake allowed on V7).

## 4. Style and module shape

- Evacuation extends existing `Policy` arm inline — ADR simplicity gate ("one code path").
- `emergency_act_target` helper unchanged pattern (`:1008-1016`).
- Production identifiers within policy.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 5. Safety

- No `unsafe`; evacuation gated on validated emergency activation + rescue cosign.
- `saturating_add` on rescue balance — no wrap.
- Finalized victim cannot issue follow-up `Unstake` — funds not double-movable.

## 6. Tests

Core emergency suite extended; focused new tests pass. CY e2e extension noted in ADR — out of this commit scope.

## 7. Concurrency / parallelism

Not in diff scope (spot-check only: `apply_tx` remains single-threaded state mutation per seal; no new shared-state surfaces observed).

## 8. BLOCKERs

None. Stake evacuation is atomic with liquid evac within one successful `apply_tx`; validator semantics align with organic `Unstake`.

## 9. Nits (non-blocking)

1. **NIT-1:** Add unit test: validator account with `staked_pwm_raw >= min_validator_stake`, emergency activation, `roll_epoch_if_needed` → assert index dropped.
2. **NIT-2:** ADR test name `emergency_activation_sweep_includes_stake` vs impl name `emergency_activation_sweep_ok` — doc naming drift only.

## 10. Verdict

**Approve with nits** — ADR 0012 correctly implemented; ordering and atomicity sound; regression test for zero-stake case present; runbook updated. Validator set behavior matches existing `Unstake` epoch model.

## 11. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260629-v7-4-stake-evac-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 36000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260629-v7-4-stake-evac-review.md'
git commit -m 'docs(v7-4): emergency stake evacuation review (a727fa4)'
```