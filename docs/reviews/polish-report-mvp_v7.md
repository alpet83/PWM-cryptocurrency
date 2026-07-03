# Polish Report: MVP v7 Plan + Sprint 1

**Date:** 2026-06-22
**Reviewer:** pwm-polish
**Artifacts:**
- `docs/plans/mvp_v7.md` (Version Spec / orchestrator plan for V7)
- `docs/plans/mvp_v7s1.md` (Sprint Brief + Slice Plans for V7-S1 Perf)
**Result:** FAIL

---

## Summary

Reviewed the V7 version spec (`mvp_v7.md`) and the Sprint 1 performance plan (`mvp_v7s1.md`). The version spec is well-structured, scoped, and internally consistent against its referenced ADR 0012. It covers architectural decisions, risk table, inter-sprint gates, and a clear demo-ready result. However, it has one blocker: the Sprint 1 acceptance criterion for throughput ("рабочий прототип даёт существенный рост throughput") is not deterministically verifiable — it uses qualitative language without a concrete numeric gate tied to a specific test command.

The sprint brief (`mvp_v7s1.md`) has two blockers: (1) missing formal Scope Gate OUT section — the document describes what is IN but never explicitly enumerates what is excluded from Sprint 1, which opens scope creep risk for the coding worker; (2) slice pre-conditions and post-conditions are absent for all five defined slices (Slice 0–4), which is a structural `[BLOCKER]` by persona policy. Rollback is also absent from all slices (`[WARN]`). Known risks are absent from the sprint brief (`[WARN]`).

Total: 3 BLOCKERs, 5 WARNs, 4 SUGGESTs.

---

## Findings

### [BLOCKER] Sprint 1 acceptance criterion "существенный рост throughput" is unverifiable

**Location:** `mvp_v7.md` § Sprint 1 (V7-S1) → Acceptance; `mvp_v7s1.md` § Цели и приёмка (высокий уровень)

**Issue:** The acceptance criterion reads "Рабочий прототип даёт существенный рост throughput без потери детерминизма." "Существенный рост" (significant growth) is not a boolean-checkable condition. The overall V7 goal targets ≥50 tx/s, and the demo-ready result section also states ≥50 tx/s, but the Sprint 1 acceptance section neither repeats this number nor references the specific harness command that produces the measurement. A coding worker completing Sprint 1 cannot determine whether to close the sprint without a concrete, reproducible pass/fail number tied to a specific command.

`mvp_v7s1.md` weakens this further with "Детальные критерии и тесты будут доработаны в тикете спринта" — deferring the acceptance criterion to a ticket that may not exist at coding start.

**Required fix:** Replace the qualitative criterion with a concrete, verifiable one, for example:
> "`python scripts/cy_cluster_transfer_ramp_soak.py` with the standard ramp profile on a single node yields sustained ≥ 50 tx/s for ≥ 60 seconds with zero seal determinism errors. Baseline measurement (before any changes) must be recorded and committed to `docs/reviews/` as part of Slice 0 closure."

Both `mvp_v7.md` and `mvp_v7s1.md` must agree on this criterion. The ≥50 tx/s number is already present in the V7 demo-ready section — it must be propagated to the Sprint 1 acceptance block explicitly.

---

### [BLOCKER] Sprint Brief (`mvp_v7s1.md`) missing Scope Gate OUT

**Location:** `mvp_v7s1.md` — entire document

**Issue:** The canonical Sprint Brief format requires an explicit "Scope gate OUT" section — an enumerated list of what is excluded from this sprint. `mvp_v7s1.md` describes what is included (Слайсы 0–4, pipeline, worker pool, DoS limits, ramp soak) but never explicitly lists exclusions. Without this, the coding worker faces an open boundary: any adjacent parallelism work (e.g., sharded `apply_tx`, tokio async runtime switch, CometBFT prototype, account-cache miss handling) could be argued as in-scope.

The parent `mvp_v7.md` has a scope/out-of-scope table for V7 as a whole, but its "out of scope" items are version-level (sharded execution, full BFT replacement) — not sprint-level. A coding worker cannot derive sprint-level OUT from version-level OUT without interpretation.

**Required fix:** Add a "## Scope Gate OUT (Sprint 1)" section to `mvp_v7s1.md` with an explicit enumerated list. Minimum expected items:
- Sharded `apply_tx` (explicit ADR required, deferred)
- Tokio async runtime replacement
- Account-state cache / cache-miss optimization
- CometBFT or custom BFT code (ADR-only in V7)
- Any wire protocol changes
- Cross-shard dispatch changes
- GPU/SIMD-level sig verification optimization

---

### [BLOCKER] All Slice Plans (Slice 0–4) missing pre-condition and post-condition

**Location:** `mvp_v7s1.md` § Декомпозиция (ориентир), Slice 0 through Slice 4

**Issue:** The persona policy states: "A well-formed Slice Plan must contain pre-condition, post-condition, and rollback for every slice. The first two are `[BLOCKER]` if missing." All five slices in `mvp_v7s1.md` are listed as single-line descriptions with no pre-condition, no post-condition, and no rollback. The word "(ориентир)" signals they are intentionally rough, but the document is the live sprint brief (not a draft to be filled later) given that coding is about to start (V6 closed 2026-06-17, V7 now in planning).

- Slice 0 (Диагностика): no pre-condition (e.g., which `cargo test` baseline must pass), no post-condition (e.g., profiling report committed, bottleneck identified and documented).
- Slice 1 (Модель очередей): no pre-condition (Slice 0 complete?), no post-condition (which compile targets, which unit tests pass).
- Slice 2 (Пул воркеров): same.
- Slice 3 (Интеграция с оркестратором и seal): same.
- Slice 4 (Тесты, соаки, документация): no post-condition tying back to the ≥50 tx/s gate.

**Required fix:** For each slice, add three fields before coding begins:
- **Pre-condition:** boolean-checkable state that must be true (e.g., "`cargo test -p pwm-core` passes; Slice N-1 post-condition verified").
- **Post-condition:** deterministically verifiable outcome (e.g., "profiling report in `docs/reviews/v7-s1-baseline-profile.md`; bottleneck identified as [specific function]").
- **Rollback:** which files/branches to revert; whether state is recoverable without data loss.

---

### [WARN] Sprint Brief (`mvp_v7s1.md`) missing Known Risks section

**Location:** `mvp_v7s1.md` — entire document

**Issue:** The canonical Sprint Brief requires a "Known risks" section. Its absence is `[WARN]` by persona policy. The risks table in `mvp_v7.md` covers version-level risks, but does not surface sprint-specific technical risks for V7-S1, such as: deadlock risk in the new lock-free queue design, risk that profiling reveals the bottleneck is in I/O rather than CPU (making the pipeline approach insufficient), risk of non-determinism in worker pool ordering under high load, or risk that the tokio runtime interacts unexpectedly with OS-thread worker pools.

**Suggested fix:** Add a "## Known Risks" section to `mvp_v7s1.md` with at minimum:
- Deadlock / livelock in the lock-free queue implementation under adversarial load.
- Root cause of ~3 tx/s may be I/O-bound (disk/snapshot), not CPU — pipeline approach may not yield ≥50 tx/s; escalation path if this is found in Slice 0.
- Non-determinism in batch ordering when affinity-workers race.
- Interaction between the new dispatcher and the existing tokio/async runtime in `pwmd`.

---

### [WARN] Sprint 1 acceptance criterion deferred to an unconfirmed ticket

**Location:** `mvp_v7s1.md` § Цели и приёмка — "Детальные критерии и тесты будут доработаны в тикете спринта"

**Issue:** Acceptance criteria are deferred to a sprint ticket that is not confirmed to exist at review time. The persona policy requires verifiable acceptance criteria in the Sprint Brief itself. Delegating them to a ticket that may not be created before coding starts creates a gate gap: the coding worker proceeds without a checkable definition of done.

**Suggested fix:** Remove the deferral sentence. Inline the concrete criteria directly in `mvp_v7s1.md` (or resolve the BLOCKER above, which implicitly closes this WARN).

---

### [WARN] Sprints V7-1, V7-2, V7-3 in `mvp_v7.md` lack pre-conditions for their slices

**Location:** `mvp_v7.md` § Sprint V7-1 (Декомпозиция), § Sprint V7-2 (Scope), § Sprint V7-3 (Scope)

**Issue:** Sprints V7-1, V7-2, and V7-3 each have a decomposition/scope section listing slices, but none provide slice-level pre-conditions or post-conditions. V7-1 Slice 1 ("RPC extension + types") and Slice 2 ("TUI UI + poll + rendering") have no stated post-conditions. V7-3 scope describes the implementation but does not state a pre-condition check (e.g., "ADR 0012 status = Accepted" — ADR 0012 is already Accepted, but this is not asserted in the sprint pre-condition). V7-2 references a ticket (`tasks/20260616-v7-bruteforce-occupied-skip-mt.json`) with acceptance criteria, which partially covers this, but the linkage is not made explicit as a pre-condition.

This is `[WARN]` rather than `[BLOCKER]` because these are future sprints that have not opened yet, and the persona policy rates missing rollback (not pre/post) as WARN for future-sprint items; however, it is a systematic gap that should be resolved before the respective sprints open.

**Suggested fix:** Before each sprint opens, ensure its slice table in `mvp_v7.md` or a dedicated `mvp_v7sN.md` is populated with pre/post-conditions per slice. Flag this as a condition of sprint-open ritual in the orchestrator notes.

---

### [WARN] V7-3 pre-condition "ADR 0012 accepted" is de-facto satisfied but not stated

**Location:** `mvp_v7.md` § Sprint V7-3 → Предусловие

**Issue:** V7-3 states "Предусловие: ADR 0012 accepted, V6-7 emergency activation." ADR 0012's status header reads "Accepted as V7 normative contract." This pre-condition is satisfied, but the sprint plan treats it as a future condition to be satisfied, creating potential confusion: a coding worker reading the plan cannot determine whether "ADR 0012 accepted" means "it will be accepted before this sprint opens" or "it is already accepted now." If the sprint opens immediately, the coding worker may delay unnecessarily waiting for a sign-off that already exists.

**Suggested fix:** Annotate the pre-condition: "ADR 0012 — already Accepted (see `docs/adr/0012-emergency-stake-evacuation.md`). Pre-condition satisfied as of plan date."

---

### [WARN] `mvp_v7s1.md` Mermaid diagram shows Worker Pool feeding Orchestrator, which then feeds Seal — but the architecture description says the orchestrator IS the main thread that does both dispatch and seal

**Location:** `mvp_v7s1.md` § Mermaid-диаграммы (план), flowchart TD

**Issue:** The Mermaid diagram shows: `G[Пул воркеров] → I[Оркестратор] → J[Seal]`. This implies a three-stage pipeline: workers → orchestrator → seal. But the architecture description in both `mvp_v7.md` and `mvp_v7s1.md` states that the "Главный поток (orchestrator)" is the one that runs seal and dispatches. The diagram inverts the direction: workers produce prepared batches that flow TO the orchestrator, not that the orchestrator is downstream of workers in a separate stage. A coding worker reading the diagram would implement a pipeline where the orchestrator is a separate component receiving work from workers, rather than the orchestrator dispatching TO workers and then collecting results for seal.

This is an architectural ambiguity that could lead to a structurally incorrect implementation.

**Suggested fix:** Revise the diagram to reflect actual data flow:
```
Ingress → Dispatcher (orchestrator-owned) → Queue(s) → Workers
Workers → prepared-batch → Orchestrator → Seal
```
And clarify in prose: "The orchestrator thread owns both the dispatcher and the seal step. Workers are subordinate, not upstream."

---

### [SUGGEST] Sprints V7-4+ section is titled as a single section but covers three separate sprints

**Location:** `mvp_v7.md` § Sprint V7-4+ : Offchain production, Devnet, BFT ADR (черновик)

**Issue:** V7-4, V7-5, and V7-6 are collapsed into one section with "(черновик)" qualifier. This is acceptable for a version spec at this stage, but the heading implies a single sprint ("V7-4+") while containing three distinct sprint scopes. This could confuse an orchestrator reading the plan to derive sprint count or resource allocation.

**Suggested fix:** Retitle as "### Sprints V7-4, V7-5, V7-6 (Draft)" and add a one-line sentence clarifying these will each get their own `mvp_v7s4.md`, `mvp_v7s5.md`, `mvp_v7s6.md` when the respective sprint opens.

---

### [SUGGEST] "Substantial growth" language also appears in `mvp_v7.md` Sprint 1 acceptance block

**Location:** `mvp_v7.md` § Sprint 1 (V7-S1) → Acceptance, bullet 3: "Рабочий прототип даёт существенный рост throughput без потери детерминизма"

**Issue:** This is the same unverifiable phrase as the BLOCKER above, but noted here separately as a SUGGEST for the version spec (the BLOCKER is primarily owned by the sprint brief). Once the BLOCKER is resolved in `mvp_v7s1.md`, the corresponding line in `mvp_v7.md` should be updated to match.

**Suggested fix:** Replace with the concrete criterion from the BLOCKER fix above, or state: "≥50 tx/s sustained per `mvp_v7s1.md` acceptance criteria."

---

### [SUGGEST] Throughput gate value "≥50 tx/s" appears in multiple locations with slightly different qualifiers

**Location:** `mvp_v7.md`: demo-ready result §6, Sprint 1 scope, V7-5 throughput gate, inter-sprint gates table; `mvp_v7s1.md`: Цели и приёмка

**Issue:** The target figure appears in five locations with varying qualifiers ("sustained ≥50 tx/s", "стабильные ≥50 tx/s", "≥ целевого уровня (см. V7-5 и Sprint 1)", "≥50 tx/s sustained"). Two locations use "or agreed minimum with owner" hedges. This creates ambiguity about whether 50 is the actual contractual floor or a placeholder.

**Suggested fix:** Define the number once, canonically, in `mvp_v7.md` § Межспринтовые гейты → Devnet gate. All other locations reference it as "see Devnet gate." If the number may change after profiling, say so explicitly: "Baseline target ≥50 tx/s; may be revised after Sprint 1 Slice 0 profiling with owner sign-off."

---

### [SUGGEST] Sprint 1 decomposition note "(ориентир)" should be dropped or replaced with a normative statement

**Location:** `mvp_v7s1.md` § Декомпозиция (ориентир)

**Issue:** The label "(ориентир)" (guideline/orientation) signals the decomposition is non-binding. For a sprint brief that is about to drive coding work, all decomposition must be normative. Leaving it labeled as "ориентир" gives a coding worker grounds to deviate from the defined slices without escalating.

**Suggested fix:** Remove "(ориентир)" from the section heading and add an explicit note: "Slice breakdown is normative for this sprint. Deviations require orchestrator sign-off before proceeding."

---

## Corrected artifact

No corrected artifact produced — the BLOCKERs require human decisions (concrete throughput criterion agreed with owner, explicit OUT scope enumeration, and slice pre/post-condition authoring by a domain-aware agent or human) before a rewrite is appropriate.
