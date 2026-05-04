# Sprint Checklist Template (v1 Testnet)

**Sprint:** `<Sprint N / Name>`  
**Dates:** `<YYYY-MM-DD .. YYYY-MM-DD>`  
**Scope reference:** `<spec/docs/tasks links>`  
**Status:** `draft | in_progress | complete`

---

## 1) Sprint Goal

- **Primary objective:** `<one clear statement>`
- **Demo-ready definition for this sprint:** `<what must be runnable/visible>`
- **Out of scope:** `<explicit exclusions>`

---

## 2) Shared Constraints (for all roles)

- [ ] Keep strict-upgrade from v0 (no hidden core rewrite).
- [ ] Keep protocol-derived routing (`domain_hi(sender)` vs `domain_hi(receiver)`).
- [ ] Preserve baseline compatibility on local same-shard flow.
- [ ] Keep docs/spec alignment with current agreed baseline.

---

## 3) Responsibility Matrix

## `pwm-coding` (owner: implementation)

### Inputs to fill
- **Implementation scope:** `<modules/files/components>`
- **Design notes / assumptions:** `<short bullets>`
- **Risky areas:** `<state transitions, edge cases, perf hotspots>`

### Required checklist
- [ ] Implementation plan broken into 3-7 atomic steps.
- [ ] Explicit list of changed files prepared.
- [ ] Negative cases identified before coding starts.
- [ ] Backward-compat notes captured (what remains unchanged).
- [ ] Demo run path documented (commands and expected output).

### Output artifacts
- `Code changes`: `<paths>`
- `Technical notes`: `<link or section>`
- `Demo commands`: `<commands>`

---

## `pwm-testing` (owner: verification)

### Inputs to fill
- **Test scope:** `<unit/integration/smoke/manual>`
- **Coverage focus:** `<critical paths + invariants>`
- **Known gaps to monitor:** `<if any>`

### Required checklist
- [ ] Regression suite selection defined.
- [ ] Minimum negative scenarios listed (>=2).
- [ ] Replay / double-spend / invalid-proof class checks (if applicable) listed.
- [ ] Test execution evidence plan defined (logs/results summary).
- [ ] Pass/fail gates and stop criteria defined.

### Output artifacts
- `Test report`: `<path or summary>`
- `Failing cases`: `<if any>`
- `Residual risk notes`: `<bullets>`

---

## `pwm-review` (owner: coherence and quality gate)

### Inputs to fill
- **Review scope:** `<docs/code/architecture subset>`
- **Consistency baseline docs:** `<WHITE_SPEC / RFC / Matrix links>`
- **Critical invariants to enforce:** `<bullets>`

### Required checklist
- [ ] Spec/code coherence reviewed for sprint scope.
- [ ] No contradiction with baseline constraints.
- [ ] No hidden behavior drift in agreed protocol rules.
- [ ] Findings ordered by severity (critical/major/minor).
- [ ] Clear verdict: `PASS` or `REQUEST CHANGES`.

### Output artifacts
- `Review report`: `<path or summary>`
- `Blocking findings`: `<if any>`
- `Recommended corrections`: `<minimal list>`

---

## `orchestrator` (owner: coordination and release decision)

### Inputs to fill
- **Delegation sequence:** `<coding -> testing -> review>`
- **Decision deadlines:** `<dates/timeboxes>`
- **Escalation policy:** `<when to stop and discuss>`

### Required checklist
- [ ] Sprint checklist published before implementation start.
- [ ] Delegation prompts aligned with current sprint scope.
- [ ] After Sprint 1: roadmap coherence review scheduled and completed.
- [ ] Demo gate confirmed (1 happy path + 2 negative scenarios).
- [ ] Manual visual check requested only when stability/perf doubts exist (especially TUI).
- [ ] Final sprint verdict captured: `ready | partial | blocked`.
- [ ] Post-sprint optimization audit requested from `pwm-optimus` (only after accepted closeout snapshot).
- [ ] `pwm-optimus` report linked in artifacts with actionable, non-blocking optimization backlog.
- [ ] Context continuity maintained (decisions, trade-offs, open questions, blockers).
- [ ] Delegation discipline preserved (`coding -> testing -> review -> orchestration decision`).
- [ ] Subagent drift monitored episodically (hangs/loops/patchwork workarounds).
- [ ] If drift detected: scope reset + prompt correction + explicit stop/restart note.
- [ ] Summary artifacts kept concise and traceable for sprint closeout.

### Output artifacts
- `Sprint status note`: `<summary>`
- `Task/ticket updates`: `<ids/links>`
- `Go/No-Go decision`: `<statement>`

---

## 4) Demo Script (operator-facing)

### Preconditions
- `<environment, config, required processes>`

### Steps
1. `<step>`
2. `<step>`
3. `<step>`

### Expected result
- `<observable success criteria>`

### Failure signatures to watch
- `<errors/timeouts/inconsistencies>`

---

## 5) Risk Register (Sprint-local)

| Risk | Probability | Impact | Mitigation | Owner |
|---|---|---|---|---|
| `<risk>` | `L/M/H` | `L/M/H` | `<action>` | `<role>` |

---

## 6) End-of-Sprint Gate

- **Coding verdict:** `pass | fail`
- **Testing verdict:** `pass | fail`
- **Review verdict:** `pass | fail`
- **Orchestrator final status:** `ready_for_next_sprint | carry_over | blocked`

### Carry-over items (if any)
- `<item + reason + target sprint>`

