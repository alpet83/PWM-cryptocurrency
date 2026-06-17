# ADR 0012: Emergency activation stake evacuation (V7)

## Status

**Accepted as V7 normative contract.** Extends [ADR 0011](0011-policy-activation-target.md) (V6 balance-only evacuation). **Implementation deferred to V7** — runtime in V6 remains ADR 0011 non-goal.

## Context

V6 emergency routing (`routing.emergency_redirect` + `ActivatePolicy` with `activation_target`) evacuates only spendable **`balance_pwm`** to rescue in the same `apply_tx` ([ADR 0011](0011-policy-activation-target.md) §Emergency routing binding). **`staked_pwm_raw`** stays on the finalized victim account.

Operator soak (2026-06) showed a realistic failure mode: victim may **`Stake`** before rescue activation. After `finalized`, ordinary **`Unstake`** is rejected (`PolicyAccountFinalized`), so staked funds are **permanently inaccessible** under V6 — contradicting the product intent that emergency routing should recover **all liquid value the victim could have moved**, including value locked only in stake.

Owner decision: **stake evacuation is implied** by emergency routing semantics; V6 documents the gap; **V7 implements** atomic unstake-and-credit alongside balance evacuation.

## Decision

### Semantics (V7)

When `ActivatePolicy` succeeds for `routing.emergency_redirect` with valid `activation_target` (same preconditions as ADR 0011):

1. Existing V6 steps unchanged: activate policy, set `finalized`, cancel victim `pending_conservation`, evacuate **`balance_pwm`** to `activation_target`.
2. **Additionally:** if victim `staked_pwm_raw > 0`, in the **same** `apply_tx` (after policy activation, before nonce/fee finalize):
   - debit victim `staked_pwm_raw` to `0`;
   - credit **`activation_target.balance_pwm`** by the unstaked amount (same units as ordinary `Unstake` → liquid credit);
   - apply the same validator-set / epoch accounting side effects as a successful `TxBody::Unstake { amount }` for that amount (stake admission at epoch boundary must observe the reduced stake).

No separate user `Unstake` transaction is required or permitted after activation on the victim account.

### Ordering (deterministic)

Within one `apply_tx` for emergency activation:

```text
validate_pol_action → apply_policy_action (finalized)
→ clear victim pending_conservation
→ evac balance_pwm → activation_target
→ evac staked_pwm_raw → activation_target.balance_pwm (V7)
→ fee debit, nonce++
```

### Wire / fee

- **No wire change:** same `ActivatePolicy { policy_id, activation_target }`, `fee = 0`.
- CLI prepared activation files from V6 remain valid; V7 nodes apply extended evacuation automatically.

### Non-Goals (V7)

- **`marks`** evacuation or mark-balance transfer (marks remain account-local; victim stops earning new marks when stake is zero).
- Cross-shard `activation_target` (unchanged defer from ADR 0011).
- Partial stake evacuation (all-or-nothing: entire `staked_pwm_raw`).

### V6 compatibility

- Nodes on V6 rules: balance-only evacuation (current behavior).
- V7 activation on mixed networks: follow chain **height-gated** or **genesis/feature** rollout defined in V7 sprint ticket (default: testnet-only until owner sign-off).

## Implementation plan (V7-3)

| Step | Owner | Notes |
|------|-------|-------|
| **Spec** | Done | This ADR + roadmap V7-3 + runbook oracle update |
| **Core** | `pwm-coding` | Extend evacuation block in `pwm-core` `State::apply_tx` (`Policy` arm); reuse unstake invariants from `TxBody::Unstake` |
| **Validator set** | `pwm-coding` | Ensure epoch snapshot / `active_validator_indices` reflect post-activation stake drop (same as organic unstake) |
| **Tests** | `pwm-testing` | Unit: `emergency_activation_sweep_includes_stake`; regression: V6 balance-only case unchanged when `staked=0`; CY e2e extend emergency wave |
| **CLI / wallet** | Optional | No new flags if automatic; update `pwm-cli.md` / operator copy: «activate after stake OK in V7» |
| **Runbook** | Docs | [v6-owner-stability-soak-50k.md](../runbooks/v6-owner-stability-soak-50k.md) §шаг 8: V7 oracle includes stake on rescue |

**Sprint anchor:** `V7-3` in [CONCEPT_ROADMAP.md](../CONCEPT_ROADMAP.md); coding ticket: `tasks/20260617-v7-emergency-stake-evacuation-impl.json` (backlog until V7 coding slice opens).

**Simplicity gate:** one code path in `apply_tx`; no new `PolicyAction` variant; no post-activation `Unstake` exception for finalized accounts.

## Consequences

- V6 operator runbooks must keep **«activate before stake»** until V7 runtime is deployed.
- Post-V7, emergency spot-check oracle: rescue liquid balance includes evacuated stake; victim `staked_pwm_raw == 0`.
- Audit / conservation: total PWM supply unchanged (stake → liquid move on same shard).

## References

- [ADR 0011: Policy activation target](0011-policy-activation-target.md)
- [ADR 0009: Address flags runtime](0009-address-flags-runtime-enforcement.md)
- [RFC 6: Policy engine](../rfc/6-policy-engine.md) §7.3.3
- [MVP v6 plan](../plans/mvp_v6.md) V6-7
- Operator soak: [v6-owner-stability-soak-50k.md](../runbooks/v6-owner-stability-soak-50k.md)
