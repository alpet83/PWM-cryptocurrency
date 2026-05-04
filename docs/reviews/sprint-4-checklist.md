# Sprint Checklist (Sprint 4)

**Sprint:** `Sprint 4 / Shard Runtime Identity and Native-Peer Priority`  
**Dates:** `2026-04-24 .. 2026-05-08`  
**Scope reference:** `docs/WHITE_SPEC_v0.md`, `docs/rfc/6-policy-engine.md`, `docs/rfc/8-shard-runtime-identity-and-peering.md`, `docs/MVP-checklist.md`  
**Status:** `partial_ready_for_implementation`

---

## 1) Sprint Goal

- **Primary objective:** Formalize and prepare implementation-ready architecture for cluster-bound shard runtime identity, node self-identification in network handshakes, and native-first peering priority.
- **Demo-ready definition for this sprint:** Team has coherent RFC + sprint artifacts that define launch parameterization, identity verification, peer classes (`native/foreign`), and deterministic priority policy without protocol drift.
- **Out of scope:** Runtime code implementation, protocol transaction format changes, range-based shard heuristics (including `0x80 split`), and finality internals.

---

## 2) Shared Constraints (for all roles)

- [x] Keep `spec-level geo-shard` semantics fixed by `domain_hi` cluster identity.
- [x] Keep process/runtime labels (`Shard A/B`) as operational aliases only, not protocol truth.
- [x] Do not introduce `domain_hi` range heuristics for routing/identity/policy.
- [x] Keep compatibility with current `WHITE_SPEC` and RFC baseline.
- [x] Keep strict gate order: `coding -> testing -> review -> orchestrator decision`.

---

## 3) Responsibility Matrix

## `pwm-coding` (owner: specification package)

### Inputs to fill
- **Implementation scope:** Produce RFC for shard runtime identity and peering policy, plus sprint-4 kickoff artifacts and link sync in baseline docs.
- **Design notes / assumptions:** Runtime identity is configuration-bound (`network_id`, `cluster_identity`, `node_identity`, capabilities); peer class and priority are derived from declared/signed identity, not from endpoint naming or address ranges.
- **Risky areas:** Contradiction with existing shard semantics in `WHITE_SPEC`/RFC-6, ambiguous migration from `--shard A|B`, underdefined anti-spoof checks.

### Required checklist
- [x] RFC includes glossary (`spec-level geo-shard`, runtime shard instance, domain cluster, native/foreign shard).
- [x] Launch parameterization is explicit and implementation-oriented.
- [x] Node self-ID envelope in handshake is defined with anti-spoof constraints.
- [x] Peer priority policy covers queueing, reconnect/backoff, gossip weight, failover.
- [x] Observability baseline (metrics/logs) is documented.
- [x] Migration path from `--shard A|B` to cluster-bound runtime config is captured.
- [x] Acceptance criteria for next implementation stage are explicit and testable.

### Output artifacts
- `docs/rfc/8-shard-runtime-identity-and-peering.md`
- `docs/reviews/sprint-4-checklist.md`
- `docs/reviews/sprint-4-status-note.md`
- `tasks/20260424-sprint4-orchestrated.json` (coding delegation completion update)

---

## `pwm-testing` (owner: validation matrix)

### Inputs to fill
- **Test scope:** Validate implementation readiness of RFC acceptance criteria and produce concrete verification matrix for future runtime changes.
- **Coverage focus:** Handshake identity checks, peer class determinism, native-first priority behavior, degraded/failover semantics, migration compatibility path.
- **Known gaps to monitor:** No runtime code exists yet for direct execution proof in this kickoff pass.

### Required checklist
- [x] Validation matrix defined for each RFC acceptance criterion.
- [x] Negative scenarios listed (>=5): spoofed identity, replayed nonce, network/genesis mismatch, forged native claim, priority regression under native deficit.
- [x] Required observability signals mapped to pass/fail assertions.
- [x] Compatibility checks for `--shard A|B` alias migration path included.
- [x] Hard fail gate defined for any heuristic/range-based shard classification logic.

### Output artifacts
- `docs/reviews/sprint-4-test-report.md` (planned)
- `Validation matrix` with deterministic pass/fail clauses per criterion
- `Residual risks` with explicit handoff notes for implementation sprint

---

## `pwm-review` (owner: coherence and quality gate)

### Inputs to fill
- **Review scope:** Independent coherence review of new RFC and sprint-4 artifacts.
- **Consistency baseline docs:** `docs/WHITE_SPEC_v0.md`, `docs/rfc/6-policy-engine.md`, `docs/rfc/8-shard-runtime-identity-and-peering.md`, `docs/MVP-checklist.md`.
- **Critical invariants to enforce:** No protocol drift, no reintroduction of range heuristics, deterministic native/foreign semantics by fixed `domain_hi` identity.

### Required checklist
- [x] Findings structured by severity (`critical/major/minor`).
- [x] No contradiction between RFC-8 and `WHITE_SPEC`/RFC-6 semantics.
- [x] Migration path does not weaken existing guarantees.
- [x] Observability and security requirements are actionable (not purely aspirational).
- [x] Explicit verdict produced: `PASS` or `REQUEST CHANGES`.

### Output artifacts
- `docs/reviews/sprint-4-review-report.md` (planned)
- `Blocking findings` with invariant references
- `Minimal correction list` for follow-up coding pass

---

## `orchestrator` (owner: gate discipline and decision)

### Inputs to fill
- **Delegation sequence (strict):** `pwm-coding -> pwm-testing -> pwm-review -> orchestrator decision`.
- **Decision cadence:** coding completion first, testing matrix second, review verdict third, final decision last.
- **Escalation policy:** stop gate on any contradiction with `WHITE_SPEC`/RFC-6 or any heuristic shard semantics.

### Required checklist
- [x] Sprint-4 kickoff checklist published before testing/review pass.
- [x] Scope and invariants aligned across delegations.
- [x] Strict order enforced: coding -> testing -> review -> decision.
- [x] Final gate status recorded as `ready_for_implementation | partial | blocked`.
- [x] Follow-up tasks for implementation sprint created from validated acceptance criteria.

### Output artifacts
- `docs/reviews/sprint-4-status-note.md`
- `tasks/20260424-sprint4-orchestrated.json`
- explicit orchestrator decision note after all gates

---

## 4) Sprint Risks (kickoff)

| Risk | Probability | Impact | Mitigation | Owner |
|---|---|---|---|---|
| RFC ambiguity in identity envelope leads to inconsistent implementation | M | H | Keep required fields and acceptance gates explicit | pwm-coding |
| Native/foreign priority policy remains non-testable | M | H | pwm-testing defines per-criterion matrix with measurable outcomes | pwm-testing |
| Drift from baseline shard semantics in white spec/policy RFC | L | H | pwm-review performs invariant-focused coherence check | pwm-review |
| Gate order bypass under schedule pressure | L | H | orchestrator enforces strict sequence and blocks early decision | orchestrator |

---

## 5) End-of-Sprint Gate

- **Coding verdict:** `pass`
- **Testing verdict:** `partial`
- **Review verdict:** `pass`
- **Orchestrator final status:** `partial`

### Carry-over items (if any)
- Runtime implementation tasks are intentionally deferred to the next sprint after testing/review close acceptance matrix and coherence verdict.

