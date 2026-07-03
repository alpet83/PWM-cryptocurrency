# Polish Report Recheck: MVP v7 Plan + Sprint 1

**Date:** 2026-06-22
**Reviewer:** pwm-polish
**Artifacts:**
- `docs/plans/mvp_v7.md`
- `docs/plans/mvp_v7s1.md`
**Original report:** `docs/reviews/polish-report-mvp_v7.md`
**Scope:** Re-verify 3 BLOCKERs and 5 WARNs from original report only.

---

## Verification Table

| # | Finding (original) | Status | Reason |
|---|--------------------|--------|--------|
| B1 | [BLOCKER] Sprint 1 acceptance criterion "существенный рост throughput" is unverifiable | **RESOLVED** | `mvp_v7.md` Sprint 1 Acceptance now references "Sustained ≥ 50 tx/s за ≥ 60 секунд... — полный критерий в `mvp_v7s1.md`"; `mvp_v7s1.md` § Критерий приёмки спринта contains a concrete, boolean-checkable criterion with exact command, numeric gate, duration, and zero-error requirement. Baseline commit to `docs/reviews/v7-s1-slice0-baseline-profile.md` is also mandated. |
| B2 | [BLOCKER] Sprint Brief (`mvp_v7s1.md`) missing Scope Gate OUT | **RESOLVED** | `mvp_v7s1.md` now contains an explicit "## Scope Gate OUT (Sprint 1)" section with 8 enumerated exclusions matching all items required by the original fix (sharded apply_tx, tokio replacement, account-cache, BFT code, wire/snapshot changes, cross-shard dispatch, GPU/SIMD, /v2/* API). |
| B3 | [BLOCKER] All Slice Plans (Slice 0–4) missing pre-condition and post-condition | **RESOLVED** | All five slices (0–4) in `mvp_v7s1.md` now have explicit Pre-condition, Post-condition, and Rollback fields. Each pre-condition is boolean-checkable (`cargo test` pass + prior slice post-condition verified); each post-condition is deterministically verifiable with specific artifacts and commands. |
| W1 | [WARN] Sprint Brief missing Known Risks section | **RESOLVED** | `mvp_v7s1.md` now contains "## Известные риски (Sprint 1)" — a 5-row table covering I/O-bound bottleneck risk, deadlock/livelock, non-determinism under affinity-worker racing, tokio runtime conflict, and multi-node degradation. All four items suggested in the original fix are covered. |
| W2 | [WARN] Sprint 1 acceptance criterion deferred to an unconfirmed ticket | **RESOLVED** | The deferral sentence "Детальные критерии и тесты будут доработаны в тикете спринта" is no longer present; the full criterion is inlined directly in `mvp_v7s1.md`. |
| W3 | [WARN] Sprints V7-1, V7-2, V7-3 lack pre-conditions for their slices | **STILL OPEN** | `mvp_v7.md` Sprints V7-1/2/3 decomposition sections remain unchanged — no slice-level pre/post-conditions added. This was noted as a future-sprint gap; no fix applied. |
| W4 | [WARN] V7-3 pre-condition "ADR 0012 accepted" is de-facto satisfied but not stated | **RESOLVED** | `mvp_v7.md` Sprint V7-3 Предусловие now reads: "ADR 0012 accepted (уже выполнено — статус «Accepted as V7 normative contract», см. `docs/adr/0012-emergency-stake-evacuation.md`); V6-7 emergency activation." — the annotation requested by the original fix is present. |
| W5 | [WARN] Mermaid diagram shows Worker Pool feeding Orchestrator — architectural ambiguity | **PARTIAL** | A prose note was added directly after the diagram explicitly clarifying that "Orchestrator is the main thread owning both dispatcher and seal; workers are subordinate, not upstream." The arrow direction in the diagram itself (`G[Пул воркеров] → I[Оркестратор] → J[Seal]`) is unchanged, so a naive diagram reader could still misread data flow, but the immediately adjacent note provides adequate mitigation for a coding worker reading the full section. |

---

## Updated Overall Result

**PARTIAL**

- 3 BLOCKERs: all resolved.
- 5 WARNs: 4 resolved, 1 still open (W3 — future-sprint slice pre/post-conditions), 1 partial (W5 — diagram ambiguity mitigated by prose note but diagram unchanged).

No BLOCKERs remain. Coding on Sprint 1 (V7-S1) may proceed. W3 must be addressed before each of V7-1/2/3 opens. W5 is low risk given the prose clarification but the diagram should be corrected at next edit opportunity.

---

## Corrected artifact

No corrected artifact — remaining findings do not require human decisions on Sprint 1; W3 is a future-sprint action item and W5 is adequately mitigated for current coding purposes.
