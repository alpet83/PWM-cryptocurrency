# Sprint Checklist (Sprint 3)

**Sprint:** `Sprint 3 / Evidence Hardening and Gate Formalization`  
**Dates:** `2026-05-09 .. 2026-05-22`  
**Scope reference:** `docs/WHITE_SPEC_v0.md`, `docs/rfc/1-address-format.md`, `docs/rfc/6-policy-engine.md`, `docs/MVP-checklist.md`  
**Status:** `ready_for_next_sprint`

---

## 1) Sprint Goal

- **Primary objective:** Deliver the next increment after Sprint 2 by hardening evidence quality and gate formalization for already accepted routing/policy behavior.
- **Demo-ready definition for this sprint:** Operator can reproduce a compact, deterministic acceptance/rejection matrix (happy + negative) for shard A/B with stable error contract and explicit gate artifacts suitable for release decision.
- **Out of scope:** New transaction types, new routing algorithms, heuristic `0x80` split, cross-shard finality redesign, protocol-level policy expansion.

---

## 2) Shared Constraints (for all roles)

- [x] Preserve Sprint 2 routing invariants and recipient prefilter semantics.
- [x] Preserve strict separation: process-shard mapping vs protocol routing decision path.
- [x] Keep recipient policy baseline (`reserve/witness/unknown` reject in regular flow).
- [x] Keep strict-upgrade compatibility and no protocol drift vs baseline specs.
- [x] Do not introduce behavior that conflicts with `WHITE_SPEC` + RFC baseline.

---

## 3) Responsibility Matrix

## `pwm-coding` (owner: implementation)

### Inputs to fill
- **Implementation scope:** Documentation/runtime-surface hardening for deterministic gate evidence after Sprint 2 closure: align artifacts, tighten operator-facing scenario definitions, and remove ambiguity in acceptance/negative classifications.
- **Design notes / assumptions:** Protocol rules are unchanged from Sprint 2; this sprint improves reproducibility and decision quality, not protocol semantics.
- **Risky areas:** Ambiguous scenario phrasing, mismatch between expected status/message contracts and actual behavior, accidental scope creep into protocol changes.

### Required checklist
- [x] Kickoff docs for Sprint 3 published and internally consistent.
- [x] Acceptance criteria mapped to concrete happy/negative scenario groups.
- [x] Negative scenarios listed with deterministic expected outcome contracts.
- [x] Risk register updated with owners and mitigations.
- [x] Handoff package to `pwm-testing` contains reproducible scenario matrix.
- [x] Shard semantics ambiguity hardening completed in baseline docs (`WHITE_SPEC`, RFC policy, `pwmd` ops doc) with explicit anti-`0x80 split` note.

### Output artifacts
- `Kickoff docs`: `docs/reviews/sprint-3-checklist.md`, `docs/reviews/sprint-3-status-note.md`.
- `Task update`: `tasks/20260424-sprint3-orchestrated.json` delegation completion note.
- `Technical notes`: concise rationale for Sprint 3 scope boundaries.
- `Implementation pass addendum`: shard semantics clarification and islandization wording aligned across spec/ops docs.

---

## `pwm-testing` (owner: verification)

### Inputs to fill
- **Test scope:** Execute Sprint 3 acceptance/negative matrix against current behavior without redefining protocol rules.
- **Coverage focus:** Deterministic status/message contracts for process-shard gate, local `TRANSFER` same-domain rule, recipient prefilter classes, malformed input handling.
- **Known gaps to monitor:** No long-running perf soak and no cross-shard finality closure in this sprint.

### Required checklist
- [x] Acceptance scenarios executed with pass/fail evidence.
- [x] Negative scenarios executed (minimum set from section 5).
- [x] Status/message contract stability checked across shard A/B.
- [x] Non-deterministic or flaky outcomes escalated as blocking for gate.
- [x] Test report produced with concise repro commands and verdict.

### Output artifacts
- `Test report`: `docs/reviews/sprint-3-test-report.md` (planned).
- `Failure list`: minimal reproducible failing cases, if any.
- `Residual risks`: explicit carry-over items for next sprint.

---

## `pwm-review` (owner: coherence and quality gate)

### Inputs to fill
- **Review scope:** Coherence review of Sprint 3 kickoff and test evidence vs baseline specs and Sprint 2 accepted invariants.
- **Consistency baseline docs:** `docs/WHITE_SPEC_v0.md`, `docs/rfc/1-address-format.md`, `docs/rfc/6-policy-engine.md`, `docs/MVP-checklist.md`.
- **Critical invariants to enforce:** No routing-policy contradiction, no heuristic reintroduction, no silent semantics drift.

### Required checklist
- [x] Artifact coherence checked across checklist/status/test report/task note.
- [x] Acceptance and negative matrices validated against baseline constraints.
- [x] Findings ordered by severity (`critical/major/minor`).
- [x] Explicit verdict produced: `PASS` or `REQUEST CHANGES`.
- [x] Blocking contradictions called out with minimal correction path.

### Output artifacts
- `Review report`: `docs/reviews/sprint-3-review-report.md` (planned).
- `Blocking findings`: concrete invariant violations with file references.
- `Correction plan`: minimal next actions before orchestrator decision.

---

## `orchestrator` (owner: coordination and release decision)

### Inputs to fill
- **Delegation sequence (strict):** `pwm-coding -> pwm-testing -> pwm-review -> orchestrator decision`.
- **Decision policy:** No decision before all three prior gates return explicit verdicts.
- **Escalation policy:** Immediate stop on spec contradiction, non-deterministic evidence, or unresolved blocking review finding.

### Required checklist
- [x] Kickoff artifacts published before testing delegation.
- [x] Gate order enforced strictly with no bypass.
- [x] Delegation mini-reports captured after each role.
- [x] Final decision recorded as `ready | partial | blocked`.
- [x] Carry-over risks and ownership recorded in closeout note.

### Output artifacts
- `Sprint status note`: `docs/reviews/sprint-3-status-note.md`.
- `Task/ticket updates`: `tasks/20260424-sprint3-orchestrated.json`.
- `Final decision note`: orchestrator verdict after strict gate sequence.

---

## 4) Acceptance Criteria (Sprint 3 Gate Baseline)

1. Sprint 3 kickoff artifacts exist and are coherent with baseline docs.
2. Routing/policy invariants from Sprint 2 remain unchanged and explicitly restated.
3. Acceptance + negative scenario matrix is complete, deterministic, and reproducible.
4. Strict gate order is documented and followed (`coding -> testing -> review -> orchestrator decision`).
5. Final sprint decision can be made from concise artifacts without hidden assumptions.

---

## 5) Negative Scenario Matrix (must remain deterministic)

| Scenario | Expected HTTP status | Expected message contract (substring) |
|---|---|---|
| sender on wrong process shard | `409 CONFLICT` | `tx belongs to process shard` |
| `TRANSFER` with `domain_hi(sender) != domain_hi(receiver)` | `409 CONFLICT` | `cross-domain transfer is disabled` |
| recipient in reserve/witness class | `400 BAD_REQUEST` | `recipient domain` + (`reserve` \| `witness-only`) |
| recipient in unknown/non-indexed domain | `400 BAD_REQUEST` | `recipient domain` + `not recognized` |
| malformed tx/domain/signature shape | `400 BAD_REQUEST` | `tx validation failed` |

---

## 6) Risk Register (Sprint-local)

| Risk | Probability | Impact | Mitigation | Owner |
|---|---|---|---|---|
| Artifact drift between checklist/status/task notes causes gate ambiguity | M | M | Keep single-source wording for invariants and gate order across all Sprint 3 docs | pwm-coding |
| Non-deterministic error text/status across shards breaks evidence confidence | M | H | `pwm-testing` validates stable contracts on both shard paths before pass verdict | pwm-testing |
| Scope creep into protocol redesign under "hardening" label | M | H | Reject protocol-expanding changes in review; keep Sprint 3 constrained to evidence/gate hardening | pwm-review |
| Gate order bypassed due to schedule pressure | L | H | Orchestrator blocks final decision until all prior gates explicitly pass | orchestrator |

---

## 7) End-of-Sprint Gate

- **Coding verdict:** `pass`
- **Testing verdict:** `pass`
- **Review verdict:** `pass`
- **Orchestrator final status:** `ready_for_next_sprint`

### Carry-over items (if any)
- Perf/load hardening and cross-shard finality remain outside Sprint 3 scope unless explicitly reprioritized.
