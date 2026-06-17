# RFC 6 Addendum (V6): ActivatePolicy extension and emergency evacuation

**Parent:** [6-policy-engine.md](../6-policy-engine.md) §7.3.3  
**Status:** Normative for MVP v6  
**Normative ADR:** [ADR 0011](../../adr/0011-policy-activation-target.md)

## 1. Wire change

Replace V4/V5 activation arm with:

```text
ActivatePolicy {
  policy_id: PolicyId,
  activation_target: Option<AccountId>,
}
```

`SetPolicy` / `DeactivatePolicy` unchanged in V6.

## 2. Fee-free activation (V6)

All `PolicyTx` carrying `ActivatePolicy` MUST have `fee = 0`. See ADR 0011 for stable reject code.

## 3. Emergency routing (`routing.emergency_redirect`)

Normative apply sequence when activating dormant/immediate emergency policy:

1. Validate `activation_target == Account.rescue_address`.
2. Validate owner + rescue cosignatures (RFC 6 §7.3.3).
3. In one `apply_tx`:
   - set policy active + `finalized = true`;
   - transfer full spendable `balance_pwm` to `activation_target` (same-shard transfer semantics);
   - increment sender nonce per policy tx rules.

Incoming transfers after finalization follow existing redirect semantics (RFC 6).

## 4. Non-emergency policies

If `activation_target` is set for policies other than `routing.emergency_redirect`, reject `E_POLICY_ACTIVATION_TARGET_NOT_ALLOWED` in V6.

## 5. Deferred activation interaction

`Deferred { activate_at_height }` from ADR 0005 is unchanged. `ActivatePolicy` for manually dormant policies still applies; `activation_target` rules apply at activation time, not at `SetPolicy` time.

## 6. CLI reference (V6-7)

`pwm-cli tx-policy-activate` MUST accept `--activation-target` for emergency flows. `tx-init` corporate path SHOULD pre-build signed activation per [RFC 10 addendum](v6-rfc10-prepared-policy-activation.md).
