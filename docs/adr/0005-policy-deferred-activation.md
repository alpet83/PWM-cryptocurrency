# ADR 0005: Deferred policy activation by chain height

## Status

**Accepted.** This ADR is the normative V5 contract for `ActivationMode::Deferred { activate_at_height }`. It covers policy activation scheduling only. Address flags, non-disableable profiles, and delayed transfer/conservation semantics are specified separately and are not part of this ADR.

## Context

MVP V4 introduced dedicated `PolicyTx`, per-account policy state, `Dormant` and `Immediately` activation modes, emergency routing, and structured `E_POLICY_*` rejects.

V5 needs a minimal deterministic third activation mode so operators can schedule policy activation by chain height before broader address-flag work lands. The important boundary is simplicity: no VM, no callbacks, no wall-clock dependency, no delayed execution queue for ordinary transfers.

## Decision

Extend the normative policy model:

```text
ActivationMode =
  Dormant
  Immediately
  Deferred { activate_at_height: u64 }

PolicyAction =
  SetPolicy { policy, activation }
  ActivatePolicy { policy_id }
  DeactivatePolicy { policy_id }
```

`SetPolicy { activation = Deferred { activate_at_height } }` installs the policy and records its activation height. The policy is evaluator-active when:

```text
current_chain_height >= activate_at_height
```

No separate `ActivatePolicy` transaction is required after the height is reached.

## Normative Rules

- The only time source is chain height. Wall-clock timestamps, local node time, and time zones MUST NOT affect activation.
- `activate_at_height` is an absolute chain height, not a relative delay.
- If `activate_at_height <= inclusion_height`, the policy is active immediately after the `SetPolicy` transaction is applied.
- If `activate_at_height > inclusion_height`, the policy is stored as pending and becomes active automatically in evaluator reads at or after that height.
- `ActivatePolicy` before `activate_at_height` MUST be rejected with `E_POLICY_NOT_ACTIVE`.
- `ActivatePolicy` at or after `activate_at_height` MUST be rejected with `E_POLICY_DENIED` and an "already active" message, because deferred activation is automatic by height and no extra state transition is required.
- `DeactivatePolicy` before `activate_at_height` is allowed only for reversible policies and removes the pending deferred activation.
- Irreversible/system policies keep their existing irreversibility rules.
- `evaluate_policy` remains pure: it receives read-only state plus current chain height and returns a decision without state mutation.

## Genesis Height Convention

For `initial_policies[]` in genesis or extended `INIT`:

- `activate_at_height` is interpreted against the same chain height numbering exposed by `head.height`.
- A deferred policy with `activate_at_height = 0` is active from genesis.
- A deferred policy with `activate_at_height = N` is inactive for evaluator calls where `current_chain_height < N` and active where `current_chain_height >= N`.

Implementation tickets MUST include replay tests around the genesis/head convention so snapshot reloads do not shift the activation boundary.

## State and Snapshot

V5 implementation needs a durable representation of pending deferred policies, typically:

```text
DeferredPolicyEntry {
  policy_id,
  policy,
  activate_at_height: u64
}
```

The exact Rust type is implementation-owned, but snapshot schema v3 MUST preserve enough information to replay deferred activation deterministically.

## Explicit Non-Goals

- No address-flag enforcement.
- No non-disableable policy profiles.
- No delayed execution of ordinary `Transfer`.
- No mempool/seal holding queue.
- No wall-clock activation.
- No scripts, DSL, plugins, dynamic dispatch, or external callbacks.

## Consequences

- RFC 0006 and RFC 0007 must be updated to make `Deferred { activate_at_height }` evaluator-normative for V5.
- CLI may expose `--activation deferred --activate-at-height <N>` once backend support exists.
- Operators are responsible for converting calendar intent into chain height off-chain.

## References

- [RFC 0006: Policy Engine](../rfc/6-policy-engine.md)
- [RFC 0007: Transaction & State Model](../rfc/7-tx-and-state-model.md)
- [ADR 0006: Address flags and non-disableable profiles](0006-address-flags-and-nondisableable-profiles.md)
- [MVP v5 plan](../plans/mvp_v5.md)

## History

| Date | Event |
|---|---|
| 2026-05-17 | Draft ADR 0005 created as a minimal deferred-only path. |
| 2026-05-23 | Accepted for V5-1 spec freeze; scope narrowed to chain-height policy activation only. |
