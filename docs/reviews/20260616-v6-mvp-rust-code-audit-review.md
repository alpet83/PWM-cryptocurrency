# Rust Code Audit — MVP V6 window

**Date:** 2026-06-16  
**Ticket:** `20260616-v6-mvp-rust-code-audit-review`  
**Audit window:** `522bcf1..3019528`  
**Audited paths:** V6-touched Rust files (37), prioritized around stake-gated admission, RFC16 leader rotation/failover, CONSERVATION delayed transfer, Mode B escrow, COSIGN_NON_DISABLEABLE, emergency activation evac, slashing evidence stubs, peer sync scoring, pwmd lifecycle/seal, snapshot v4/genesis  
**Categories checked:** all rust-code-audit categories + V6 behavioral/security regressions + missing tests  
**Tool:** V5 audit template, commit-window triage, CQDS `start_grep`, entity-name check, direct file reads

---

## Executive summary

| Severity | Count |
|----------|-------|
| Critical | 0 |
| High | 3 |
| Warning | 5 |
| Note | 4 |
| **Total** | **12** |

**Verdict:** `needs attention`

No rust-code-audit category produced a merge-blocking memory-safety finding in the V6-touched production files: no new V6 `unsafe` in scope, no `std::sync::Mutex` held across `.await` in production paths, no lifetime laundering, no large stack allocation hazard, and no public blanket-impl semver hazard.

The main risks are economic/liveness semantics in the V6 integration layer:

1. CONSERVATION delayed transfers do not reserve sender balance or nonce at enqueue; a conflicting tx before drain can invalidate the pending row, and `drain_conservation_at_height` silently drops failed drains.
2. Stake-gated epoch recompute can yield an empty active validator set after genesis; only cold-start height-1 fail-fast (`3019528`) exits the process — mid-chain empty sets stall sealing without the same fatal diagnostic.
3. RFC16 primary-miss failover (`skip_missed_h`) advances canonical height without sealing a block, without epoch-boundary side effects at the skipped height, and without wiring `UnavailableProposer` evidence stubs added in v6-9.

V6 surfaces otherwise show solid test coverage for happy paths (conservation drain, emergency evac, cosign enforcement, Mode B lock/refund/release, snapshot v4 wire, peer score deltas).

---

## Scope and triage

### Commit window

Reviewed window per ticket: `522bcf1..3019528` (includes `3019528`; excludes scripts-only commits after unless they touch Rust).

Representative commits in scope:

- `e92c095` / `a3ee3e2` / `8c7c363` / `1f48d73`: V6 GenCfg, activation_target wire, State types, reject wire stubs.
- `7d6557e`: snapshot schema v4 + V6 state wire.
- `2a59346`: stake-gated active validator set at epoch boundary.
- `2f9aa94` / `4d68dcb` / `6d802b0`: proposer rotation + trust-snapshot parity + primary-miss failover on seal loop.
- `937bb83`: Mode B cross-shard escrow lock, refund, import release.
- `b3750cf`: COSIGN_NON_DISABLEABLE runtime enforcement (ADR 0009).
- `85241e9`: ADR 0011 emergency activation evac + CLI prepared activation.
- `b9e0e1c`: CONSERVATION delayed Transfer queue + seal drain.
- `7086434`: evidence stubs + operator-local peer sync scoring.
- `de9ccb3` / `eaa288e`: CY soak genesis loader + `conservation_delay_blocks` wiring.
- `d251fb5`: pwmd replay aligned with seal rules (repair/io pick_prod_idx).
- `3019528`: fail-fast on empty startup validator set.

### Touched Rust files by subsystem

`git diff --name-only 522bcf1..3019528 -- 'crates/**/*.rs'` produced **37** Rust files:

- **Core state/tx/economics/consensus:** `crates/pwm-core/src/chain.rs`, `crates/pwm-core/src/genesis.rs`, `crates/pwm-core/src/reject_wire.rs`, `crates/pwm-core/src/state.rs`, `crates/pwm-core/src/tx.rs`, `crates/pwm-core/src/types.rs`.
- **Daemon lifecycle/snapshot/API/transport:** `crates/pwmd/src/lifecycle.rs`, `crates/pwmd/src/snapshot/genesis.rs`, `crates/pwmd/src/snapshot/io.rs`, `crates/pwmd/src/snapshot/repair.rs`, `crates/pwmd/src/snapshot/types.rs`, `crates/pwmd/src/api/common.rs`, `crates/pwmd/src/api/handlers_backfill.rs`, `crates/pwmd/src/api/handlers_peer.rs`, `crates/pwmd/src/transport.rs`, `crates/pwmd/src/transport/handshake_state.rs`, `crates/pwmd/src/transport/incoming_hello.rs`, `crates/pwmd/src/transport/peer_session/mod.rs`, `crates/pwmd/src/transport/peer_session/seed/session/steady_session.rs`, `crates/pwmd/src/transport/peer_session/sync_live.rs`, `crates/pwmd/src/transport/policy.rs`, `crates/pwmd/src/transport/score.rs`, `crates/pwmd/src/transport/transport_tick.rs`, `crates/pwmd/src/transport/tests/production.rs`, `crates/pwmd/src/slice20_e2e_tests.rs`, `crates/pwmd/src/tests/prelude.rs`, `crates/pwmd/src/tests/snapshot_roaming.rs`, `crates/pwmd/src/tests/transport_peer.rs`.
- **CLI / TUI:** `crates/pwm-cli/src/cli_cmd.rs`, `crates/pwm-cli/src/cli_dispatch.rs`, `crates/pwm-cli/src/cmd_account.rs`, `crates/pwm-cli/src/cmd_genesis.rs`, `crates/pwm-cli/src/cmd_roaming.rs`, `crates/pwm-cli/src/cmd_tx.rs`, `crates/pwm-cli/src/tests/mod.rs`, `crates/pwm-tui/src/marks_display.rs`, `crates/pwm-tui/tests/wallet_roaming.rs`.

### Narrowed high-risk scope (follow-up fix pass)

1. `crates/pwm-core/src/state.rs`: `drain_conservation_at_height`, conservation `Transfer` enqueue arm, `refund_exp_locks`, emergency evac on `Policy::ActivatePolicy`.
2. `crates/pwm-core/src/chain.rs`: `recompute_active_idxs`, `pick_prod_idx`, `roll_epoch_if_needed`, `seal` ordering (refund → apply → drain).
3. `crates/pwmd/src/lifecycle.rs`: `skip_missed_h`, `local_prod_for_h`, `mk_pick_fatal_diag` / `exit_fatal_pick`, seal loop failover window.
4. `crates/pwmd/src/snapshot/repair.rs` + `io.rs`: replay proposer parity with live seal rules.
5. `crates/pwmd/src/snapshot/genesis.rs`: v4/v5 loader for stake + `conservation_delay_blocks`.
6. `crates/pwmd/src/transport/score.rs`: operator-local scoring boundaries vs sync peer selection.

---

## Findings

### HIGH-001: CONSERVATION pending does not reserve balance; conflicting spend invalidates drain

**Location:** `crates/pwm-core/src/state.rs` — `Transfer` arm when `conservation_flag(&id)` (~405–421); tests `conservation_delay_enqueue` (~4096–4102) document intentional no-debit/no-nonce at enqueue.

**Why:** A conservation-flagged outgoing `Transfer` only pushes `PendingConservationTransfer` and returns. Sender `balance_pwm` and `nonce` stay unchanged until `drain_conservation_at_height`. `ConservationPendingExists` blocks only a second conservation `Transfer`, not `Export`, `Stake`, `Unstake`, or `Policy` txs that consume balance and bump nonce.

**Impact:** Sender can enqueue a delayed transfer, then `Export` or spend balance before `execute_at_height`. At drain, `apply_due_conservation` fails `BadNonce` or `Insufficient`, and the pending transfer is dropped (see HIGH-002). Recipient never receives funds; operator may believe the signed delayed transfer is still guaranteed. Breaks CONSERVATION “delayed but deterministic” expectation unless explicitly documented as soft reservation only.

**Fix direction:** Reserve `amount+fee` (and nonce slot) at enqueue, or reject any balance-affecting tx while `pending_conservation` exists for the sender. Add `conservation_export_race_reject` / `conservation_stake_race_reject` regression tests.

---

### HIGH-002: Failed conservation drains are silently discarded

**Location:** `crates/pwm-core/src/state.rs:251–261` — `drain_conservation_at_height`.

```rust
let _ = self.apply_due_conservation(row, current_height, gen_cfg);
```

**Why:** When `execute_at_height` is reached, each row is attempted once. Errors from `apply_due_conservation` are ignored; the row is not re-queued in `remaining`.

**Impact:** Deterministic silent loss of a queued conservation transfer on any drain-time failure (nonce drift, insufficient balance, policy redirect failure). All nodes apply the same drop, so consensus may stay aligned while user funds semantics are wrong. Complicates operator debugging — no log, no `TxError`, no evidence stub.

**Fix direction:** On failure, retain row with retry metadata, emit structured log/metric, or surface a consensus-visible evidence/reject path. At minimum, never drop without logging reason. Test `conservation_drain_insufficient_requeues_or_logs`.

---

### HIGH-003: Empty active validator set after genesis only fail-fast at height 1

**Location:** `crates/pwm-core/src/chain.rs:49–54` (`pick_prod_idx` empty-set error); `crates/pwmd/src/lifecycle.rs:1214–1262` (`mk_pick_fatal_diag` gates on `tip_h == 0 && lead_h == 1`).

**Why:** `recompute_active_idxs` can return `[]` when all validator stakes fall below `min_validator_stake` at an epoch boundary. `3019528` adds process exit only for the cold-start proposer pick at genesis height 1. Later, `pick_prod_idx` errors in seal/replay paths without the fatal exit path.

**Impact:** Mid-chain liveness stall: proposer seal loop spins, cluster gate never advances, operators lack the actionable `fatal_protocol_blocker` hint unless they recognize stake/epoch misconfiguration. Differs from genesis misconfig where pwmd now exits fast.

**Fix direction:** Extend fatal diagnostic to any height where `active_validator_indices` is empty and local role is proposer, or prevent empty set by consensus rule (minimum one validator must remain staked). Test `epoch_empty_active_set_midchain_diag`.

---

### WARN-001: Failover `skip_missed_h` does not record UnavailableProposer evidence

**Location:** `crates/pwmd/src/lifecycle.rs:1265–1271`, `1508–1517`; `crates/pwm-core/src/chain.rs:129–141` (`append_unavailable_proposer_evidence` exists but pwmd has zero call sites).

**Why:** v6-9 added evidence log stubs and duplicate-safe `append_evidence`, but primary-miss failover only bumps `canonical_h` via `set_canon_h` after one `nominal_ms` window.

**Impact:** Slashing/evidence pipeline cannot observe primary miss from live seal loop; operator forensics and future slashing hooks lack structured audit trail. Not a balance seizure issue (by design), but a spec/observability gap for RFC16 + v6-9.

**Fix direction:** On `skip_missed_h`, call `append_unavailable_proposer_evidence` with deterministic payload (missed height, expected `prod_idx`). Test `failover_appends_unavailable_proposer_evidence`.

---

### WARN-002: Skipped miss height bypasses epoch roll and block body

**Location:** `crates/pwmd/src/lifecycle.rs:1265–1271` (`skip_missed_h`); `crates/pwm-core/src/chain.rs:61–65` (`roll_epoch_if_needed` only at seal).

**Why:** Failover advances `canonical_h` without sealing a block at the missed height. If `epoch_length_blocks` divides the skipped height, `roll_epoch_if_needed` and `refund_exp_locks` / `drain_conservation_at_height` for that height never run on the canonical path.

**Impact:** Active validator set, conservation drains, and Mode B lock refunds scheduled for the skipped height are deferred to the next sealed block — potentially shifting economics by one block relative to RFC16 “≤1 miss” intent. Likely acceptable for default epoch lengths ≫ 1, but sharp edge for lab configs with `epoch_length_blocks=1` or small values.

**Fix direction:** Document normatively, or run height-scoped state hooks (epoch roll, refund, drain) when skipping. Test `failover_epoch_boundary_h1_skipped`.

---

### WARN-003: Production fn name policy violation in genesis loader

**Location:** `crates/pwmd/src/snapshot/genesis.rs:164` — `def_min_val_stake_s` (5 segments, prod limit 4).

**Impact:** Style/policy drift; not a runtime bug.

**Fix direction:** Rename to `def_min_val_stake` or similar ≤4 segments per `AGENT_PROMPT_coding.md`.

---

### WARN-004: Snapshot repair remains multi-file non-atomic

**Location:** `crates/pwmd/src/snapshot/repair.rs:78–80` (unchanged pattern; V6 slice aligned replay with `pick_prod_idx` / `roll_epoch_if_needed`).

**Impact:** Offline repair can leave partial rewrites on failure (carried from V5 audit).

**Fix direction:** Temp-dir swap or mandatory backup on non-dry-run; document partial-failure semantics.

---

### WARN-005: Peer sync scoring is operator-local with no consensus effect

**Location:** `crates/pwmd/src/transport/score.rs` (entire module); wired from transport tick / peer session in window.

**Why:** Scores influence sort order for operator peer selection only — by design per v6-9 — but misconfiguration could starve sync if treated as consensus truth.

**Impact:** Low security risk if documented; medium ops risk if runbooks imply slashing from scores.

**Fix direction:** Ensure operator docs state “non-consensus, no balance seizure”; optional cap/floor on score-driven disconnect.

---

### NOTE-001: CONSERVATION soft reservation documented only in unit tests

**Location:** `crates/pwm-core/src/state.rs` tests `conservation_delay_enqueue`, `conservation_seal_drains` (`chain.rs`).

**Why:** Behavior (balance unchanged until drain) is test-locked but not cited in reviewed RFC snippets for this pass.

**Fix direction:** Add normative sentence in CONSERVATION RFC/ADR: reserve vs soft-queue semantics.

---

### NOTE-002: `logging.rs` pre-existing `unsafe` env blocks outside V6 diff

**Location:** `crates/pwmd/src/logging.rs:1146+` (CQDS grep; file not in V6 touched list).

**Impact:** Out of V6 slice scope; flag for separate hygiene pass.

---

### NOTE-003: IPv4 duplicate phase rejection fixed since V5

**Location:** `crates/pwmd/src/snapshot/genesis.rs:267–276` — `parse_claim_phases` now rejects duplicate `phase` IDs (V5 WARN-004 closed).

---

### NOTE-004: V6 fail-fast cold start well tested

**Location:** `crates/pwmd/src/lifecycle.rs` tests `prod_pick_fatal_start`, `prod_pick_non_fatal_after_start` (~2483–2551).

---

## V6 focus surface assessment

| Surface | Assessment |
|---------|------------|
| Stake-gated validator admission | Implemented in `recompute_active_idxs` + epoch boundary; cold-start fail-fast good; mid-chain empty set gap (HIGH-003). |
| RFC16 leader rotation / failover | `pick_prod_idx(height % len)` consistent across chain, lifecycle, repair, sync; failover skip ≤1 block via `nominal_ms`; evidence/epoch side effects incomplete (WARN-001/002). |
| CONSERVATION delayed transfer | Queue + seal drain wired; soft reservation + silent drop risks (HIGH-001/002). |
| Mode B EXPORT lock / refund / IMPORT | Lock on export, `refund_exp_locks` at seal, release on import; tests in `state.rs`; repair replay aligned in V6. |
| COSIGN_NON_DISABLEABLE | `cosign_non_dis` + `policy_weakens_cosign` in `validate_pol_action` and tx shape validation; tests present. |
| Emergency activation + evac | `activation_target` required/matched; balance evac on activate; cancels pending conservation (tested). |
| Slashing evidence stubs | Core append/duplicate reject; no pwmd wiring on failover (WARN-001). |
| pwmd lifecycle / seal | Variant C deadline scheduler, cluster gate, fail-fast `3019528`; dense but tested paths. |
| Snapshot v4 / genesis | `conservation_delay_blocks`, `min_validator_stake` string JSON; v4 state wire fields with `ser_json_u128` on economic containers. |

---

## rust-code-audit category results

### CAT-1 Lifetime laundering

No findings in V6-touched files.

### CAT-2 `std::sync::Mutex` across `.await`

No production findings. Only test helper import in `pwm-tui/tests/common/mod.rs`.

### CAT-3 Drop / RAII trap

WARN-004: snapshot repair multi-step writes not group-atomic.

### CAT-4 `unsafe` without `// SAFETY:`

No new V6 production `unsafe` in touched files. Pre-existing `logging.rs` blocks noted (NOTE-002).

### CAT-5 Async cancellation safety

Not re-audited deeply in V6 window (V5 WARN on HTTP direct-seal still applies to `handlers_tx.rs`, outside V6 diff). Seal loop uses structured `tokio::select!` wake paths; no new cancel-safety regression identified in touched lifecycle code.

### CAT-6 Blanket impl semver hazard

No findings.

### CAT-7 Large stack allocation

No findings in V6-touched files.

### Integer overflow / economics

Transfer/Export/Import use `checked_add` on amounts+fees where relevant; stake paths use saturating ops in rewards. Conservation `fee_pwm` narrowed to `u64` at enqueue — consistent with pending row shape.

### Silent error swallow (`let _ =`)

HIGH-002: conservation drain. Other `let _ =` hits in V6 scope are mostly test cleanup or non-critical flush (`block_timing::try_flush_once`).

### Consensus non-determinism

Seal time mode defaults to wall clock in production; deterministic mode used in tests. Failover timing tied to `nominal_ms` grid — nodes with same genesis params should agree on skip window.

---

## Wire JSON / u128

**Scope:** V6 touched snapshot v4 state wire (`SnapshotStateV4`), core `State` / `PendingConservationTransfer` / `CrossShardLock` / `ExportProvenance`, tx JSON mirrors, genesis v4/v5 string fields — not new `PeerWireMsg` enum variants in this window.

- `PendingConservationTransfer.amount_pwm`, `CrossShardLock.amount_pwm`, `ExportProvenance.amount`: `#[serde(with = "crate::ser_json_u128")]`.
- `SnapshotStateV4` stores `fee_pool` and account balances as decimal strings in snapshot rows (v3 pattern retained).
- `TxBody` economic fields retain `ser_json_u128` in serialize + manual deserialize mirror.
- Genesis loader: `parse_u128_json` for `min_validator_stake`, `block_reward`, etc.
- `State` still derives `Serialize/Deserialize` for local/bincode digest — not peer JSON wire; snapshot conversion uses typed v4 wire structs.

**Conclusion:** No derive-only `u128` on new peer-facing JSON in V6 focus. Snapshot v4 economic fields use string encoding. No request-change for wire `u128` in this audit.

---

## Style and module shape

- `python scripts/check_entity_name_segments.py` on all 37 V6-touched `.rs` files: **one** production violation — `def_min_val_stake_s` (WARN-003).
- V6 modules generally carry `//!` banners (`transport/score.rs`, `snapshot/repair.rs`, etc.).
- `state.rs` / `lifecycle.rs` remain the highest-density modules; no new facade blob regression beyond expected V6 feature growth.

---

## Missing tests / follow-up ticket suggestions

Suggested ticket slugs (ids only):

1. `20260617-v6-conservation-balance-reserve` — HIGH-001/002: reserve or block conflicting spend; no silent drain drop.
2. `20260617-v6-empty-active-set-midchain` — HIGH-003: liveness diagnostic or protocol rule.
3. `20260617-v6-failover-evidence-wire` — WARN-001: UnavailableProposer on `skip_missed_h`.
4. `20260617-v6-failover-epoch-skip` — WARN-002: epoch/conservation/refund at skipped height.
5. `20260617-v6-genesis-fn-rename` — WARN-003: `def_min_val_stake_s` rename.
6. `20260617-v6-snap-repair-atomic` — WARN-004: repair atomicity (carryover).

---

## Open assumptions / questions

- Is CONSERVATION intentionally a **soft queue** (no balance lock until drain)? Tests imply yes; RFC should state it explicitly or HIGH-001 must be fixed.
- Should mid-chain empty `active_validator_indices` be fatal for proposers (like genesis), or should protocol forbid unstaking below quorum?
- Should failover primary-miss append evidence in v6.0 or defer to post-publication slashing sprint?

---

## Verification performed

- `git diff --name-only 522bcf1..3019528 -- 'crates/**/*.rs'` — 37 files.
- `git log --oneline --no-merges 522bcf1..3019528 -- 'crates/**/*.rs'` — 17 commits.
- CQDS `cq_files_ctl#start_grep` on `unsafe`, `std::sync::Mutex`, `let _ =`, `pick_prod_idx`, `pending_conservation`, `min_validator_stake`, `COSIGN_NON_DISABLEABLE`, `activation_target`, `evidence`, `refund_exp_locks`, `policy_weakens_cosign`.
- `python scripts/check_entity_name_segments.py` on full V6 Rust file list.
- Direct reads: `chain.rs`, `state.rs` (conservation, Mode B, emergency, cosign), `lifecycle.rs` (seal loop, failover, fail-fast), `snapshot/genesis.rs`, `snapshot/types.rs`, `transport/score.rs`, `repair.rs` diff.

No product Rust was modified by this audit.

---

## Participation / token estimate

```yaml
agent: pwm-review
result: PARTIAL
artifacts: docs/reviews/20260616-v6-mvp-rust-code-audit-review.md
token_usage:
  source: estimate
  input: 42000
  output: 7500
  total: 49500
  confidence: medium
```

Result is `PARTIAL` / `needs attention` because follow-up issues need coding-owner decisions or fixes before publication sign-off.

---

## Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'P:\opt\docker\PWM-cryptocurrency'
git add 'docs/reviews/20260616-v6-mvp-rust-code-audit-review.md'
git add 'tasks/20260616-v6-mvp-rust-code-audit-review.json'
git add 'tasks/20260603-v6-prepublication-umbrella.json'
git commit -m 'docs(v6): pre-publication rust code audit review'
```
