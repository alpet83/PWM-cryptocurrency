# ADR 0009: Address flags runtime enforcement (V6)

## Status

**Accepted as V6 normative contract.** Implements runtime enforcement deferred from [ADR 0006](0006-address-flags-and-nondisableable-profiles.md). V6-6 and V6-8 coding slices MUST follow this ADR; V5 MUST NOT enforce these flags.

## Context

ADR 0006 defines `COSIGN_NON_DISABLEABLE` (bit 0) and `CONSERVATION` (bit 1) as predicates encoded in address bytes. V5 allows decode/display only. V6 must enforce them in consensus-critical paths without introducing an `Account.address_flags` field.

V6 also introduces conservation delay, emergency routing with `activation_target` ([ADR 0011](0011-policy-activation-target.md)), and Mode B cross-shard locks. Flag enforcement MUST compose with those mechanisms without hidden sweeps or policy bypass.

## Decision

### Flag source

- Flags are decoded from the sender (or subject) `AccountId` at validation and apply time.
- No migration writes flags into account state.

### `COSIGN_NON_DISABLEABLE` (bit 0)

When the relevant address has bit 0 set:

1. **Baseline cosign:** `cosign_required` is treated as always-on for protected actions on that account (per RFC 6 protected-action taxonomy).
2. **PolicyTx hardening:** `DeactivatePolicy` or `SetPolicy` that would weaken/remove mandatory cosign MUST be rejected with stable `E_POLICY_FLAG_NON_DISABLEABLE`.
3. **Emergency routing:** activation and rescue flows remain allowed when RFC 6 + ADR 0011 gates pass (owner + rescue cosign, `activation_target == rescue_address`).
4. **Mempool:** transactions that would fail (2) MUST be rejected at mempool admission with the same stable code as seal apply.

### `CONSERVATION` (bit 1)

When the sender address has bit 1 set:

1. **Outgoing `Transfer`:** not applied immediately. It enters `PendingConservationTransfer` (shard state, snapshot v4).
2. **Delay source:** chain height only. Parameter `conservation_delay_blocks: u64` in `GenCfg` (default `86400` for ~24h at 1s block time in devnet profiles; operators MAY override in genesis).
3. **Execution:** at `inclusion_height + conservation_delay_blocks`, seal tick applies the pending transfer if still valid (balance, nonce, policies).
4. **Cancellation / redirect:** during the pending window, emergency routing activation ([ADR 0011](0011-policy-activation-target.md)) or other accepted policy paths MAY redirect value before final execution; conservation queue entries MUST be removed or superseded deterministically when emergency evac completes on the same account.
5. **Incoming transfers:** NOT delayed by this flag in V6.
6. **Mempool:** immediate outgoing `Transfer` from conservation addresses MUST NOT be admitted as immediately executable; either reject with `E_CONSERVATION_DELAY_REQUIRED` or accept as pending-only per implementation profile — consensus outcome MUST match seal apply.

### Interaction matrix (normative)

| Action | COSIGN_NON_DISABLEABLE | CONSERVATION |
|--------|------------------------|--------------|
| Ordinary `Transfer` | cosign gate enforced | pending queue |
| `PolicyTx` weaken cosign | reject | allowed if other gates pass |
| `ActivatePolicy` emergency | cosign + ADR 0011 | allowed; may cancel pending conservation |
| Cross-shard `EXPORT` | cosign if export is protected action | V6: no special delay unless later ADR |

### Validator paths

Consensus-critical enforcement MUST occur in:

- `validate_tx_shape` (or equivalent pre-apply checks)
- `state::apply_tx`
- seal-time pending conservation drain
- mempool admission (same stable errors)

Non-consensus tooling (CLI/TUI) MAY preview flag effects but MUST NOT be the sole enforcement layer.

## Wire and state (frozen for V6)

```text
GenCfg.conservation_delay_blocks: u64  // default 86400

PendingConservationTransfer {
  sender: AccountId,
  recipient: AccountId,
  amount_pwm: u128,  // JSON: decimal string per RFC 12 / RFC 7
  fee_pwm: u64,
  nonce: u64,
  enqueue_height: u64,
  execute_at_height: u64,  // enqueue_height + conservation_delay_blocks
  tx_hash: Hash32,         // original transfer identity for idempotency
}
```

Stable rejects (additive):

- `E_POLICY_FLAG_NON_DISABLEABLE`
- `E_CONSERVATION_DELAY_REQUIRED`
- `E_CONSERVATION_PENDING_EXISTS` (optional: one pending outgoing per account in V6)

## Non-Goals (V6)

- Wall-clock conservation timers.
- Incoming transfer delay.
- Mutable flag upgrade after address creation.
- Full multi-rescue orchestration UI.

## Consequences

- Snapshot v4 MUST persist pending conservation queue (see mvp_v6 V6-2).
- Wallet/CLI SHOULD display pending conservation and flag decode before enforcement lands in V6-6/V6-8.
- Tests in V6-6/V6-8 MUST cover cosign non-disableable + conservation + emergency interaction.

## References

- [ADR 0006: Address flags (spec)](0006-address-flags-and-nondisableable-profiles.md)
- [ADR 0011: Policy activation target](0011-policy-activation-target.md)
- [RFC 1: Address format](../rfc/1-address-format.md)
- [RFC 6: Policy engine](../rfc/6-policy-engine.md)
- [RFC addendum: V6 RFC6 activation](../rfc/addenda/v6-rfc6-activate-policy-activation-target.md)
- [MVP v6 plan](../plans/mvp_v6.md)
