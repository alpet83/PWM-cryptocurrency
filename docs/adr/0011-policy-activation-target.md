# ADR 0011: Policy activation target and fee-free activation (V6)

## Status

**Accepted as V6 normative contract.** Extends V4 `ActivatePolicy` and emergency routing ([RFC 6](../rfc/6-policy-engine.md) §7.3.3). Coding in V6-2 (wire) and V6-7 (apply + CLI).

## Context

V4 emergency routing uses `rescue_address` in account/profile and `ActivatePolicy { policy_id }`. V6 requires explicit `activation_target` on the activation transaction for signature clarity, fee waiver policy, and balance evacuation to rescue as an ordinary same-shard value move in the same `apply_tx`.

Owner decision (V6): all `ActivatePolicy` transactions are **fee-free** in V6 because activation imposes future routing costs on moved funds.

## Decision

### Wire extension (additive)

```text
PolicyAction =
  SetPolicy { policy, activation }
  ActivatePolicy {
    policy_id: PolicyId,
    activation_target: Option<AccountId>,
  }
  DeactivatePolicy { policy_id }
```

Serialization: `activation_target` omitted or `null` when unused. For `routing.emergency_redirect` activation, field is **required**.

### Fee rule (V6)

- `PolicyTx` with `ActivatePolicy` MUST have `fee = 0` at validation.
- Non-zero fee → stable reject `E_POLICY_ACTIVATION_FEE_MUST_BE_ZERO`.
- `SetPolicy` / `DeactivatePolicy` keep V4 fee rules unchanged in V6.

### Emergency routing binding

When activating `routing.emergency_redirect`:

1. `activation_target` MUST be present.
2. `activation_target` MUST equal `Account.rescue_address` for the policy target account.
3. Mismatch → `E_POLICY_ACTIVATION_TARGET_MISMATCH`.
4. After owner signature and rescue cosign per RFC 6, same `apply_tx`:
   - activates emergency policy and sets `finalized` as today;
   - moves **entire spendable `balance_pwm`** to `activation_target` using same debit/credit semantics as ordinary same-shard transfer (nonce on target account unchanged unless transfer rules require otherwise; sender nonce increments per policy tx rules).
5. No separate user `TxBody::Transfer` is required for evacuation.

### Future uses (documented, not V6 runtime)

`activation_target` MAY later bind:

- whitelist/blacklist routing targets for corporate senders;
- delegated rescue variants.

V6 runtime enables **only** emergency binding above. Other policy kinds MUST ignore `activation_target` if present (reject if non-null and policy is not emergency) → `E_POLICY_ACTIVATION_TARGET_NOT_ALLOWED`.

### Anti-abuse bounds (V6)

- Evacuation move is one-shot per successful activation; repeated activation on finalized account follows existing V4 irreversibility rules.
- `activation_target` MUST NOT be cross-shard foreign account in V6 (same-shard only); cross-shard rescue defer → future ADR.
- Prepared activation in wallet ([RFC 10 addendum](../rfc/addenda/v6-rfc10-prepared-policy-activation.md)) MUST store the same `activation_target` and `fee=0` for operator audit.

### Interaction with address flags

- `COSIGN_NON_DISABLEABLE`: rescue cosign still required ([ADR 0009](0009-address-flags-runtime-enforcement.md)).
- `CONSERVATION`: pending outgoing transfers MUST be resolved before or as part of evacuation (deterministic cancel or execute ordering defined in ADR 0009).

## Stable rejects (additive)

- `E_POLICY_ACTIVATION_FEE_MUST_BE_ZERO`
- `E_POLICY_ACTIVATION_TARGET_MISMATCH`
- `E_POLICY_ACTIVATION_TARGET_REQUIRED`
- `E_POLICY_ACTIVATION_TARGET_NOT_ALLOWED`

## Non-Goals (V6)

- `staked_pwm_raw` / marks evacuation in V6 runtime. **V7:** stake evacuation on emergency activation — [ADR 0012](0012-emergency-stake-evacuation.md) (Accepted; impl backlog).
- Runtime whitelist/blacklist via `activation_target`.
- Cross-shard rescue activation target.

## Consequences

- `pwm-cli tx-init` with dormant emergency policy SHOULD auto-build signed `ActivatePolicy` (V6-7).
- Snapshot/policy tx serde tests in V6-2.
- TUI/CLI copy MUST show `activation_target` on prepared activations.

## References

- [ADR 0005: Deferred activation](0005-policy-deferred-activation.md)
- [ADR 0009: Address flags runtime](0009-address-flags-runtime-enforcement.md)
- [RFC 6: Policy engine](../rfc/6-policy-engine.md)
- [RFC addendum: V6 RFC6](../rfc/addenda/v6-rfc6-activate-policy-activation-target.md)
- [RFC addendum: V6 RFC10 wallet](../rfc/addenda/v6-rfc10-prepared-policy-activation.md)
- [ADR 0012: Emergency stake evacuation](0012-emergency-stake-evacuation.md) (V7)
- [MVP v6 plan](../plans/mvp_v6.md)
