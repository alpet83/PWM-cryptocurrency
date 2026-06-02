# V5-8 Slice2 Testing RCA: deferred smoke active_policies false negative

Date: 2026-05-25  
Ticket: `20260524-v5-s8-slice2-op-smoke-deferred-testing` (FAIL)

## Symptom

Live `-DeferredOnly` run exited **3**. Report: `tmp/devnet_v5_operator_smoke_20260525_142910.md`.

- `tx-policy-set deferred` OK
- Pre-height `tx-policy-activate` exit **2** (expected)
- Head reached **107** (target activate_at **20**)
- Harness threw: `expected active_policies>0 after height 20, got 0`

## Root cause

**Harness assertion bug**, not protocol regression.

Deferred policies are stored in `Account.deferred_policies` and become active in **`policy_is_active_at()` / `evaluate_policy()`** when `chain_tip_height >= activate_at_height`. They are **not** automatically OR'd into the persisted `active_policies` bitfield when height passes.

Code references:

- `set_pol_mode` pushes `DeferredPolicyEntry` without setting `active_policies` when `activate_at_height > inclusion_height` (`crates/pwm-core/src/state.rs`)
- `policy_is_active_at` treats deferred rows as active at height (`state.rs:618-623`)
- Unit test `policy_deferred_auto_at_h` validates **evaluator** behavior, not `active_policies` storage

The smoke harness incorrectly used `GET /v1/account.active_policies > 0` as post-height proof. API exposes only the stored bitfield, not evaluator-effective deferred state.

## Correct operator proof (ADR 0005)

| Phase | Expected signal |
|---|---|
| Before height | `active_policies == 0`; `tx-policy-activate` → **PolicyNotActive** (non-zero exit) |
| After height | `tx-policy-activate` → **PolicyDenied** / already active (non-zero exit); stored `active_policies` may stay 0 |

Optional future enhancement: expose `deferred_policies` on account JSON or add behavioral `tx-send` negative case with a second funded account.

## Fix

Orchestrator harness patch (`scripts/devnet_v5_operator_smoke.ps1`):

- Remove post-height `active_policies > 0` requirement
- Use post-height `tx-policy-activate` non-zero reject as PASS evidence
- Document stored bitfield may remain 0 in runbook

## Handoff

Rerun: `20260524-v5-s8-slice2-op-smoke-deferred-testing-rerun` after harness fix. No pwm-core change required for this false negative.
