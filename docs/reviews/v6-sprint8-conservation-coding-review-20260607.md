# Review: V6-8 CONSERVATION delayed Transfer

**Ticket:** `tasks/20260607-v6-sprint8-conservation-coding.json`  
**Branch:** `v6/20260607-v6-sprint8-conservation-coding`  
**Coding commit:** `b9e0e1c` (`feat(pwm-core): CONSERVATION delayed Transfer queue + seal drain`)  
**Reviewer:** pwm-review  
**Date:** 2026-06-07

## 1. Scope recap

Slice **V6-8** implements ADR 0009 bit 1 (`CONSERVATION`) runtime for outgoing `Transfer`:

- `conservation_flag()` helper and `CONSERVATION` constant in `types.rs`
- Outgoing transfer from conservation sender → `PendingConservationTransfer` queue (height-based delay via `GenCfg.conservation_delay_blocks`)
- Seal-time drain via `Chain::seal` → `State::drain_conservation_at_height`
- Emergency activation (ADR 0011 / V6-7) cancels pending on sender via `retain`
- Unit tests `conservation_*` + `conservation_seal_drains` in `chain.rs`

Normative anchors: `docs/adr/0009-address-flags-runtime-enforcement.md`, `docs/adr/0011-policy-activation-target.md`, `docs/plans/mvp_v6.md` Sprint V6-8, ticket acceptance criteria.

## 2. Requirements fit

| Criterion | Status | Evidence |
|-----------|--------|----------|
| `conservation_flag()` alongside `cosign_non_dis` | **Met** | `types.rs`: `CONSERVATION = 1 << 1`, `conservation_flag()` |
| Outgoing conservation transfer enqueued, not applied immediately | **Met** | `state.rs` Transfer branch: push to `pending_conservation`, early `Ok(())`; balance/nonce unchanged until drain |
| `enqueue_height` / `execute_at_height = enqueue + delay` | **Met** | Set at apply; `conservation_delay_execute` asserts heights 10→12 with delay 2 |
| Before execute: balances unchanged | **Met** | Tests assert sender balance 1000, nonce 0 while pending |
| After execute: ordinary transfer semantics | **Met** | `apply_due_conservation` debits/credits, increments nonce, fee to pool; re-evaluates routing via `conservation_recipient_dst` |
| Height-only delay (no wall-clock) | **Met** | Only `conservation_delay_blocks` + block height used |
| One pending per account | **Met** | `ConservationPendingExists` when second outgoing from same sender; tested |
| Incoming transfers NOT delayed | **Met** | `conservation_incoming_not_delayed`: plain sender → conservation recipient applies immediately, queue empty |
| Emergency cancels pending deterministically | **Met** | `pending_conservation.retain(|row| row.sender != id)` on emergency evac; `conservation_emergency_cancels_pending` |
| Mempool/precheck parity with seal apply | **Met** | `precheck_apply_with_ctx` clones state and calls same `apply_tx_with_ctx`; pwmd admission uses this path (`handlers_tx.rs`, `peer_session/mod.rs`) |
| Seal hook drains queue | **Met** | `chain.rs` `seal`: after tx apply, `st.drain_conservation_at_height(height, &self.cfg)`; all pwmd paths use `Chain::seal` |
| `tx_hash` idempotency field | **Partial** | Stored as blake3 of signing message per ADR wire shape; not used for runtime dedup (acceptable for V6 snapshot identity) |
| `E_CONSERVATION_DELAY_REQUIRED` path | **N/A (profile)** | Implementation uses ADR-permitted **pending-only** profile (accept + enqueue); error enum/wire mapping exist but unused in apply |

**ADR 0009 interaction matrix (conservation column):**

- Ordinary `Transfer` from conservation sender → pending queue ✓
- `ActivatePolicy` emergency → cancels pending, evacuates balance ✓
- Incoming transfer to conservation address → immediate apply ✓
- Cross-shard EXPORT → no special delay in this slice (per ADR non-goal for V6) ✓

**Seal ordering:** txs applied at height `H`, then drain at same `H`. Enqueued row has `execute_at_height = H + delay`, so execution correctly occurs on a later seal — verified by `conservation_seal_drains` (delay=1: enqueue at h1, drain at h2).

## 3. Style and module shape

- Production identifiers within policy: `check_entity_name_segments.py` on `types.rs`, `state.rs`, `chain.rs` → **zero violations** (`prod_max: 4`).
- New helpers (`conservation_flag`, `drain_conservation_at_height`, `apply_due_conservation`, `conservation_recipient_dst`) follow existing naming and live in appropriate modules (`types`, `state`, `chain` test helpers).
- `PendingConservationTransfer.amount_pwm` uses existing `#[serde(with = "crate::ser_json_u128")]` — consistent with V6-2 snapshot model.
- Logic integrated into existing `apply_tx_with_ctx` Transfer arm rather than parallel sweep type — matches ticket brief.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice). Changes are ledger state + snapshot JSON for `PendingConservationTransfer`; `amount_pwm` already uses `ser_json_u128` from prior V6-2 work. No new `PeerWireMsg` / handshake fields.

## 4. Safety

- **No panics in production paths** beyond existing `expect("recipient gated")` pattern (same as ordinary Transfer).
- **Failed drain is silent:** `drain_conservation_at_height` uses `let _ = apply_due_conservation(...)` — if execution fails (insufficient balance, bad nonce, policy reject), pending row is dropped without re-queue. Consistent with ADR “applies if still valid”; funds stay with sender. Low observability nit, not a consensus divergence risk.
- **Enqueue balance check without debit:** sender must have funds at enqueue time; funds remain spendable via non-transfer paths (e.g. emergency evac, or other tx consuming nonce) until execute — invalid pending then drops at drain. Acceptable per ADR validity-at-execution model; operators should treat pending as soft reservation only.
- **Emergency retain** runs before evac credit — prevents double-move of queued amount. Tested.
- **Resource bounds:** one pending per account caps queue growth per sender; global `Vec` unbounded in theory but V6 one-per-account limit mitigates abuse.

## 5. Tests

**Present (pwm-core):**

| Test | Coverage |
|------|----------|
| `conservation_delay_execute` | Enqueue, delay, drain execute, balances/fee_pool |
| `conservation_pending_exists_reject` | Second outgoing + precheck parity |
| `conservation_incoming_not_delayed` | Recipient conservation flag, immediate apply |
| `conservation_emergency_cancels_pending` | Emergency activation clears queue; post-drain recipient unchanged |
| `conservation_seal_drains` | End-to-end `Chain::seal` integration |

**Gaps (non-blocking nits):**

- No explicit test that first conservation transfer **accepts** via `precheck_apply_tip` (only duplicate reject via precheck).
- No test for failed drain (e.g. insufficient balance at execute height) confirming silent drop.
- `ConservationDelayRequired` reject profile not exercised (not required given pending-only profile).

**Verification note:** Reviewer could not re-run `cargo test` locally (Windows `dlltool.exe` toolchain error). Worker reported PASS on `conservation_*` and `check --workspace`; code inspection supports that claim.

## 6. Verdict

**Approve with nits.**

Implementation satisfies V6-8 acceptance criteria and ADR 0009 conservation matrix. Pending-only mempool/seal profile is normative. Nits are documentation/test follow-ups, not merge blockers:

1. **Low:** Document or test silent discard when `apply_due_conservation` fails at drain height.
2. **Low:** Optional `precheck_apply_tip` happy-path test for first conservation enqueue.
3. **Info:** `ConservationDelayRequired` remains unused under pending-only profile — fine; wire mapping ready if profile switches.

## 7. Participation / token estimate

```json
{
  "agent": "pwm-review",
  "result": "PASS",
  "artifacts": "docs/reviews/v6-sprint8-conservation-coding-review-20260607.md",
  "token_usage": {
    "source": "estimate",
    "input": null,
    "output": null,
    "total": 45000,
    "confidence": "low"
  }
}
```

**Orchestrator verdict line:** `PASS_WITH_NITS`
