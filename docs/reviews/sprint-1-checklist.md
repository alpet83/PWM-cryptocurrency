# Sprint Checklist (Sprint 1)

**Sprint:** `Sprint 1 / Two-Shard Runtime Foundation`  
**Dates:** `2026-04-24 .. 2026-05-08`  
**Scope reference:** `docs/WHITE_SPEC_v0.md`, `docs/rfc/1-address-format.md`, `docs/rfc/6-policy-engine.md`, `docs/PHASE1_CHECKLIST.md`  
**Status:** `in_progress`

---

## 1) Sprint Goal

- **Primary objective:** Stand up two independent shard runtimes (`A` and `B`) while preserving account-core compatibility and strict-upgrade behavior.
- **Demo-ready definition for this sprint:** Operator can launch both shards locally and execute one valid local tx per shard with deterministic behavior: Phase1 recipient prefilter + pinned process-shard map (Regulatory vs TNC) + protocol `TRANSFER` gate on `domain_hi(sender) == domain_hi(receiver)`.
- **Out of scope:** Cross-shard value transfer finalization, hidden UTXO migration/pivot, production perf tuning beyond smoke-level checks.

---

## 2) Shared Constraints (for all roles)

- [x] Keep strict-upgrade from v0 (no hidden core rewrite).
- [x] Keep protocol-derived routing (`domain_hi(sender)` vs `domain_hi(receiver)`).
- [x] Preserve baseline compatibility on local same-shard flow.
- [x] Keep docs/spec alignment with current agreed baseline.

---

## 3) Responsibility Matrix

## `pwm-coding` (owner: implementation)

### Inputs to fill
- **Implementation scope:** `pwmd` shard runtime bootstrap/config, pinned shard processes (A/B) + `/v1/tx` guards (prefilter + protocol `domain_hi` compare), local tx execution path on shard A/B, minimal operator launch script/docs.
- **Design notes / assumptions:** Two shard processes stay isolated by state/config; account-core data model remains unchanged; **process shard** is an operator/runtime partition pinned to Phase1 domain classes, while **local `TRANSFER` acceptance** follows protocol comparison of `domain_hi(sender)` vs `domain_hi(receiver)` (no operator override path for tx domain choice).
- **Risky areas:** Misrouted tx on boundary domain values, accidental shared state between shard processes, startup drift between shard configs.

### Required checklist
- [x] Implementation plan split into 5 atomic steps (bootstrap, config split, router wiring, tx execution, demo script).
- [x] Explicit changed-file list maintained in sprint notes.
- [x] Negative cases defined before coding: wrong `domain_hi`, shard-down submit, malformed account domain.
- [x] Backward-compat note captured for unchanged account-core semantics.
- [x] Demo run path documented with exact commands and expected logs for both shards.

### Implementation notes (Sprint 1 foundation)
1. Bootstrap/config: `pwmd` adds `--shard A|B`, shard-default ports (`3030/3031`) and shard namespace path (`state/shard-a|shard-b/pwm-data.json`).
2. Config split/isolation: each shard writes to its own state namespace; no shared default storage path.
3. Router wiring: `/v1/tx` enforces Phase1 recipient prefilter, pinned process-shard membership (Regulatory vs TNC), protocol `TRANSFER` gate on `domain_hi(sender) == domain_hi(receiver)`, and logs decisions (`tx routing guard: ...`).
4. Local tx execution: same-shard tx path remains `INIT/TRANSFER/STAKE/UNSTAKE/BURN_MARK` without account-core changes.
5. Demo path: commands and expected observables documented below.

### Changed files (implementation)
- `crates/pwmd/src/lib.rs`
- `crates/pwmd/src/main.rs`
- `docs/reviews/sprint-1-checklist.md`

### Negative cases covered
- Wrong process shard submit (sender domain class does not match `--shard`): node returns `409 Conflict` with process-shard mismatch text.
- Cross-domain `TRANSFER` on local path (`domain_hi` differs): node returns `409 Conflict` (`EXPORT/IMPORT` required).
- Phase1 recipient policy violations (reserve/witness/unknown-domain / domain-miss): node returns `400 Bad Request` before mempool.
- Malformed account domain/signature shape: existing tx validation still returns `400 Bad Request`.

### Backward-compat note
- Account-core model and local v0 tx semantics are unchanged; Sprint 1 adds only shard bootstrap/guards/logs around existing local tx flow.

### Output artifacts
- `Code changes`: `crates/pwmd/*`, `crates/pwm-core/*` (if routing helpers required), `docs/reviews/sprint-1-checklist.md`, operator run notes under `docs/`.
- `Technical notes`: Sprint implementation note in PR description plus routing decision comments in code.
- `Demo commands`: `cargo run -p pwmd --bin pwmd -- --shard A ...`, `cargo run -p pwmd --bin pwmd -- --shard B ...`, shard-local tx submit via CLI/API.

---

## `pwm-testing` (owner: verification)

### Inputs to fill
- **Test scope:** Targeted unit + integration smoke for shard bootstrap/routing/tx; manual visual checks only if TUI/perf instability is suspected.
- **Coverage focus:** Domain-based routing determinism, shard isolation, happy-path local tx execution on each shard, upgrade compatibility on existing account-core flows.
- **Known gaps to monitor:** No full perf soak in Sprint 1; cross-shard finality scenarios deferred.

### Required checklist
- [ ] Regression subset defined (`pwmd` startup + tx API + routing tests).
- [ ] Minimum negative scenarios listed (>=2): invalid `domain_hi`, submit to inactive shard, protocol mismatch at startup.
- [ ] Replay/double-spend/invalid-proof class checks listed where applicable to shard-local tx path.
- [ ] Evidence plan defined (command outputs + compact pass/fail matrix in test report).
- [ ] Pass/fail gate defined: both shards boot cleanly and each accepts one valid local tx; any routing ambiguity is hard fail.

### Output artifacts
- `Test report`: `docs/reviews/sprint-1-test-report.md` (planned).
- `Failing cases`: Captured with reproduction commands and shard id.
- `Residual risk notes`: Perf variance under load and cross-shard behavior intentionally not closed in Sprint 1.

---

## `pwm-review` (owner: coherence and quality gate)

### Inputs to fill
- **Review scope:** Routing logic, shard bootstrap boundaries, operator/demo docs, Sprint 1 checklist closure quality.
- **Consistency baseline docs:** `docs/WHITE_SPEC_v0.md`, `docs/rfc/1-address-format.md`, `docs/rfc/6-policy-engine.md`, `docs/PHASE1_CHECKLIST.md`.
- **Critical invariants to enforce:** Strict-upgrade intact; routing only from protocol fields; no hidden UTXO pivot; shard A/B runtime independence.

### Required checklist
- [ ] Spec/code coherence reviewed for two-shard runtime scope.
- [ ] No contradiction with strict-upgrade and protocol-routing constraints.
- [ ] No hidden behavior drift from account-core compatibility baseline.
- [ ] Findings ordered by severity (`critical/major/minor`).
- [ ] Explicit verdict produced: `PASS` or `REQUEST CHANGES`.

### Output artifacts
- `Review report`: `docs/reviews/sprint-1-review-report.md` (planned).
- `Blocking findings`: Logged with file references and invariant violated.
- `Recommended corrections`: Minimal fix list for next coding pass.

---

## `orchestrator` (owner: coordination and release decision)

### Inputs to fill
- **Delegation sequence:** `pwm-coding -> pwm-testing -> pwm-review -> orchestrator decision`.
- **Decision deadlines:** Code complete by `2026-05-03`, testing by `2026-05-06`, review verdict by `2026-05-07`, sprint decision on `2026-05-08`.
- **Escalation policy:** Stop sprint gate immediately on protocol constraint violation, shard isolation break, or unresolved blocking review finding.

### Required checklist
- [ ] Sprint checklist published before implementation start.
- [ ] Delegation prompts aligned with Sprint 1 scope and constraints.
- [ ] Roadmap coherence review scheduled right after Sprint 1 closure.
- [ ] Demo gate confirms 1 happy path + 2 negative scenarios.
- [ ] Manual visual checks requested only when TUI/perf stability doubts emerge.
- [ ] Final sprint verdict captured: `ready | partial | blocked`.
- [ ] Context continuity maintained across handoffs (decisions, trade-offs, blockers, open questions).
- [ ] Delegation discipline preserved (`coding -> testing -> review -> orchestration decision`).
- [ ] Subagent drift monitored episodically (hangs, loops, workaround patchwork, scope drift).
- [ ] Drift response applied if detected: scope reset, prompt correction, explicit stop/restart note.
- [ ] Sprint closeout artifacts kept concise, traceable, and decision-oriented.

### Output artifacts
- `Sprint status note`: `docs/reviews/sprint-1-status-note.md` (planned).
- `Task/ticket updates`: Sprint board task ids updated in orchestrator log.
- `Go/No-Go decision`: Recorded as explicit `Go` or `No-Go` statement.

---

## 4) Demo Script (operator-facing)

### Preconditions
- Rust toolchain installed; workspace builds with `cargo check`.
- Operator has three terminals: shard A, shard B, client.
- Optional: clean local state by removing `state/shard-a` and `state/shard-b` before demo.

### Steps
1. Start shard A:
   `cargo run -p pwmd --bin pwmd -- --shard A --state-root state`
2. Start shard B:
   `cargo run -p pwmd --bin pwmd -- --shard B --state-root state`
3. In each node log, confirm:
   - `pwmd listening on http://127.0.0.1:3030 shard=A state_ns=shard-a` (A),
   - `pwmd listening on http://127.0.0.1:3031 shard=B state_ns=shard-b` (B),
   - `tx routing guard: shard=... sender_domain=... sender_hi=...`.
4. Submit one valid tx to each shard process (CLI/API): sender account must belong to that shard's pinned domain class; for `TRANSFER`, receiver must match sender `domain_hi` and pass Phase1 recipient policy.
5. Verify each shard accepts only its local tx and persists to its own snapshot path:
   - `state/shard-a/pwm-data.json`,
   - `state/shard-b/pwm-data.json`.

### Expected result
- Both runtimes stay up concurrently on different ports.
- Each shard process accepts only tx where sender domain class matches the pinned map for that shard (Regulatory on A, TNC on B).
- Local `TRANSFER` is accepted only when `domain_hi(sender) == domain_hi(receiver)` and recipient passes Phase1 policy; cross-domain `TRANSFER` on local path is rejected with deterministic `409` guard.
- Routing is protocol-derived from tx fields only (no route-forcing mode).

### Failure signatures to watch
- Tx accepted by wrong shard for same input payload.
- Runtime startup errors caused by shared state path or port conflicts.
- Inconsistent routing decision between validation and execution stages.

---

## 5) Risk Register (Sprint-local)

| Risk | Probability | Impact | Mitigation | Owner |
|---|---|---|---|---|
| Domain boundary routing bug (process shard vs protocol `domain_hi` compare) misroutes tx | M | H | Add boundary-value tests and log routing decision per tx | pwm-coding |
| Hidden shared state between shard A/B causes data bleed | M | H | Enforce isolated config/state dirs and startup guard checks | pwm-coding |
| Spec drift between runtime behavior and docs baseline | M | M | Mandatory pwm-review coherence pass against listed baseline docs | pwm-review |
| Manual checks skipped despite UI/perf instability signals | L | M | Orchestrator triggers manual visual check only on explicit stability doubt criteria | orchestrator |
| Sprint closes without clear go/no-go traceability | L | M | Keep compact status note with gate verdict and unresolved blockers | orchestrator |

---

## 6) End-of-Sprint Gate

- **Coding verdict:** `pass | fail`
- **Testing verdict:** `pass | fail`
- **Review verdict:** `pass | fail`
- **Orchestrator final status:** `ready_for_next_sprint | carry_over | blocked`

### Carry-over items (if any)
- Cross-shard finality and throughput/perf hardening moved to next sprint if Sprint 1 demo gate passes.

