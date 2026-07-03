# Security scope: conservation transfer flow (V7-8)

- **date:** 2026-07-02
- **ticket:** `20260702-conservation-security-scope-review`
- **commit:** `8d47e3d`
- **agent:** `pwm-review` (`pwm_review`)
- **purpose:** Structured attack-surface scope for high-capability security analysis (Fable 5). This is **not** a pass/fail implementation verdict — the deliverable is this scope document.
- **scope IN:** Conservation transfer from mempool/RPC submission through seal enqueue, pending window, and `drain_conservation_at_height` execution.
- **scope OUT:** TUI display, roaming, ClickHouse snapshot (per ticket).

---

## 1. Executive summary

V7-8 routes outgoing `Transfer` from addresses with the `CONSERVATION` flag (bit 1 of `AccountId` address bytes) into a height-delayed `pending_conservation` queue (`GenCfg.conservation_delay_blocks`, default 86 400 blocks) instead of debiting immediately. Execution at `execute_at_height` re-validates nonce, balance (`amount + fee`), and recipient routing policies, then debits/credits and advances nonce. There is **no balance reservation** at enqueue — funds remain liquid subject to narrow pending-window tx restrictions — and failed drains **re-queue indefinitely** rather than drop. Primary security questions for deep analysis are: soft-reservation fund safety, nonce/pending interaction with emergency evacuation, infinite-retry liveness, public API leakage of pending transfers, and whether signing covers all conservation-determining fields.

---

## 2. Attack surface map

| Area | Severity estimate | Notes |
|------|-------------------|-------|
| Enqueue without debit / no reservation | **Medium** | Balance checked at enqueue (`state.rs:467–468`) but not locked; sender cannot spend via most tx types while pending (`pending_tx_conflict`, `ConservationPendingExists`) — verify no bypass paths (incoming credit only helps execution). |
| Drain-time balance / fee re-check | **Medium** | `apply_due_conservation` debits `amount + fee` atomically at execution (`state.rs:324–335`); insufficient balance → `Err` → row re-pushed (`state.rs:282–290`). Recipient liveness risk, not obvious double-spend. |
| Infinite drain retry / queue growth | **Low–Medium** | Permanent `Err` (e.g. policy reject, `BadNonce` without pending clear) pins row in `pending_conservation` forever; O(n) scan each seal. One pending per sender caps per-account abuse. |
| Replay / double-submit | **Low** | `ConservationPendingExists` blocks second pending per sender; post-execution `BadNonce` blocks same signed tx; `tx_hash` stored but **unused** for runtime dedup (ADR 0009 idempotency field dormant). |
| Nonce semantics (enqueue skips advance) | **Medium** | Conservation enqueue leaves `nonce` unchanged (`state.rs:486`); only emergency `ActivatePolicy` allowed with same nonce while pending (`pending_pol_allowed`); emergency clears pending + advances nonce. Fable 5 should prove no nonce-reuse path invalidates or duplicates pending row. |
| Emergency evacuation vs drain (same block) | **Low–Medium** | Seal order: all txs → `refund_exp_locks` → `drain_conservation_at_height` (`chain.rs:187–202`). Emergency in tx loop clears pending before drain (`state.rs:711–712`). Same-block enqueue+execute only if `delay=0`. |
| API: `/v1/accounts` pending exposure | **Low–Info** | List endpoint attaches `pending_conservation` (amount, recipient, heights) for **every** account (`handlers_account.rs:60`). Pre-mainnet privacy / targeting concern. |
| Signature / flag binding | **Low** | `signing_message` covers domain, signer, derivation, nonce, transfer body (`tx.rs:398–446`); `conservation_flag` read from `computed_account_id()` bytes, not `Init.flags` (`types.rs:24–33`, `state.rs:392–470`). Mismatch between signer identity and flag encoding is consensus-critical. |
| Recipient policy re-evaluation at drain | **Medium** | `conservation_recipient_dst` re-run at execution (`state.rs:321`, `1018–1056`) — redirect/deny may change between enqueue and execute; row retries forever on persistent reject. |
| Parallel precheck / SEDA (V7) | **Low (spot-check)** | Precheck clones `State` including `pending_conservation`; seal remains single-threaded `Chain::seal`. Race only if admission and seal paths diverge — parity claimed via `precheck_apply_with_ctx`. |

---

## 3. Detailed findings per focus area

### 3.1 `apply_tx` conservation path — balance before conservation check, reservation

**Observed behavior**

- Global nonce check runs first (`state.rs:418–420`).
- Pending conflict guard (`state.rs:429–431`) then conservation-specific enqueue (`state.rs:458–486`).
- For conservation senders: balance `>= amount + fee` at enqueue (`state.rs:467–468`), but **no debit**, **no nonce increment**, push to `pending_conservation`, early `Ok(())`.
- `execute_at_height = inclusion_height + gen_cfg.conservation_delay_blocks` (`state.rs:482–483`).

**What appears safe**

- Sender cannot issue a second conservation transfer while pending (`ConservationPendingExists`, tested `conservation_pending_exists_reject`).
- Non-transfer ops that spend balance/nonce are blocked while pending: `pending_tx_conflict` returns `true` for `Stake`, `Export`, `BurnMark`, etc. (`state.rs:63–68`; tests `conservation_stake_race_reject`, `conservation_export_race_reject`).
- Emergency `ActivatePolicy` (routing emergency redirect) is explicitly allowed and clears pending (`state.rs:55–60`, `711–712`; test `conservation_emergency_cancels_pending`).

**Needs deeper analysis (Fable 5)**

- **No hard reservation:** ADR 0009 models “applies if still valid at execution.” Confirm no alternate code path debits sender balance without incrementing nonce during the pending window.
- **Incoming transfers to conservation sender** credit balance without using sender nonce — can rescue a row that failed drain for insufficient funds (`conservation_drain_insufficient_requeue`). Is this intended economics or a griefing vector against recipients waiting for execution?
- **Order of checks:** Balance is checked before `conservation_flag` branch but after nonce/signature — confirm no edge case where uninitialized or wrong-signer accounts enqueue pending rows.

**Open questions**

- Should soft reservation be documented as an explicit threat model acceptance (funds not earmarked)?
- Does any wallet/mempool layer treat enqueue as “spent” and mislead users?

---

### 3.2 Replay and double-submit — `ConservationPendingExists`

**Observed behavior**

- Second outgoing transfer from same conservation sender while a row exists → `ConservationPendingExists` (`state.rs:471–472`, `429–430`).
- `Transfer` body is **not** a `pending_tx_conflict` (`state.rs:65`) — duplicate transfer rejection relies on the conservation-specific `any(|row| row.sender == id)` check, not the generic guard alone.
- After successful drain, `nonce` increments (`state.rs:334`); replay of same signed tx fails `BadNonce` at `apply_tx` (`state.rs:418–420`).
- `PendingConservationTransfer.tx_hash` = `blake3(signing_message())` (`state.rs:484`) — matches `SignedTx::tx_hash()` (`tx.rs:534–535`) but is **never consulted** on apply or drain.

**What appears safe**

- Same tx cannot enqueue twice while pending (tested).
- Same tx cannot re-execute after success (nonce gate).

**Needs deeper analysis**

- **Block-level duplicate:** If the same `SignedTx` appeared twice in one block’s tx list, first application enqueues, second should hit `ConservationPendingExists` — verify proposer/mempool cannot grief this into inconsistent state.
- **`tx_hash` idempotency:** ADR 0009 wire shape documents `tx_hash` for idempotency; implementation stores it without dedup — intentional or gap?
- **Replay after emergency cancel:** Pending cleared, nonce may advance via policy tx; original transfer signature still nonce-bound — confirm no path re-submits stale pending from snapshot restore without re-validation.

---

### 3.3 Nonce semantics — conservation tx does not advance nonce at enqueue

**Observed behavior**

- Enqueue: `nonce` unchanged (test `conservation_delay_execute` asserts sender `nonce == 0` after enqueue).
- Execute: `from.nonce += 1` in `apply_due_conservation` (`state.rs:334`).
- While pending, only txs with **same** `tx.nonce` can pass the global check (`state.rs:418–420`).
- Allowed same-nonce tx: emergency `ActivatePolicy` only (`pending_pol_allowed`); it advances nonce and clears pending (`state.rs:730–731`, `711–712`).

**What appears safe**

- Cannot submit `Stake`/`Export`/second `Transfer` with same nonce while pending (blocked).
- Emergency path is tested end-to-end (`conservation_emergency_cancels_pending`).

**Needs deeper analysis**

- **Nonce advancement without pending clear:** Prove no other `PolicyAction` or admin path can advance nonce while leaving a pending row (would cause permanent `BadNonce` retry loop in drain).
- **Different tx type reusing nonce after failed drain:** If drain keeps failing, nonce stays at enqueue value — can user bind unrelated future operations to that stale nonce window?
- **TUI/wallet `track_nonce=false` for conservation** (`CHANGELOG.md`) — confirm off-chain tooling does not emit conflicting txs.

---

### 3.4 Fee handling — recorded at enqueue, deducted at execution

**Observed behavior**

- `fee_pwm: u64` stored on pending row from transfer fee (`state.rs:474–479`).
- At execution: `fee = u128::from(row.fee_pwm)`, `total = amount + fee`, debit `total`, `fee_pool += fee` (`state.rs:323–335`).
- Fee is **not** reserved at enqueue; taken from live balance at execution.

**What appears safe**

- Atomic debit of amount and fee together prevents partial pay of amount without fee.
- `fee_pool` only credited on successful execution (test `conservation_delay_execute`).

**Needs deeper analysis**

- **Balance covers amount but not fee at execution:** `Insufficient` → re-queue (`conservation_drain_insufficient_requeue`). Recipient waits indefinitely; sender funds may be stuck neither delivered nor freed — liveness / UX security.
- **Fee policy changes:** `fee_pwm` is fixed at enqueue — confirm no genesis/param change retroactively affects pending rows (height-only delay suggests frozen).
- **u128 fee in body vs u64 in pending row:** `try_from(*fee)` at enqueue (`state.rs:474`) — overflow rejected at enqueue; verify no truncation mismatch at execution.

---

### 3.5 `drain_conservation_at_height` error path — infinite retry

**Observed behavior**

```274:294:crates/pwm-core/src/state.rs
    pub fn drain_conservation_at_height(&mut self, current_height: u64, gen_cfg: &GenCfg) {
        let mut remaining = Vec::with_capacity(self.pending_conservation.len());
        let pending = std::mem::take(&mut self.pending_conservation);
        for row in pending {
            if current_height < row.execute_at_height {
                remaining.push(row);
                continue;
            }
            match self.apply_due_conservation(row.clone(), current_height, gen_cfg) {
                Ok(()) => {}
                Err(err) => {
                    eprintln!(...);
                    remaining.push(row);
                }
            }
        }
        self.pending_conservation = remaining;
    }
```

- Failed rows are **re-pushed**; test `conservation_drain_insufficient_requeue` confirms retry after balance restored.
- **Note:** V6 review (`v6-sprint8-conservation-coding-review-20260607.md`) described silent drop; current code at `8d47e3d` re-queues (behavior change).

**What appears safe**

- No double-credit on retry — successful path removes row from queue.
- Transient insufficient funds can resolve (tested).

**Needs deeper analysis**

- **Permanent failure modes:** `BadNonce`, `PolicyRoutingDenied`, `RecipientNotInitialized`, `PolicyAccountFinalized` — all lead to infinite retry + `eprintln!` only. Does this constitute DoS against seal performance or recipient settlement?
- **Queue head-of-line blocking:** `Vec` FIFO scan — one stuck row does not block others (all rows processed each seal), but global work grows with stuck entries.
- **Observability:** No metric/event for stuck drains — operators may not detect liveness failures.

---

### 3.6 Emergency evacuation vs conservation drain (same block seal)

**Observed behavior**

- `Chain::seal`: apply all block txs → `refund_exp_locks` → `drain_conservation_at_height` → rewards (`chain.rs:187–217`).
- Emergency activation: `pending_conservation.retain(|row| row.sender != id)` then evacuate balance/stake to `activation_target` (`state.rs:709–727`).
- Fresh enqueue in block *N* has `execute_at_height = N + delay` — not drained in same block when `delay > 0`.

**What appears safe**

- Emergency in tx phase clears pending before drain runs (test `conservation_emergency_cancels_pending`).
- ADR 0012/0009 require deterministic cancel on evac — implemented via `retain`.

**Needs deeper analysis**

- **`delay = 0` genesis misconfiguration:** Same-block enqueue and drain — interaction with intra-block tx ordering.
- **Evac mid-pending with partial balance spend:** Emergency moves full balance; pending row cleared — recipient never receives (intended cancel?). Confirm no path evacuates only part of balance leaving a still-valid pending row.
- **Recipient emergency redirect at drain time:** `conservation_recipient_dst` may redirect away from original recipient — confirm this cannot send funds to an attacker-controlled redirect that was not plausible at enqueue.

---

### 3.7 API exposure — `/v1/accounts` pending_conservation

**Observed behavior**

- `GET /v1/accounts`: iterates all accounts, sets `out.pending_conservation = pending_conservation_out(&g.chain.st, id)` per account (`handlers_account.rs:38–61`).
- `GET /v1/account/:id`: same filter for single account (`handlers_account.rs:101`).
- `pending_conservation_out` exposes: `recipient` (hex), `amount_pwm`, `fee_pwm`, `nonce`, `enqueue_height`, `execute_at_height` (`handlers_account.rs:16–28`).
- Empty vec omitted from JSON via `skip_serializing_if = "Vec::is_empty"` on `AcctOut` (`types.rs:523–524`).

**What appears safe**

- Only rows where `row.sender == key` — no cross-account leak of pending metadata.
- Http test coverage exists (`pwmd/src/tests/http_status.rs`).

**Needs deeper analysis (pre-publication)**

- **Global enumeration:** List endpoint reveals which accounts have pending conservation and full transfer metadata — enables targeting of high-value delayed transfers.
- **Recipient hex exposure:** Correlates conservation senders with counterparties before settlement.
- **Execute height timing:** Discloses exact block when funds will move — MEV / physical-security relevance?

---

### 3.8 Signature coverage and `conservation_flag` binding

**Observed behavior**

- `id = tx.computed_account_id()` drives `conservation_flag(&id)` (`state.rs:392`, `470`).
- `conservation_flag` reads `address_flags(id)` from **AccountId bytes** `[2..6]` (`types.rs:24–33`), not from on-chain `Account.flags` set at `Init`.
- `signing_message` includes: `PWMv0/TX`, domain, `signer_pk`, `derivation_index`, `nonce`, transfer discriminator, `to`, `amount`, `fee` (`tx.rs:398–446`).
- Signature verified against `signer_pk` before apply (`validate_tx_shape` / `validate_shape_no_sig`).
- Account binding: `acc.signing_pubkey == tx.signer_pk` (and derivation) (`state.rs:407–410`).

**What appears safe**

- Transfer amount/recipient/nonce/domain bound by signature.
- Conservation behavior tied to address derivation (flag baked into id at keygen) — cannot toggle conservation via `Init.flags` alone.

**Needs deeper analysis**

- **Signer vs `from` field:** There is no separate `from` in body — identity is `computed_account_id()`. Confirm HD derivation cannot produce conservation-flag id with non-conservation intent.
- **Cosign paths:** Conservation accounts may require cosign (`cosign_non_dis` interaction) — verify `evaluate_policy` runs before enqueue and cosign cannot be stripped mid-pending.
- **`tx_hash` in pending row** uses signing message at enqueue time — if cosign requirements differ at drain, confirm policy re-check is complete.

---

### Concurrency / parallelism

Components: `State.pending_conservation` (`Vec`, single mutex via `Chain` seal), `drain_conservation_at_height` at end of seal, RPC reads via `g.inner.read()` (`handlers_account.rs:36`).

- **Shared mutable state:** Pending queue mutated only during `apply_tx` (enqueue/clear) and `drain_conservation_at_height` — both on seal thread. No lock held across `.await` in core path.
- **Hazards:** Precheck clones full state for admission; parallel precheck workers (V7 SEDA) must not assume pending queue is empty if another worker enqueued — Fable 5 should verify admission/seal parity under interleaved conservation txs.
- **Test gaps:** No stress test for many conservation accounts draining same height; no concurrent precheck+seal test for conservation.

---

## 4. Recommended prompt for Fable 5 agent

Use the following prompt (adjust model-specific wrappers as needed):

---

**Task:** Perform a high-capability security analysis of the PWM conservation transfer mechanism at commit `8d47e3d`. Goal: fund-safety, liveness, and policy bypass — not style review.

**Threat model:** Adversarial account holder with conservation-flagged address; adversarial proposer ordering txs within a block; RPC reader enumerating `/v1/accounts`; honest recipients relying on delayed settlement. Assume default `conservation_delay_blocks = 86400` unless testing param edge cases.

**Primary invariants to prove or refute:**

1. Conservation sender cannot reduce spendable balance below `amount + fee` at execution time through any allowed tx sequence while pending row exists (unless emergency evac or successful drain).
2. At most one effective settlement per enqueued conservation transfer (no double credit to recipient, no double debit ambiguity).
3. Pending row cannot survive emergency evacuation on the same sender.
4. Replay of identical or mutated signed transfers cannot bypass `ConservationPendingExists` or `BadNonce`.
5. Drain failures cannot cause consensus divergence across nodes replaying the same blocks.

**Files and line ranges (read in this order):**

| File | Lines | Focus |
|------|-------|-------|
| `crates/pwm-core/src/types.rs` | 15–34 | `CONSERVATION` flag decode from `AccountId` |
| `crates/pwm-core/src/genesis.rs` | 22, 115, 232 | `conservation_delay_blocks` default |
| `crates/pwm-core/src/tx.rs` | 398–446, 534–536, 905–915 | `signing_message`, `tx_hash`, error enums |
| `crates/pwm-core/src/state.rs` | 55–68, 125–136, 255–343, 418–486, 692–733, 1018–1056 | pending guards, enqueue, drain, emergency cancel, recipient dst |
| `crates/pwm-core/src/chain.rs` | 187–217, 346–389 | seal ordering, integration test |
| `crates/pwmd/src/api/handlers_account.rs` | 16–61 | API exposure |
| `docs/adr/0009-address-flags-runtime-enforcement.md` | full | normative contract vs implementation |
| `docs/adr/0012-emergency-stake-evacuation.md` | § evacuation + pending cancel | emergency interaction |

**Tests to execute or reason about:** `conservation_delay_execute`, `conservation_pending_exists_reject`, `conservation_drain_insufficient_requeue`, `conservation_emergency_cancels_pending`, `conservation_stake_race_reject`, `conservation_export_race_reject`, `conservation_incoming_not_delayed`, `conservation_seal_drains`.

**Explicit attack scenarios to model:**

- A: Enqueue max transfer, attempt stake/export/second transfer/burn with same nonce while pending.
- B: Enqueue, drain balance to zero via allowed paths, observe drain retry; restore balance via incoming transfer; confirm single settlement.
- C: Enqueue, activate emergency routing same block as `execute_at_height`, confirm cancel before drain credits recipient.
- D: Replay identical tx after successful execution; replay while pending.
- E: Enumerate `/v1/accounts` for pending metadata leakage impact.
- F: Recipient activates `RoutingEmergencyRedirect` or `RoutingSameDomainOnly` during pending window — drain retry behavior.
- G: `conservation_delay_blocks = 0` — same-block enqueue + drain ordering.

**Deliverable:** Ranked findings (Critical/High/Medium/Low/Info) with proof sketches or counterexamples; suggested mitigations only where invariant fails.

**Out of scope:** TUI (`pwm-tui`), roaming, ClickHouse snapshot persistence.

---

## 5. Verdict

**Approve with nits** — scope document complete for Fable 5 handoff. Implementation is structurally aligned with ADR 0009 pending-only profile; highest-value deep dives are soft reservation economics, infinite drain retry liveness, unused `tx_hash` idempotency, and `/v1/accounts` enumeration privacy.

### Nits (scope-quality, non-blocking)

1. **Doc drift:** V6 review described silent drop on drain failure; `8d47e3d` re-queues — ensure ADR/CHANGELOG reflects current behavior.
2. **`tx_hash` unused:** ADR wire comments promise idempotency; runtime does not consult it — clarify normative intent.
3. **`ConservationDelayRequired`:** Enum/wire exist but pending-only profile never emits — note for mempool profile switches.
4. **Fable 5 should add:** explicit test or proof for block-level duplicate tx inclusion and `delay=0` edge case.

---

## 6. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260702-conservation-security-scope.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 32000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260702-conservation-security-scope.md'
git commit -m 'docs(v7-8): conservation transfer security scope for Fable 5 (8d47e3d)'
```