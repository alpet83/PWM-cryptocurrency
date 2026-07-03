# Rust Code Audit — MVP V7 window

**Date:** 2026-06-29  
**Ticket:** `20260629-v7-rust-code-audit`  
**Audit window:** V7 slices on `main` (V7-1 perf pipeline, V7-3 TUI/AcctOut conservation, V7-4 stake evac, V7-5 offchain batch)  
**Reference format:** `docs/reviews/20260528-v5-mvp-rust-code-audit-review.md`  
**Audited paths (priority):** `crates/pwmd/src/api/handlers_account.rs`, `api/types.rs`, `offchain.rs`, `handlers_offchain.rs`, `pipeline/*`, `lifecycle.rs` (seal `st_before` gate), `crates/pwm-core/src/state.rs` (ActivatePolicy evac + conservation), `crates/pwm-tui` (conservation display)  
**Categories:** security, correctness, simplicity, concurrency spot-check, wire JSON / u128  
**Tooling:** direct file reads, grep; `git show` / `cargo test` / `check_entity_name_segments.py` unavailable in review sandbox

---

## Executive summary

| Severity | Count |
|----------|-------|
| BLOCKER | 0 |
| MAJOR | 1 |
| MINOR | 4 |
| NIT | 5 |
| **Total** | **10** |

**Verdict:** `PASS_WITH_FINDINGS`

No merge-blocking memory-safety or cross-account leakage defects found in the ticket focus paths. V7 security-sensitive surfaces behave as designed:

- `pending_conservation` API exposure is sender-scoped with an HTTP regression test.
- Emergency stake evacuation is single-transaction atomic with idempotency guards and unit tests.
- Merkle offchain uses domain-separated SHA-256 with round-trip proof tests.

The one **MAJOR** finding is operational, not a logic bug: `/v1/offchain/batch` is unauthenticated and stores batches only in process-local memory — acceptable for the documented V7-5 MVP stub, but operators must treat it as non-production trust boundary until anchoring/auth land.

---

## Scope recap

| V7 slice | Primary Rust surfaces |
|----------|----------------------|
| V7-1 perf | `crates/pwmd/src/pipeline/` (`worker.rs`, `queue.rs`, `hot_index.rs`, `dispatch.rs`), `lifecycle.rs` seal path, `handlers_tx.rs` `Arc<SignedTx>` |
| V7-3 TUI / AcctOut | `handlers_account.rs`, `api/types.rs` `AcctOut.pending_conservation`, `pwm-tui` conservation display |
| V7-4 stake evac | `pwm-core/src/state.rs` `apply_tx_with_ctx` Policy arm + `apply_policy_action` |
| V7-5 offchain | `pwmd/src/offchain.rs`, `api/handlers_offchain.rs` |
| V7-2 bruteforce | `pwm-cli` — shallow scan only (per ticket) |

---

## Focus-area verification

| # | Focus | Result | Evidence |
|---|-------|--------|----------|
| 1 | `pending_conservation` sender guard | **PASS** | Only enrichment path: `pending_conservation_out` filters `row.sender == key` (`handlers_account.rs:16-28`); `acct_out_for_runtime` initializes empty (`common.rs:479`); test `http_status.rs:320-396` asserts other account omits field |
| 2 | Stake evac atomicity / no double-evac | **PASS** | Evac in same `apply_tx_with_ctx` arm after `apply_policy_action` (`state.rs:709-728`); `saturating_add` credits; re-activation blocked via `finalized_policy_allowed` + `is_finalized_blocked` (`973-984`, `946-957`); tests `emergency_activation_sweep_ok`, `emergency_evac_epoch_index` |
| 3 | Merkle SHA-256 + proof soundness | **PASS** with MINOR | Domain tags `PWMv1/OFFLEAF` / `OFFNODE` / `OFFANCHOR` (`offchain.rs:10-13`); `node_hash` hashes tag+left+right; odd-leaf duplication consistent in `merkle_root` and `merkle_proof`; unit tests `merkle_proof_verify` |
| 4 | `skip_serializing_if` empty vec | **PASS** | `#[serde(default, skip_serializing_if = "Vec::is_empty")]` on `AcctOut.pending_conservation` (`types.rs:523-524`); empty → field omitted (not `null`); test confirms `other_json.get("pending_conservation").is_none()` |
| 5 | `fee_pwm` sourcing | **PASS** | API maps `row.fee_pwm.to_string()` from `PendingConservationTransfer.fee_pwm: u64` (`handlers_account.rs:23`, `state.rs:131`); enqueue uses `u64::try_from(*fee)` rejecting oversized fees (`state.rs:474`) |
| 6 | V7-1 `Arc<SignedTx>` / `st_before` gate | **PASS** | `ClientTxJob { tx: Arc<SignedTx> }` (`queue.rs:15-17`); worker uses `job.tx.as_ref()` (`worker.rs:318`); `st_before` only when `tracing::enabled!(DEBUG)` (`lifecycle.rs:1907-1970`) — no use-after-move |
| 7 | Simplicity | **PASS** with nits | No new `unsafe` in V7 hot paths; pipeline split across focused modules; production `expect` limited to mutex poison / offchain store lock |
| 8 | Report deliverable | **PASS** | This file |

---

## Findings by crate

### `crates/pwm-core`

#### MINOR-001: Conservation `fee_pwm` capped at `u64` while transfer fee is `u128`

**Location:** `state.rs:474-479`

Enqueue rejects `fee > u64::MAX` via `u64::try_from(*fee)`. Consistent internally, but large-fee conservation transfers are impossible without schema change. Document or align types if large fees are ever required.

#### MINOR-002: Emergency evac uses `expect` on rescue account after validation

**Location:** `state.rs:716-719`

`validate_pol_action` already requires initialized rescue account before `apply_policy_action`. Panic path is unreachable on valid txs; prefer `ok_or(TxError::...)` for defense-in-depth (nit-level panic hygiene).

**Stake evac correctness (no finding):** Liquid + staked credited in one commit; `pending_conservation` cleared for sender (`712`); validator set recomputed on epoch roll (test `emergency_evac_epoch_index`).

---

### `crates/pwmd`

#### MAJOR-001: Offchain batch API is unauthenticated and ephemeral

**Location:** `handlers_offchain.rs:10-27`, `offchain.rs:38-41`

`POST /v1/offchain/batch` accepts arbitrary entry vectors with no auth; `OffchainStore` is in-memory `Mutex<HashMap>` — batches vanish on restart and are not tied to on-chain anchor execution in this slice.

**Impact:** Any RPC client can flood batch IDs; proofs attest only to operator-local state, not consensus truth.

**Mitigation (documented MVP):** Matches V7-5 stub intent and ADR 0003 centralized-batch baseline. **Before production:** persist batches, require anchor tx confirmation, add operator auth.

#### MINOR-003: `verify_proof` treats unknown `position` as `"right"`

**Location:** `offchain.rs:158-162`

Malformed proof steps with garbage `position` fall through to right-sibling branch instead of rejecting. Exploit requires attacker-controlled proof input on verify endpoint; server-generated proofs are safe. Recommend explicit position enum match returning `false`.

#### MINOR-004: Full precheck still clones state for non-hot txs

**Location:** `worker.rs:398-416`, `state.rs:222-231`

`precheck_full` → `precheck_apply_with_ctx` clones entire `State`. Hot-index fast path avoids this for flag-clean transfers (`worker.rs:354-367`). Known V7-1 tradeoff; not a correctness bug.

**`pending_conservation` API (no finding):** Sender filter is the sole population path; list endpoint applies same helper per account id.

**V7-1 perf (no finding):** `Arc<SignedTx>` shared across dispatch/worker/reply; `ValidatedTx` clones at boundary (`worker.rs:419-423`) — intentional copy once validated.

---

### `crates/pwm-tui`

#### NIT-001: TUI omits `fee_pwm` from `PendingConservationRow`

**Location:** `models.rs:12-18`, `account_view.rs:276-293`

API returns `fee_pwm`; TUI parses amount/nonce/heights only. Display line shows count + next execute height (`tui_loop.rs:930-939`) — acceptable for V7-3, but fee invisible to operator.

#### NIT-002: TUI trusts server-side `pending_conservation` filter

**Location:** `account_view.rs:276-294`

No client-side re-filter by wallet key. Correct as long as API guard remains (focus area 1). Add defensive filter if multi-wallet views are introduced.

---

### `crates/pwm-cli` (V7-2 shallow scan)

#### NIT-003: `addr-bruteforce` auto tx-init over HTTP

**Location:** `cmd_addr.rs` (grep)

Uses live RPC for auto init when not offline — standard CLI pattern. No deterministic fallback signing keys observed in bruteforce path (contrast V5 `claim_ipv4_batch` HIGH finding). No deep audit performed per ticket.

---

## Style and module shape

Docs/config excluded. V7 pipeline modules are decomposed (`worker`, `queue`, `dispatch`, `hot_index`, `counters`) rather than monolithic `main.rs` growth.

Entity-name segment check (`scripts/check_entity_name_segments.py`) not executed — sandbox terminal failure. Manual spot-check: V7 helpers (`pending_conservation_out`, `precheck_hot`, `conservation_pending_txt`) are ≤4 segments.

### Wire JSON / u128

**Scope:** `PendingConservationTransfer.amount_pwm` uses `#[serde(with = "crate::ser_json_u128")]` (`state.rs:129`). `PendingConservationOut.amount_pwm` / `fee_pwm` serialize as decimal strings in API (`handlers_account.rs:22-23`). Offchain batch JSON uses string amounts in `OffchainEntryIn` (`types.rs:541-544`). No derive-only `u128` on peer wire structs in this slice.

Wire JSON / u128: **no violations found** in audited V7 paths.

---

## Safety

| Area | Assessment |
|------|------------|
| Integer overflow | Balance/stake math uses `saturating_add` / `checked_add` in evac and transfer paths |
| Cross-account leakage | Not observed; conservation queue filtered by sender |
| Panics in hot path | `offchain` mutex `expect` and worker mutex poison `expect` — fail-fast on poison only |
| `unsafe` | None in V7 production pipeline/API paths; test-only `set_var` in `logging.rs:1146-1152` |
| DoS | Bounded worker queues (`handlers_tx.rs:275-281`); offchain batch unbounded in-memory growth (see MAJOR-001) |

---

## Tests

| Area | Coverage | Gap |
|------|----------|-----|
| `pending_conservation` API filter | `http_status.rs` sender vs other | None for `/v1/accounts` list with mixed pending rows |
| Stake evac | `emergency_activation_sweep_ok`, `emergency_evac_epoch_index`, `emergency_activation_no_stake` | No explicit double-activation evac attempt test (guarded by policy semantics) |
| Merkle | `merkle_root_*`, `merkle_proof_verify` | No negative test for tampered `position` / wrong sibling |
| Pipeline hot path | `worker.rs` module tests | No property test for hot vs full precheck equivalence |
| Offchain HTTP | Not observed in grep | Recommend handler integration test for batch + proof round-trip |

---

## Concurrency / parallelism

**Components:** `WorkerPool` OS threads + `std::sync::Mutex` receivers; `ArcSwap` `StateSnapshot`; `HotIndex` refreshed post-seal (`lifecycle.rs:1918-1920`); `OffchainStore` mutex; seal path holds `inner.write()` during `chain.seal_entries`.

**Hazards found:** None correctness-blocking. **Known window:** workers precheck against snapshot + hot index while seal mutates chain under write lock — stale precheck may reject briefly or hot path may false-reject until refresh; acceptable for admission path.

**Test gaps:** No stress test for seal + worker precheck interleaving; no offchain store contention test.

---

## Per-crate findings table

| Crate | BLOCKER | MAJOR | MINOR | NIT |
|-------|---------|-------|-------|-----|
| `pwm-core` | 0 | 0 | 2 | 0 |
| `pwmd` | 0 | 1 | 2 | 0 |
| `pwm-tui` | 0 | 0 | 0 | 2 |
| `pwm-cli` | 0 | 0 | 0 | 1 |
| **Total** | **0** | **1** | **4** | **5** |

---

## Recommended follow-ups (non-blocking)

1. Document offchain trust model in operator runbook (MAJOR-001 acceptance).
2. Add `verify_proof` strict position validation (MINOR-003).
3. HTTP test: `/v1/accounts` with pending conservation on one sender only.
4. TUI: parse/display `fee_pwm` in conservation detail (NIT-001).
5. Update `docs/adr/README.md` index for ADRs 0013–0015 (carry-over from V7-7 ADR review).

---

## Verdict

**PASS_WITH_FINDINGS** (approve with nits) — V7 focus paths satisfy security and correctness requirements. No BLOCKERs. One MAJOR operational note (unauthenticated ephemeral offchain store) is consistent with V7-5 MVP scope but must not ship as production trust anchor without follow-up.

---

## Participation

```yaml
agent: pwm-review
result: PASS
verdict: PASS_WITH_FINDINGS
artifacts: docs/reviews/20260629-v7-rust-code-audit.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 45000
  confidence: medium
```

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260629-v7-rust-code-audit.md'
git commit -m 'docs(audit): V7 Rust code audit report'
```