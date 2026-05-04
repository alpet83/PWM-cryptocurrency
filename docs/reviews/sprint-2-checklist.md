# Sprint Checklist (Sprint 2)

**Sprint:** `Sprint 2 / Routing Discipline and Demo Hardening`  
**Dates:** `2026-04-24 .. 2026-05-08`  
**Scope reference:** `docs/WHITE_SPEC_v0.md`, `docs/rfc/1-address-format.md`, `docs/rfc/6-policy-engine.md`, `docs/MVP-checklist.md`  
**Status:** `ready_for_next_sprint`

---

## 0) Implementation pass update (2026-04-24)

- [x] Added deterministic automated assertions in `crates/pwmd/src/lib.rs` for recipient prefilter rejections: `reserve`, `witness`, `unknown/non-indexed`.
- [x] Locked error contract for these paths: `400 BAD_REQUEST` with stable substrings (`recipient domain` + class-specific reason).
- [x] Preserved existing guard behavior assertions: wrong process shard -> `409`, local `TRANSFER` `domain_hi` mismatch -> `409`.
- [ ] Full shard A/B runtime evidence remains delegated to `pwm-testing` gate report.

---

## 1) Sprint Goal

- **Primary objective:** Deliver the next increment after Sprint 1 by hardening routing semantics and gate discipline without protocol drift.
- **Demo-ready definition for this sprint:** Operator can show deterministic local acceptance/rejection on shard A/B with strict separation of process-shard mapping and protocol routing: process gate by `domain_hi(sender)`, `TRANSFER` local gate by `domain_hi(sender) == domain_hi(receiver)`, plus Phase1 recipient prefilter (`reserve/witness/unknown`).
- **Out of scope:** New protocol rules, heuristic routing (`0x80` split), cross-shard finality implementation, heavy performance tuning.

---

## 2) Shared Constraints (for all roles)

- [x] Keep strict-upgrade and compatibility baseline from Sprint 1.
- [x] Do not use `0x80` heuristic for shard/process or tx routing decisions.
- [x] Keep process-shard mapping and protocol routing as separate layers.
- [x] Keep recipient prefilter (`reserve/witness/unknown-domain`) as an independent pre-mempool guard.
- [x] Keep docs/spec alignment with current accepted baseline.

---

## 3) Responsibility Matrix

## `pwm-coding` (owner: implementation)

### Inputs to fill
- **Implementation scope:** Tighten/verify runtime guards and docs for deterministic routing behavior (process shard by sender domain class, protocol `TRANSFER` gate by sender/receiver `domain_hi` compare, recipient prefilter before local acceptance).
- **Design notes / assumptions:** Process shard mapping is operator/runtime partitioning and is not a protocol routing substitute; local `TRANSFER` admission stays protocol-derived; recipient prefilter blocks forbidden recipients before mempool.
- **Risky areas:** Regressions near domain boundary values, mixed handling order between prefilter and routing guard, ambiguous error signaling in negative paths.

### Required checklist
- [x] Implementation plan split into atomic steps with explicit guard order.
- [x] Changed-file list maintained in sprint notes.
- [x] Negative scenarios fixed before final handoff (`wrong process shard`, `domain_hi mismatch`, `reserve/witness/unknown recipient`, malformed domain).
- [x] Backward-compat note captured for unchanged account-core semantics.
- [x] Demo run path documented with expected pass/fail observables.

### Output artifacts
- `Code/doc changes`: affected files under `crates/pwmd/*` and `docs/reviews/*` as needed.
- `Technical notes`: concise explanation of guard order and routing invariants.
- `Demo commands`: shard A/B run and tx submit checks for happy + negative cases.

---

## `pwm-testing` (owner: verification)

### Inputs to fill
- **Test scope:** Regression/smoke validation of routing discipline and recipient prefilter ordering for Sprint 2 acceptance gate.
- **Coverage focus:** Determinism of local `TRANSFER` gate (`domain_hi(sender) == domain_hi(receiver)`), process-shard mismatch rejection, recipient prefilter rejection classes, stable behavior across shard A/B.
- **Known gaps to monitor:** No heavy perf soak in this sprint; no cross-shard finality closure.

### Required checklist
- [x] Regression subset defined for shard startup + `/v1/tx` guards.
- [x] Minimum negative scenarios listed and executed (>=4): process-shard mismatch, `TRANSFER` sender/receiver `domain_hi` mismatch, reserve/witness recipient, unknown-domain recipient.
- [x] Error contract checked for deterministic status codes/messages in negative flows (matrix below used as gate contract).
- [x] Evidence plan defined (compact pass/fail matrix + repro commands).
- [x] Pass/fail gate defined: all guard classes deterministic on both shards; any ambiguity is hard fail.

### Kickoff expected error contract (gate baseline)

| Scenario | Expected HTTP status | Expected message contract (substring) |
|---|---|---|
| sender on wrong process shard | `409 CONFLICT` | `tx belongs to process shard` |
| `TRANSFER` with `domain_hi(sender) != domain_hi(receiver)` | `409 CONFLICT` | `cross-domain transfer is disabled` |
| recipient in reserve/witness class | `400 BAD_REQUEST` | `recipient` + policy rejection text |
| recipient in unknown/non-indexed domain | `400 BAD_REQUEST` | `unknown` or policy rejection text |
| malformed domain/signature shape | `400 BAD_REQUEST` | `tx validation failed` / `domain mismatch` |

Note: first two rows already have lightweight automated assertions in `crates/pwmd/src/lib.rs`; recipient prefilter rows are mandatory Sprint 2 execution checks even if still covered by smoke/manual API repro in kickoff phase.

### Output artifacts
- `Test report`: `docs/reviews/sprint-2-test-report.md` (planned).
- `Failing cases`: with minimal reproduction and shard id.
- `Residual risk notes`: explicit carry-over for perf/cross-shard topics.

---

## `pwm-review` (owner: coherence and quality gate)

### Inputs to fill
- **Review scope:** Spec/code/doc coherence for Sprint 2 routing discipline and gate order.
- **Consistency baseline docs:** `docs/WHITE_SPEC_v0.md`, `docs/rfc/1-address-format.md`, `docs/rfc/6-policy-engine.md`, `docs/MVP-checklist.md`.
- **Critical invariants to enforce:** No `0x80` heuristic reintroduction; process-shard vs protocol routing separation preserved; recipient prefilter remains separate guard layer; strict-upgrade unchanged.

### Required checklist
- [x] Spec/code coherence reviewed for Sprint 2 scope.
- [x] No contradiction with agreed routing invariants and recipient policy.
- [x] No behavior drift in account-core compatibility baseline.
- [x] Findings ordered by severity (`critical/major/minor`).
- [x] Explicit verdict produced: `PASS` or `REQUEST CHANGES`.

### Output artifacts
- `Review report`: `docs/reviews/sprint-2-review-report.md` (planned).
- `Blocking findings`: file references + violated invariant.
- `Recommended corrections`: minimal fix list for next coding pass.

---

## `orchestrator` (owner: coordination and release decision)

### Inputs to fill
- **Delegation sequence (strict):** `pwm-coding -> pwm-testing -> pwm-review -> orchestrator decision`.
- **Decision deadlines:** coding completion, then testing, then review, then final decision in the same gate order.
- **Escalation policy:** immediate stop on protocol contradiction, non-deterministic guard behavior, or unresolved blocking review finding.

### Required checklist
- [x] Sprint checklist published before coding starts.
- [x] Delegation prompts aligned with Sprint 2 scope and invariants.
- [x] Gate order enforced strictly: coding -> testing -> review -> decision.
- [x] Demo gate confirms happy path and required negative scenarios.
- [x] Final sprint verdict captured: `ready | partial | blocked`.
- [x] Context continuity preserved across handoffs (decisions, blockers, open questions).
- [x] Sprint closeout artifacts remain concise and decision-oriented.

### Output artifacts
- `Sprint status note`: `docs/reviews/sprint-2-status-note.md`.
- `Task/ticket updates`: `tasks/20260424-sprint2-orchestrated.json`.
- `Go/No-Go decision`: explicit orchestrator verdict after all gates.

---

## 4) Demo Script (operator-facing)

### Preconditions
- Workspace passes quick build sanity (outside this kickoff artifact step).
- Operator has three terminals: shard A, shard B, client.
- Optional: clean `state/shard-a` and `state/shard-b` for reproducible run.

### Steps
1. Start shard A:
   `cargo run -p pwmd --bin pwmd -- --shard A --state-root state`
2. Start shard B:
   `cargo run -p pwmd --bin pwmd -- --shard B --state-root state`
3. Confirm startup logs show distinct port/state namespace per shard.
4. Submit one valid local tx per shard (sender belongs to shard class; for `TRANSFER` receiver matches sender `domain_hi`; recipient passes Phase1 prefilter).
5. Run negative submits:
   - sender on wrong process shard,
   - `TRANSFER` with sender/receiver `domain_hi` mismatch,
   - recipient in reserve/witness class,
   - recipient with unknown/invalid domain mapping.

### Expected result
- Both runtimes stay up concurrently and isolated.
- Process-shard guard is enforced by sender domain class only.
- Local `TRANSFER` acceptance depends on `domain_hi(sender) == domain_hi(receiver)`.
- Recipient prefilter rejects reserve/witness/unknown recipients before local acceptance path.
- No heuristic `0x80` logic appears in routing decisions.

---

## 5) Risk Register (Sprint-local)

| Risk | Probability | Impact | Mitigation | Owner |
|---|---|---|---|---|
| Guard-order drift (prefilter vs routing vs mempool) creates inconsistent results | M | H | Fix explicit guard order and verify with deterministic negatives | pwm-coding |
| Hidden heuristic logic (`0x80`-style) reintroduced by shortcut patch | M | H | Review for explicit `domain_hi`-based checks only | pwm-review |
| Non-deterministic error contracts across shards | M | M | Testing matrix must validate status/message consistency | pwm-testing |
| Sprint gate sequence bypassed under schedule pressure | L | H | Orchestrator enforces strict gate order before decision | orchestrator |

---

## 6) End-of-Sprint Gate

- **Coding verdict:** `pass`
- **Testing verdict:** `pass`
- **Review verdict:** `pass`
- **Orchestrator final status:** `ready_for_next_sprint`

### Carry-over items (if any)
- Perf/load hardening and cross-shard finality remain outside Sprint 2 scope unless explicitly reprioritized.
