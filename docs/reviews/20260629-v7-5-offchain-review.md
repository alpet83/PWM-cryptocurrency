# Review: V7-5 offchain batch API Merkle root + proof (5f96ec8)

- date: 2026-06-29
- ticket: `20260629-v7-5-offchain-review`
- coding_ticket: `20260629-v7-5-offchain-batch`
- commit: `5f96ec8` (branch `main` at review time)
- scope: `crates/pwmd/src/offchain.rs`, `api/handlers_offchain.rs`, `api/types.rs`, `api/router.rs`, `state.rs`, `bootstrap.rs`, `docs/OFFCHAIN_STUB.md`
- norm: additive HTTP API; no wire / `pwm-core` state changes

## 1. Scope recap

V7-5 delivers process-local offchain batch storage with SHA-256 Merkle root and inclusion proofs:

| endpoint | behavior |
|----------|----------|
| `POST /v1/offchain/batch` | Accept `{account_id, amount, nonce}[]` → `batch_id`, `merkle_root`, `entry_count`, `anchor_tx_hash` |
| `GET /v1/offchain/batch/:id` | Stored metadata |
| `GET /v1/offchain/batch/:id/proof/:entry_index` | Leaf + sibling proof + self-check |

`pwmd` `0.1.79` → `0.1.80`. `anchor_tx_hash` is a **deterministic surrogate** (not consensus-visible tx).

## 2. Requirements fit

| Focus area | Verdict | Evidence |
|------------|---------|----------|
| 1. Merkle root/proof correctness | **PASS** | Tagged SHA-256 leaves/nodes (`offchain.rs:10-12`, `:98-120`, `:177-183`); odd leaf duplicate (`:115-116`, `:140-141`); `verify_proof` inverts `merkle_proof` sibling order (`:149-164`); handler self-check (`handlers_offchain.rs:56-60`) |
| 2. POST batch | **PASS** | Parse + empty reject (`handlers_offchain.rs:14-21`); tip snapshot under read lock (`:22-25`); `insert` returns `batch_out` (`:26-27`) |
| 3. GET proof arbitrary index | **PASS** with nit | `merkle_proof` walks levels with sibling pick + duplicate rule (`offchain.rs:123-146`); tested for index 1 in 3-leaf tree — not all indices |
| 4. `anchor_tx_hash` surrogate | **PASS** (staged PARTIAL) | `anchor_hash(batch_id, root, tip_hash, tip_height)` (`:167-174`); documented in `OFFCHAIN_STUB.md` §Anchor status. **Sufficient for V7-5 Merkle API slice**; **insufficient alone** for full `mvp_v7.md` demo #4 (on-chain anchor) |
| 5. `OffchainStore` contention | **PASS** | Separate `Arc<OffchainStore>` (`state.rs:84`, `bootstrap.rs:184`); `Mutex<HashMap>` — not on seal hot path; process-local MVP acceptable per stub doc |
| 6. Unit tests `merkle_*` | **PASS** with nit | `merkle_root_single_entry`, `merkle_root_two_entries`, `merkle_proof_verify` (`offchain.rs:203-230`); no HTTP handler tests |
| 7. Pre-existing PARTIAL flaws | **PASS** (not introduced) | New modules only; no fmt/event-test churn |

## 3. Merkle implementation

### Leaf / node preimages (matches `OFFCHAIN_STUB.md`)

```98:120:crates/pwmd/src/offchain.rs
pub(crate) fn entry_leaf(entry: &BatchEntry) -> [u8; 32] {
    // PWMv1/OFFLEAF || account_id || amount_be_u128 || nonce_be_u64
}
pub(crate) fn merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    // PWMv1/OFFNODE pairs; duplicate last on odd levels
}
```

Single-leaf tree: root equals leaf (while-loop not entered) — consistent with proof `[]` verifying to root.

### Proof path

- Sibling position `"left"` / `"right"` matches `verify_proof` hash order.
- When sibling index missing (odd tail), uses `level[idx]` — same duplicate rule as root builder.

### Anchor surrogate

```167:174:crates/pwmd/src/offchain.rs
fn anchor_hash(batch_id: u64, root: [u8; 32], tip_hash: [u8; 32], tip_height: u64) -> [u8; 32] {
    // PWMv1/OFFANCHOR || batch_id_be || root || tip_hash || tip_height_be
}
```

**Impact on V7-5 done criterion:**

| criterion | status at `5f96ec8` |
|-----------|---------------------|
| Client Merkle submit + proof verify | **Done** |
| Cross-node / restart durability | **Not done** (process-local `HashMap`) |
| Consensus-visible anchor | **Deferred** (documented PARTIAL) |

Acceptable **approve_with_nits** for coding ticket scoped to API + Merkle; orchestrator should keep follow-up ticket for on-chain anchor before marking `mvp_v7` offchain demo #4 complete.

## 4. HTTP handlers

- `DefaultBodyLimit::max(V1_TX_BODY_LIMIT)` on router (`router.rs:74`) bounds POST payload.
- `v1_off_proof` returns 500 if internal verify fails — catches implementation drift.
- **Nit:** handlers omit `ensure_ready` (unlike `/v1/account`); POST can read tip during init — low risk for lab offchain path.

## 5. Style and module shape

- New `offchain.rs` module with `//!` banner — appropriate extraction from monolith.
- DTOs in `types.rs`; handlers thin.
- Distinct from legacy `pwm-core/offchain.rs` (BLAKE3 v0 stub) — documented in `OFFCHAIN_STUB.md` §Legacy.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice). HTTP `amount` uses decimal string parse — consistent with account API.

## 6. Safety

- No `unsafe`; parse errors → `400 BAD_REQUEST`.
- Process-local store: batches lost on restart — documented; not a consensus trust boundary yet.
- **DoS nit:** unbounded batch entry count within body limit — acceptable MVP; production may want per-batch cap.

## 7. Tests

| test | coverage |
|------|----------|
| `merkle_root_single_entry` | 1-leaf root |
| `merkle_root_two_entries` | 2-leaf root |
| `merkle_proof_verify` | 3-leaf proof index 1 |

**Gaps (nit):** proof for index 0/2 in odd tree; HTTP round-trip; anchor determinism test.

## 8. Concurrency / parallelism

Components: HTTP task → `OffchainStore::insert/get` (`Mutex`); tip read via `inner.read().await`.

- Off-chain path isolated from seal loop and worker queues — no hot-path lock contention.
- `BatchRecord` clone on `get` — fine for MVP batch sizes.
- `AtomicU64` batch id — no race on id assignment.

## 9. BLOCKERs

None. Merkle math and proof verification are internally consistent; no cross-account leakage (offchain entries are opaque client-supplied rows).

## 10. Nits (non-blocking)

1. **NIT-1:** Document PARTIAL explicitly in ticket closeout — on-chain anchor required for full V7 offchain demo gate (`mvp_v7.md` item 4).
2. **NIT-2:** Add `ensure_ready` to offchain handlers for consistency with `/v1/*` readiness contract.
3. **NIT-3:** Extend `merkle_proof_verify` to indices 0 and 2 (odd-length tree).
4. **NIT-4:** HTTP integration test for POST → GET → proof round-trip.

## 11. Verdict

**Approve with nits** — SHA-256 Merkle root/proof implementation and three endpoints are correct for MVP client verification; `anchor_tx_hash` surrogate is honestly documented and acceptable for **V7-5 staged delivery**, with explicit follow-up for consensus-visible anchoring and persistence.

## 12. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260629-v7-5-offchain-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 38000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260629-v7-5-offchain-review.md'
git commit -m 'docs(v7-5): offchain batch Merkle API review (5f96ec8)'
```