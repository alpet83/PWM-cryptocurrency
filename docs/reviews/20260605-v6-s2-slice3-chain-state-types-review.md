# Review: V6-2 slice 3 — V6 chain state types on `State`

**Ticket:** `tasks/20260605-v6-s2-slice3-chain-state-types.json`  
**Worktree:** `P:/opt/docker/pwm-protocol-worktrees/v6-sprint2-core-model`  
**Reviewer:** pwm-review  
**Date:** 2026-06-05

---

## 1. Scope recap

Slice 3 of sprint **V6-2 (Core state model + snapshot v4)** adds wire-ready V6 shard/chain state types to `pwm_core::State` with serde defaults, without seal/apply enforcement. Claimed checklist items (from `docs/plans/mvp_v6.md` §V6-2 decomposition row 3 and ticket brief):

- `epoch_counter`, `active_validator_indices` — v6-rfc4 stake-gated active set context
- `CrossShardLock` + `CrossShardLockState` — v6-rfc9 Mode B escrow
- `EvidenceRecord` + `EvidenceType` — ADR 0010 slashing evidence stubs
- `PendingConservationTransfer` — ADR 0009 conservation delay queue
- Backward-compatible JSON decode of legacy `{accounts, fee_pool}` state blobs
- Compile fix in `pwmd` snapshot deserialize via `..ChainState::default()` spread

Out of scope (correctly deferred): snapshot v4 migration, `validate_tx_shape` reject stubs, seal/apply logic (slices 4–5 and V6-3…V6-8).

---

## 2. Requirements fit

**PASS — types and field shapes match frozen specs.**

| Spec | Requirement | Implementation |
|------|-------------|----------------|
| v6-rfc4 §3 | `epoch_counter: u64`, `active_validator_indices: Vec<u16>` on shard state | Present on `State` with `#[serde(default)]` |
| v6-rfc9 §3 | `CrossShardLock` fields + `Locked \| Released \| Refunded` | `CrossShardLock` / `CrossShardLockState` match wire text |
| ADR 0010 | `EvidenceRecord`, `EvidenceType` variants incl. `CustomStub(u16)` | Struct + enum align; append-only semantics not exercised (expected) |
| ADR 0009 | `PendingConservationTransfer` with height fields and `tx_hash` | All fields present; `fee_pwm: u64` per ADR |
| Ticket brief | No seal/apply behavior | No changes to `apply_tx`, seal tick, or mempool paths beyond mechanical `ActivatePolicy { .. }` pattern updates from slice 2 |

**Partial coverage (intentional for this slice):**

- Snapshot v3 wire (`SnapshotStateWire`, `serialize_snapshot_state`) still serializes only accounts/fee_pool/imported/exported — V6 vectors are **not** persisted on v3 snapshot round-trip. `deserialize_snapshot_state` / `state_from_wire` now spread `..ChainState::default()`, so loaded snapshots get zero/empty V6 fields. This matches the plan: slice 5 owns snapshot v4 + migration.
- `digest()` (bincode state root) will hash new fields when populated; with defaults the root remains compatible for empty V6 state. Full root semantics when fields are non-empty belong to later enforcement slices.

**Ancillary diff:** `pwmd/src/snapshot/types.rs` also wires `PolicyAction::ActivatePolicy { activation_target }` for v2/v3 snapshot policy encoding — slice 2 scope, required for compile after slice 2 enum change. Correct and consistent with ADR 0011; not a slice 3 functional gap.

---

## 3. Style and module shape

- **`python scripts/check_entity_name_segments.py`** on claimed paths: **zero violations** (prod ≤4, test ≤5).
- New production identifiers are within policy: `CrossShardLock`, `CrossShardLockState`, `EvidenceType`, `EvidenceRecord`, `PendingConservationTransfer`, field names ≤4 segments.
- Test names `st_v6_defaults_json`, `st_v6_json_roundtrip` — within test budget.
- Types live adjacent to existing `ExportProvenance` in `state.rs`; no new façade bloat.
- **Nit (low):** module banner `//! Canonical chain state: accounts map, fees, burns, import consumed IDs.` predates V6; could mention epoch/escrow/evidence/conservation in a follow-up doc pass (not blocking).

### Wire JSON / u128

Scope: chain-state JSON serde on `State` and nested V6 structs — **not** peer `PeerWireMsg` / catch-up wire in this slice. Normative RFC/ADR text already specifies decimal-string JSON for balance-scale `u128` (ADR 0009, v6-rfc9, v6-rfc4).

- `CrossShardLock.amount_pwm` — `#[serde(with = "crate::ser_json_u128")]` ✓
- `PendingConservationTransfer.amount_pwm` — same ✓
- `State.fee_pool: u128` — **pre-existing** derive-only on top-level `State` JSON (unchanged); tests use `fee_pool: 0`. Not introduced by this slice; snapshot v3 path already decimal-encodes fee_pool via `SnapshotStateWire`.
- V6 nested round-trip test exercises large `u128` values through JSON successfully.

**Wire JSON / u128:** applicable to local state JSON only; new balance fields use serde_json-safe encoding. No peer wire contract change in this slice.

---

## 4. Safety

- **No new consensus paths** — types only; no balance mutation, lock refund, or conservation drain.
- **No new panics** in production paths from this slice.
- **`ActivatePolicy { policy_id, .. }`** pattern updates preserve existing policy validation behavior; `activation_target` ignored where not yet enforced.
- **Trust boundaries unchanged** — snapshot deserialize contract errors for duplicate ids unchanged; V6 fields default safely on legacy input.

---

## 5. Tests

**Present:**

- `st_v6_defaults_json` — legacy minimal JSON decodes with empty/default V6 fields.
- `st_v6_json_roundtrip` — populated V6 vectors + large `u128` JSON round-trip; bincode serialize smoke (deserialize still blocked by pre-existing Account marks issue, documented in test comment).

**Missing (acceptable for slice 3; recommend in slice 5):**

- pwmd integration test that snapshot deserialize via `deserialize_snapshot_state` yields default V6 fields after v3 wire load.
- Explicit test that non-zero V6 state affects `digest()` when slice 5 lands (state root contract).

Tests run: `cargo test -p pwm-core st_v6_` — **2 passed**.

---

## 6. Verdict

**Approve with nits**

Prioritized nits (none block merge for slice 3):

1. **Low — doc:** extend `state.rs` module banner when convenient.
2. **Low — traceability:** PolicyAction snapshot wiring in same diff is slice 2 carry-over; ensure slice 2 review artifact exists or cross-ref in umbrella ticket.
3. **Informational — slice 5:** snapshot v3 path still drops V6 fields on save; slice 5 must extend wire + migration before relying on persisted V6 state.

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260605-v6-s2-slice3-chain-state-types-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 18500
  confidence: medium
```

---

**Verdict:** APPROVE_WITH_NITS — V6 chain state types match ADR 0009/0010 and v6-rfc4/v6-rfc9; serde defaults and tests adequate for wire-only slice; snapshot v4 persistence correctly deferred to slice 5.
