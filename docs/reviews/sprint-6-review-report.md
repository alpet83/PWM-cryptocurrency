# Sprint 6 Review Report (Slice #1)

Политика улик: narrative — про семантику кода и риски; **не** дублировать в тексте per-file numstat по `docs/reviews/sprint-6-*.md` и task-json (процессный шум, self-reference). В `review_evidence_manifest_sliceN.scoped_diff_stat` по умолчанию только пути **`crates/**`** и **`tools/**`** — см. `tools/README-slice-artifacts.md` (*Sprint 6 evidence policy*).

Политика темпа: дальнейшие micro-оптимизации в `pwmd` **батчить по 3–4 осмысленных правки на один slice/closeout** (один прогон тестов + один коммит по manifest), чтобы не раздувать конвейер на десятки однотипных шагов. **Декомпозиция `crates/pwmd/src/lib.rs` по модулям** — отдельный спринт/инициатива, не смешивать с узким Sprint 6 optimization conveyor.

Date: 2026-04-24

## Verdict

PASS (accepted as baseline reset)

## Findings by severity

### High
- No semantic defects confirmed by available evidence (`pwm-testing` regression gate is green).

### Medium
- The diff labeled as optimization-only was broader than intended (scope drift), which reduced review confidence for strict "refactor-only" framing.

### Low
- Iterative optimization process discipline needs stronger scope boundaries in coding handoffs.

## Decision

- Slice #1 is accepted as the new baseline (no blocker by semantics).
- Process correction applied: subsequent optimization slices must be strict `optimization-only` with narrow, explicit diff boundaries.

## Recommendation

`ready_for_next_slice`

- Slice #2 should focus on one isolated quick win (shared native-live/degraded helper extraction only), with no tx-path/API behavior changes.

---

## Slice #2 Review Gate

### Verdict

REQUEST CHANGES (process evidence gap, not semantic blocker)

### Findings by severity

#### High
- No semantic blocker was confirmed in inspected helper extraction paths.

#### Medium
- Strict optimization-only scope discipline could not be proven with high confidence because the working tree/diff context is too broad for a narrow-slice audit trail.

#### Low
- Optimization value is present (native-live/degraded duplication reduced), but review evidence quality must be improved for strict gates.

### Recommendation

- Keep slice #2 code as accepted baseline by orchestrator semantics policy.
- For the next slice, require narrow diff evidence (`touched symbols + diff stat + isolated scope`) before review verdict finalization.

---

## Slice #3 Review Gate

### Verdict

REQUEST CHANGES (process-evidence, semantic drift not found)

### Findings by severity

#### High
- No semantic blocker was confirmed for backoff calculator unification.

#### Medium
- Strict narrow-diff discipline is still not formally provable in the current broad working-tree context; review confidence is limited by evidence quality, not by detected behavior issues.

#### Low
- Optimization objective is met (duplicate delay formula removed via shared helper), but process evidence remains below strict gate target.

### Recommendation

- Keep slice #3 code as accepted baseline under orchestrator semantics policy.
- For slice #4, attach explicit narrow-scope evidence set before review (`scope files`, `touched symbols`, `no-change assertions`, short scoped diff snapshot).

---

## Slice #4 Review Gate

### Verdict

REQUEST CHANGES (process-evidence), semantic pass

### Findings by severity

#### High
- No semantic blocker was confirmed for typed class-label refactor.

#### Medium
- Strict narrow-diff proof remains insufficient in current broad working-tree context; process evidence quality is below strict gate expectation.

#### Low
- Optimization value is achieved (stringly-typed label duplication reduced via typed mapping), but review traceability still needs tighter slice-isolated evidence.

### Recommendation

- Keep slice #4 code as accepted baseline under orchestrator semantics policy.
- For slice #5, attach explicit scope-evidence pack directly in review artifacts: scoped diff snapshot, touched symbols, and no-change contract assertions.

---

## Slice #5 Scope Proof (pre-review)

### Allowed files (strict list)

- `crates/pwmd/src/lib.rs`
- `docs/reviews/sprint-6-checklist.md`
- `docs/reviews/sprint-6-status-note.md`
- `docs/reviews/sprint-6-review-report.md`
- `tasks/20260424-sprint6-optimization.json`

### Touched symbols (slice-local)

- `compose_class_result_key(...)`
- `record_transport_attempt(...)`

### Explicit no-change assertions

- tx-path/tx guards: no changes
- HTTP routes: no changes
- response fields: no changes
- error messages: no changes
- new API fields: not added
- business logic/transport semantics: behavior-preserving refactor only

### Scoped diff stat snapshot

- `crates/pwmd/src/lib.rs` | modified (scoped `git diff --stat`: 2510 lines changed, broad pre-existing working-tree context)
- `docs/reviews/sprint-6-checklist.md` | untracked (new review artifact in current working tree)
- `docs/reviews/sprint-6-status-note.md` | untracked (new review artifact in current working tree)
- `docs/reviews/sprint-6-review-report.md` | untracked (new review artifact in current working tree)
- `tasks/20260424-sprint6-optimization.json` | untracked (new task artifact in current working tree)

---

## Slice #5 Review Gate

### Verdict

REQUEST CHANGES (process-evidence), semantic pass

### Findings by severity

#### High
- No semantic blocker was confirmed for key-composition helper extraction.

#### Medium
- Even with added pre-review scope proof, strict narrow-diff evidence is still limited by broad working-tree context; process traceability remains below strict gate bar.

#### Low
- Optimization value is present but micro-scoped (maintainability uplift via centralized key composition).

### Recommendation

- Keep slice #5 code as accepted baseline under orchestrator semantics policy.
- Continue iterative optimization with explicit slice-isolated evidence pack in each next review cycle.

---

## Slice #6 Scope Proof (pre-review)

### Allowed files (strict list)

- `crates/pwmd/src/lib.rs`
- `docs/reviews/sprint-6-checklist.md`
- `docs/reviews/sprint-6-status-note.md`
- `docs/reviews/sprint-6-review-report.md`
- `tasks/20260424-sprint6-optimization.json`

### Touched symbols (slice-local)

- `compose_class_state_key(...)`
- `record_transport_attempt(...)`

### Explicit no-change assertions

- tx-path/tx guards: no changes
- HTTP routes: no changes
- response fields: no changes
- error messages: no changes
- new API fields: not added
- business logic/transport semantics: behavior-preserving refactor only

### Scoped diff stat snapshot

- `crates/pwmd/src/lib.rs` | modified (scoped `git diff --stat`: 2515 lines changed, broad pre-existing working-tree context)
- `docs/reviews/sprint-6-checklist.md` | untracked (new review artifact in current working tree)
- `docs/reviews/sprint-6-status-note.md` | untracked (new review artifact in current working tree)
- `docs/reviews/sprint-6-review-report.md` | untracked (new review artifact in current working tree)
- `tasks/20260424-sprint6-optimization.json` | untracked (new task artifact in current working tree)

---

## Slice #6 Review Gate

### Verdict

APPROVE WITH NITS (semantic pass, process-evidence improved)

### Findings by severity

#### High
- No semantic blocker was confirmed for class-state key helper centralization.

#### Medium
- Strict narrow-diff traceability is still partially constrained by broad pre-existing working-tree context, but evidence quality improved materially via `review_evidence_manifest_slice6` + pre-review scope proof.

#### Low
- Optimization value is incremental but valid (reduced duplication and clearer key-composition path for class-state maps).

### Recommendation

- Accept slice #6 as baseline.
- Continue the same manifest-driven evidence pattern for subsequent slices to reduce remaining process friction.

---

## Slice #7 Scope Proof (pre-review)

### Allowed files (strict list)

- `crates/pwmd/src/lib.rs`
- `docs/reviews/sprint-6-checklist.md`
- `docs/reviews/sprint-6-status-note.md`
- `docs/reviews/sprint-6-review-report.md`
- `tasks/20260424-sprint6-optimization.json`

### Touched symbols (slice-local)

- `increment_reject_reason_total(...)`
- `process_incoming_peer_hello(...)`

### Explicit no-change assertions

- tx-path/tx guards: no changes
- HTTP routes: no changes
- response fields: no changes
- error messages: no changes
- new API fields: not added
- transport scheduling/backoff semantics: no changes

### Scoped diff stat snapshot

- `crates/pwmd/src/lib.rs` | modified (scoped `git diff --stat`: 2519 lines changed; broad pre-existing working-tree context)
- `docs/reviews/sprint-6-checklist.md` | untracked
- `docs/reviews/sprint-6-status-note.md` | untracked
- `docs/reviews/sprint-6-review-report.md` | untracked
- `tasks/20260424-sprint6-optimization.json` | untracked

---

## Slice #7 Review Gate

### Verdict

REQUEST CHANGES (process-evidence incomplete), semantic pass

### Findings by severity

#### High
- No semantic blocker was confirmed for reject-reason counter helper centralization.

#### Medium
- Process-evidence package exists, but strict gate trail remained incomplete before final review sync entries were recorded.

#### Low
- Optimization value is valid but micro-scoped.

### Recommendation

- Complete review sync entries for slice #7 and continue manifest-driven workflow.

---

## Slice #8 Scope Proof (pre-review)

### Allowed files (strict list)

- `crates/pwmd/src/lib.rs`
- `docs/reviews/sprint-6-checklist.md`
- `docs/reviews/sprint-6-status-note.md`
- `docs/reviews/sprint-6-review-report.md`
- `tasks/20260424-sprint6-optimization.json`

### Touched symbols (slice-local)

- `increment_class_accept_total(...)`
- `process_incoming_peer_hello(...)`

### Explicit no-change assertions

- tx-path/tx guards: no changes
- HTTP routes: no changes
- response fields: no changes
- error messages: no changes
- new API fields: not added
- transport scheduling/backoff semantics: no changes

### Scoped diff stat snapshot

- `crates/pwmd/src/lib.rs` | modified (scoped `git diff --stat`: 12 lines changed in current slice workspace snapshot)
- `docs/reviews/sprint-6-checklist.md` | modified (22 insertions)
- `docs/reviews/sprint-6-review-report.md` | modified (34 insertions)
- `docs/reviews/sprint-6-status-note.md` | modified (11 insertions)
- `tasks/20260424-sprint6-optimization.json` | modified (45 insertions)

---

## Slice #8 Review Gate

### Verdict

REQUEST CHANGES (process-evidence incomplete), semantic pass

### Findings by severity

#### High
- No semantic blocker was confirmed for class-accept counter helper centralization.

#### Medium
- Strict process gate was not closed at review time due to missing finalized review-sync records.

#### Low
- Optimization value is valid and behavior-preserving, but micro-scoped.

### Recommendation

- Complete review-sync records for slice #8 and continue manifest-driven workflow.

---

## Slice #9 Scope Proof (pre-review)

### Allowed files (strict list)

- `crates/pwmd/src/lib.rs`
- `docs/reviews/sprint-6-checklist.md`
- `docs/reviews/sprint-6-status-note.md`
- `docs/reviews/sprint-6-review-report.md`
- `tasks/20260424-sprint6-optimization.json`

### Touched symbols (slice-local)

- `increment_class_bucket(...)` (new)
- `increment_class_accept_total(...)`
- `v1_dev_peers(...)`

### Explicit no-change assertions

- tx-path/tx guards: no changes
- HTTP routes: no changes
- response fields: no changes (same `PeerStatsOut` construction and field values)
- error messages: no changes
- new API fields: not added
- transport scheduling/backoff semantics: no changes
- `class_accept_total` / `connected_by_class` key strings and counting rules: unchanged (same `class_label` mapping and peer status filter in `v1_dev_peers`)

### Scoped diff stat snapshot

- `crates/pwmd/src/lib.rs` | modified (scoped `git diff --stat`: 6 insertions, 7 deletions)
- `docs/reviews/sprint-6-checklist.md` | modified (22 insertions)
- `docs/reviews/sprint-6-review-report.md` | modified (36 insertions)
- `docs/reviews/sprint-6-status-note.md` | modified (11 insertions)
- `tasks/20260424-sprint6-optimization.json` | modified (47 insertions)

---

## Slice #9 Review Gate

### Verdict

**PASS** (semantic), процессные артефакты синхронизированы для closeout slice #9

### Findings by severity

#### High

- Семантических регрессий по tx-path, guards, HTTP маршрутам и контрактам ответов не выявлено; изменения ограничены private helpers и dev-агрегацией `connected_by_class`.

#### Medium

- Нет.

#### Low

- Узкий выигрыш по DRY для string-keyed per-class counters; дальнейшие slices — только при явном backlog и том же manifest-discipline.

### Recommendation

- Зафиксировать slice #9 отдельным коммитом; при следующем slice повторять узкий `allowed_files` + `review_evidence_manifest_sliceN`.

---

## Slice #10 Scope Proof (pre-review)

### Allowed files (strict list)

- `crates/pwmd/src/lib.rs`
- `docs/reviews/sprint-6-checklist.md`
- `docs/reviews/sprint-6-status-note.md`
- `docs/reviews/sprint-6-review-report.md`
- `tasks/20260424-sprint6-optimization.json`

### Touched symbols (slice-local)

- `is_peer_liveish(...)` (new)
- `prioritize_peer_candidates(...)`
- `count_native_live_peers(...)`
- `v1_dev_peers(...)`

### Explicit no-change assertions

- tx-path/tx guards: no changes
- HTTP routes: no changes
- response fields: no changes
- error messages: no changes
- new API fields: not added
- transport scheduling/backoff semantics: no changes
- liveish status set semantics: unchanged (`Accepted | Connected | Retrying`)

### Scoped diff stat snapshot

- `crates/pwmd/src/lib.rs` | modified (scoped `git diff --numstat`: 11 insertions, 14 deletions)
- `docs/reviews/sprint-6-checklist.md` | modified (22 insertions)
- `docs/reviews/sprint-6-review-report.md` | modified (37 insertions)
- `docs/reviews/sprint-6-status-note.md` | modified (11 insertions)
- `tasks/20260424-sprint6-optimization.json` | modified (48 insertions)

---

## Slice #10 Review Gate

### Verdict

**PASS** (semantic), process review-sync completed for slice #10

### Findings by severity

#### High

- Semantic regressions were not found; `is_peer_liveish(...)` preserves the same liveish status set and replaces duplicated inline predicates in local read/selection paths only.

#### Medium

- None.

#### Low

- Optimization value is intentionally micro-scoped (predicate DRY); keep the same manifest discipline for subsequent slices.

### Recommendation

- Record slice #10 as a separate commit and continue narrow, behavior-preserving optimization slices with explicit `review_evidence_manifest_sliceN`.

---

## Slice #11 Scope Proof (pre-review)

### Allowed files (strict list)

- `crates/pwmd/src/lib.rs`
- `docs/reviews/sprint-6-checklist.md`
- `docs/reviews/sprint-6-status-note.md`
- `docs/reviews/sprint-6-review-report.md`
- `docs/reviews/sprint-6-test-report.md`
- `tasks/20260424-sprint6-optimization.json`

### Touched symbols (slice-local)

- `is_native_for_local(...)` (new)
- `prioritize_peer_candidates(...)`
- `count_native_live_peers(...)`

### Explicit no-change assertions

- tx-path/tx guards: no changes
- HTTP routes: no changes
- response fields: no changes
- error messages: no changes
- new API fields: not added
- transport scheduling/backoff semantics: no changes
- peer class mapping rule: unchanged (strict domain equality via `classify_peer`)

### Scoped diff stat snapshot

- `crates/pwmd/src/lib.rs` | modified (scoped `git diff --numstat` vs HEAD: 7 insertions, 4 deletions)
- `docs/reviews/sprint-6-checklist.md` | modified (21 insertions)
- `docs/reviews/sprint-6-review-report.md` | modified (64 insertions)
- `docs/reviews/sprint-6-status-note.md` | modified (22 insertions)
- `docs/reviews/sprint-6-test-report.md` | modified (38 insertions)
- `tasks/20260424-sprint6-optimization.json` | modified (80 insertions)

---

## Slice #11 Review Gate

### Verdict

**PASS** (semantic), process review-sync completed for slice #11

### Findings by severity

#### High

- No semantic regressions identified; helper is a pure refactor of an existing `classify_peer(...) == PeerClass::Native` predicate.

#### Medium

- None.

#### Low

- Micro-scoped DRY only; keep manifest-driven slice discipline.

### Recommendation

- Commit slice #11 as an isolated change-set; continue iterative optimization with `review_evidence_manifest_sliceN` + optional tooling dry-run for file lists.

---

## Slice #12 Scope Proof (pre-review)

### Allowed files (strict list)

- `crates/pwmd/src/lib.rs`
- `docs/reviews/sprint-6-checklist.md`
- `docs/reviews/sprint-6-status-note.md`
- `docs/reviews/sprint-6-review-report.md`
- `docs/reviews/sprint-6-test-report.md`
- `tasks/20260424-sprint6-optimization.json`

### Touched symbols (slice-local)

- `classify_peer_for_hs(...)` (new)
- `run_transport_tick_with(...)`
- `process_incoming_peer_hello(...)`

### Explicit no-change assertions

- tx-path/tx guards: no changes
- HTTP routes: no changes
- response fields: no changes
- error messages: no changes
- new API fields: not added
- transport scheduling/backoff semantics: no changes
- peer classification semantics: unchanged (same `classify_peer` rule; wrapper only threads `hs.local_domain_hi`)

### Scoped diff stat snapshot

- `crates/pwmd/src/lib.rs` | modified (`git diff --numstat HEAD`: 6 insertions, 2 deletions)
- `docs/reviews/sprint-6-checklist.md` | modified (20 insertions, 0 deletions)
- `docs/reviews/sprint-6-review-report.md` | modified (64 insertions, 0 deletions)
- `docs/reviews/sprint-6-status-note.md` | modified (22 insertions, 0 deletions)
- `docs/reviews/sprint-6-test-report.md` | modified (38 insertions, 0 deletions)
- `tasks/20260424-sprint6-optimization.json` | modified (80 insertions, 0 deletions)

---

## Slice #12 Review Gate

### Verdict

**PASS** (semantic), process review-sync completed for slice #12

### Findings by severity

#### High

- Семантических регрессий не выявлено: `classify_peer_for_hs` — прямой delegate в `classify_peer(hs.local_domain_hi, peer_domain_hi)`; изменены только две production-точки, указанные в scope.

#### Medium

- None.

#### Low

- Микро-DRY вокруг handshake state; дальше — только при явном backlog и manifest-discipline.

### Recommendation

- Зафиксировать slice #12 отдельным коммитом; продолжать узкие optimization-only slices с `review_evidence_manifest_sliceN` и dry-run `slice-commit.ps1` при необходимости.

---

## Slice #13 Scope Proof (pre-review)

### Allowed files (strict list)

- `crates/pwmd/src/lib.rs`
- `docs/reviews/sprint-6-checklist.md`
- `docs/reviews/sprint-6-status-note.md`
- `docs/reviews/sprint-6-review-report.md`
- `docs/reviews/sprint-6-test-report.md`
- `tasks/20260424-sprint6-optimization.json`

### Touched symbols (slice-local)

- `increment_string_u64_bucket(...)` (new)
- `increment_reject_reason_total(...)`
- `increment_class_bucket(...)`

### Explicit no-change assertions

- tx-path/tx guards: no changes
- HTTP routes: no changes
- response fields: no changes
- error messages: no changes
- new API fields: not added
- string-keyed counter semantics: unchanged (`reject_reason_total` keys `reason_label.to_string()`; class buckets `class_label(class).to_string()`; same `entry(...).or_insert(0) += 1` behavior)

### Scoped diff stat snapshot

- `crates/pwmd/src/lib.rs` | modified (git diff --numstat vs HEAD: 6 insertions, 5 deletions)
- `docs/reviews/sprint-6-checklist.md` | modified (21 insertions)
- `docs/reviews/sprint-6-review-report.md` | modified (63 insertions)
- `docs/reviews/sprint-6-status-note.md` | modified (22 insertions)
- `docs/reviews/sprint-6-test-report.md` | modified (37 insertions)
- `tasks/20260424-sprint6-optimization.json` | modified (80 insertions)

---

## Slice #13 Review Gate

### Verdict

**PASS** (semantic), process review-sync completed for slice #13

### Findings by severity

#### High

- Семантических регрессий не выявлено: helper — thin wrapper вокруг прежнего `entry(...).or_insert(0) += 1` для тех же string keys и map.

#### Medium

- None.

#### Low

- Узкий DRY вокруг `HashMap<String, u64>` инкремента; manifest-discipline сохранена.

### Recommendation

- Зафиксировать slice #13 отдельным коммитом; `slice-artifacts.ps1 -Mode fill-diff` на UTF-8 task-json с кириллицей **не использовать** до фикса кодировки (ConvertTo-Json + Set-Content исказили файл) — для numstat предпочтительно ручное копирование строк из `git diff --numstat HEAD` или точечный патч manifest.

---

## Slice #14 Scope Proof (pre-review)

### Allowed files (strict list)

- `crates/pwmd/src/lib.rs`
- `docs/reviews/sprint-6-checklist.md`
- `docs/reviews/sprint-6-status-note.md`
- `docs/reviews/sprint-6-review-report.md`
- `docs/reviews/sprint-6-test-report.md`
- `tasks/20260424-sprint6-optimization.json`

### Touched symbols (slice-local)

- `increment_string_u64_bucket(...)` (reuse)
- `record_transport_attempt(...)`
- `record_churn_attempt(...)`

### Explicit no-change assertions

- tx-path/tx guards: no changes
- HTTP routes: no changes
- response fields: no changes
- error messages: no changes
- new API fields: not added
- transport scheduling/backoff semantics: no changes
- dial/churn counter key semantics: unchanged (`dial_attempt_by_class_result` keys via `compose_class_result_key(...)` → same `format!("{}:{}", class_key, result.as_label())`; `seed_attempt_by_result` keys `result.as_label().to_string()`; same `entry(...).or_insert(0) += 1` behavior via helper)
- `last_attempt_ms_by_class` / `last_result_by_class` updates in `record_transport_attempt`: unchanged

### Scoped diff stat snapshot

- `crates/pwmd/src/lib.rs` | modified (git diff --numstat vs HEAD: 5 insertions, 9 deletions)
- `docs/reviews/sprint-6-checklist.md` | modified (20 insertions)
- `docs/reviews/sprint-6-review-report.md` | modified (65 insertions)
- `docs/reviews/sprint-6-status-note.md` | modified (22 insertions)
- `docs/reviews/sprint-6-test-report.md` | modified (33 insertions)
- `tasks/20260424-sprint6-optimization.json` | modified (80 insertions)

---

## Slice #14 Review Gate

### Verdict

**PASS** (semantic), process review-sync completed for slice #14

### Findings by severity

#### High

- Семантических регрессий не выявлено: helper переиспользован для тех же string-keyed map инкрементов в transport/churn путях.

#### Medium

- None.

#### Low

- Узкий DRY; батчинг артефактов (один патч на несколько файлов) снижает число мелких правок.

### Recommendation

- Зафиксировать slice #14 отдельным коммитом; slice #15 добавляет `patch-manifest-numstat` для автоподстановки `scoped_diff_stat` без полного `ConvertTo-Json` всего task-файла.

---

## Slice #15 Scope Proof (pre-review, tooling)

### Allowed files (strict list)

- `tools/slice-artifacts.ps1`
- `tools/README-slice-artifacts.md`
- `docs/reviews/sprint-6-checklist.md`
- `docs/reviews/sprint-6-status-note.md`
- `docs/reviews/sprint-6-review-report.md`
- `docs/reviews/sprint-6-test-report.md`
- `tasks/20260424-sprint6-optimization.json`

### Touched symbols (slice-local)

- `Build-ManifestScopedDiffStatHumanStrings(...)`
- `Find-ScopedDiffStatArrayBoundsRaw(...)`
- `Update-TaskManifestNumstatRaw(...)`
- `patch-manifest-numstat` mode branch

### Explicit no-change assertions

- `crates/pwmd` Rust sources: no changes
- tx-path/tx guards: no changes
- HTTP routes: no changes
- response fields: no changes
- error messages: no changes
- new API fields: not added
- transport scheduling/backoff semantics: no changes

### Code delta (manifest policy)

- Фактический numstat по sprint-артефактам и task-json **не** дублируется в этом отчёте (избегаем self-reference и мгновенно устаревающих метаданных).
- Сводка **только по коду/тулингу** для slice #15 — в `tasks/20260424-sprint6-optimization.json` → `review_evidence_manifest_slice15.scoped_diff_stat` (пути `tools/**`; без `docs/reviews/sprint-6-*.md` и без строки по самому task-json).

---

## Slice #15 Review Gate

### Verdict

**PASS** (tooling / process), review-sync completed for slice #15

### Findings by severity

#### High

- Нет затрагивания runtime `pwmd`; режим `patch-manifest-numstat` снижает риск порчи кириллицы от полной пересборки JSON.

#### Medium

- None.

#### Low

- Raw-патч зависит от наличия валидного JSON-массива `scoped_diff_stat` внутри manifest; при экзотическом форматировании нужен ручной контроль.

### Recommendation

- Для sprint-6 task json с кириллицей по умолчанию использовать `patch-manifest-numstat` вместо `fill-diff` для обновления numstat; в `scoped_diff_stat` по умолчанию только `crates/**` и `tools/**` (см. `README-slice-artifacts.md`). Markdown closeout — **одним батч-патчем**, без зеркалирования numstat по артефактам в review-тексте.

---

## Slice #16 Scope Proof (pre-review, optimization-only)

### Allowed files (strict list)

- `crates/pwmd/src/lib.rs`
- `docs/reviews/sprint-6-checklist.md`
- `docs/reviews/sprint-6-status-note.md`
- `docs/reviews/sprint-6-review-report.md`
- `docs/reviews/sprint-6-test-report.md`
- `tasks/20260424-sprint6-optimization.json`

### Touched symbols (slice-local)

- `transport_outbound_slot(...)` (new)
- `run_transport_tick_with(...)`

### Explicit no-change assertions

- tx-path/tx guards: no changes
- HTTP routes: no changes
- response fields: no changes
- error messages: no changes
- new API fields: not added
- transport scheduling/backoff semantics: unchanged (только объединение чтения limit + выбора scheduled counter)
- `select_backoff_for_class`, `record_transport_attempt`, dial/churn metrics: unchanged call order и контракты

### Code delta (manifest policy)

- Сводка по коду — в `tasks/20260424-sprint6-optimization.json` → `review_evidence_manifest_slice16.scoped_diff_stat` (только `crates/**` по политике улик).

---

## Slice #16 Review Gate

### Verdict

**PASS** (semantic), process review-sync completed for slice #16

### Findings by severity

#### High

- Семантических регрессий не выявлено: helper возвращает ту же пару (mutable scheduled slot, limit), что и два прежних `match` подряд.

#### Medium

- None.

#### Low

- Узкий DRY вокруг outbound cap lookup в transport tick.

### Recommendation

- Зафиксировать slice #16 отдельным коммитом; дальше — следующие узкие optimization-only slices при наличии backlog.

---

## Slice #17 Scope Proof (pre-review, optimization-only)

### Allowed files (strict list)

- `crates/pwmd/src/lib.rs`
- `docs/reviews/sprint-6-checklist.md`
- `docs/reviews/sprint-6-status-note.md`
- `docs/reviews/sprint-6-review-report.md`
- `docs/reviews/sprint-6-test-report.md`
- `tasks/20260424-sprint6-optimization.json`

### Touched symbols (slice-local)

- `dial_attempt_class_key(...)` (new)
- `run_real_transport_tick(...)` (seed dial loop call-site)

### Explicit no-change assertions

- tx-path/tx guards: no changes
- HTTP routes: no changes
- response fields: no changes
- error messages: no changes
- new API fields: not added
- `attempt_seed_connect` result tuple semantics: unchanged
- `record_transport_attempt` / `record_churn_attempt` call order: unchanged

### Code delta (manifest policy)

- Сводка по коду — в `tasks/20260424-sprint6-optimization.json` → `review_evidence_manifest_slice17.scoped_diff_stat` (только `crates/**`).

---

## Slice #17 Review Gate

### Verdict

**PASS** (semantic), process review-sync completed for slice #17

### Findings by severity

#### High

- Семантических регрессий не выявлено: helper эквивалентен прежней цепочке `map`/`unwrap_or` по строковым label.

#### Medium

- None.

#### Low

- Узкий DRY вокруг optional `PeerClass` → dial metric class key.

### Recommendation

- Зафиксировать slice #17 отдельным коммитом; продолжать узкие optimization-only slices по мере backlog.

---

## Slice #18 Scope Proof (pre-review, optimization-only)

### Allowed files (strict list)

- `crates/pwmd/src/lib.rs`
- `docs/reviews/sprint-6-checklist.md`
- `docs/reviews/sprint-6-status-note.md`
- `docs/reviews/sprint-6-review-report.md`
- `docs/reviews/sprint-6-test-report.md`
- `tasks/20260424-sprint6-optimization.json`

### Touched symbols (slice-local)

- `enqueue_seed_by_last_peer_class(...)` (new)
- `run_real_transport_tick(...)`

### Explicit no-change assertions

- tx-path/tx guards: no changes
- HTTP routes: no changes
- response fields: no changes
- error messages: no changes
- new API fields: not added
- seed rotation cursor / churn `seed_rotation_total`: unchanged semantics
- порядок `due.extend(native|unknown|foreign)` и attempt budget: unchanged

### Code delta (manifest policy)

- Сводка по коду — в `tasks/20260424-sprint6-optimization.json` → `review_evidence_manifest_slice18.scoped_diff_stat` (только `crates/**`).

---

## Slice #18 Review Gate

### Verdict

**PASS** (semantic), process review-sync completed for slice #18

### Findings by severity

#### High

- Семантических регрессий не выявлено: helper — прямой перенос прежнего `match` по `Option<PeerClass>`.

#### Medium

- None.

#### Low

- Узкий DRY вокруг seed queue routing.

### Recommendation

- Зафиксировать slice #18 отдельным коммитом; при исчерпании локальных micro-DRY — планировать модульную декомпозицию `lib.rs` отдельной серией (не смешивать с узкими optimization slices без явного решения).
## Slice #19 Scope Proof (pre-review, optimization-only, batched)

### Allowed files (strict list)

- `crates/pwmd/src/lib.rs`
- `docs/reviews/sprint-6-checklist.md`
- `docs/reviews/sprint-6-status-note.md`
- `docs/reviews/sprint-6-review-report.md`
- `docs/reviews/sprint-6-test-report.md`
- `tasks/20260424-sprint6-optimization.json`

### Touched symbols (slice-local)

- `apply_transport_peer_result(...)` (new)
- `seed_peer_state_mut(...)` (new)
- `update_known_peer_status(...)` (new)
- `retryable_connect_outcome()` (new)
- `run_transport_tick_with(...)`
- `attempt_seed_connect(...)`
- `run_real_transport_tick(...)`

### Explicit no-change assertions

- tx-path/tx guards: no changes
- HTTP routes: no changes
- response fields: no changes
- error messages: no changes
- new API fields: not added
- scheduler/backoff/churn semantics: unchanged
- peer status transition semantics: unchanged

### Code delta (manifest policy)

- Сводка по коду — в `tasks/20260424-sprint6-optimization.json` → `review_evidence_manifest_slice19.scoped_diff_stat` (только `crates/**`).

---

## Slice #19 Review Gate

### Verdict

**PASS** (semantic), process review-sync completed for slice #19

### Findings by severity

#### High

- Семантических регрессий не выявлено: batched helper extraction сохраняет прежние ветки и значения в transport/seed paths.

#### Medium

- None.

#### Low

- Рефакторинг затрагивает несколько соседних участков сразу (батч 4 правок), что повышает важность дисциплины regression gate; текущий full suite зелёный.

### Recommendation

- Зафиксировать slice #19 отдельным коммитом; продолжать batched micro-slices по 3–4 правки до точки убывающей отдачи, затем переключаться на отдельный sprint декомпозиции `lib.rs`.

## Slice #20 Scope Proof (pre-review, optimization-only, batched)

### Allowed files (strict list)

- `crates/pwmd/src/lib.rs`
- `docs/reviews/sprint-6-checklist.md`
- `docs/reviews/sprint-6-status-note.md`
- `docs/reviews/sprint-6-review-report.md`
- `docs/reviews/sprint-6-test-report.md`
- `tasks/20260424-sprint6-optimization.json`

### Touched symbols (slice-local)

- `rotate_seed_order(...)` (new)
- `update_seed_peer_after_attempt(...)` (new)
- `set_seed_peer_next_due(...)` (new)
- `apply_reconnect_streak_tick(...)` (new)
- `run_real_transport_tick(...)`

### Explicit no-change assertions

- tx-path/tx guards: no changes
- HTTP routes: no changes
- response fields: no changes
- error messages: no changes
- new API fields: not added
- scheduler/backoff/churn semantics: unchanged
- peer status transition semantics: unchanged

### Code delta (manifest policy)

- Сводка по коду — в `tasks/20260424-sprint6-optimization.json` → `review_evidence_manifest_slice20.scoped_diff_stat` (только `crates/**`).

---

## Slice #20 Review Gate

### Verdict

**PASS** (semantic), process review-sync completed for slice #20

### Findings by severity

#### High

- Семантических регрессий не выявлено: batched helper extraction сохраняет прежние ветки и значения в real transport tick paths.

#### Medium

- None.

#### Low

- Рефакторинг затрагивает несколько соседних участков сразу (батч 4 правок), что повышает важность дисциплины regression gate; текущий full suite зелёный.

### Recommendation

- Зафиксировать slice #20 отдельным коммитом; продолжать batched micro-slices по 3–4 правки до точки убывающей отдачи, затем переключаться на отдельный sprint декомпозиции `lib.rs`.

## Slice #21 Scope Proof (pre-review, optimization-only, batched)

### Allowed files (strict list)

- `crates/pwmd/src/lib.rs`
- `docs/reviews/sprint-6-checklist.md`
- `docs/reviews/sprint-6-status-note.md`
- `docs/reviews/sprint-6-review-report.md`
- `docs/reviews/sprint-6-test-report.md`
- `tasks/20260424-sprint6-optimization.json`

### Touched symbols (slice-local)

- `soak_counter_cap(...)` (new)
- `refresh_real_tick_state(...)` (new)
- `collect_due_seed_attempts(...)` (new)
- `apply_seed_attempt_result(...)` (new)
- `finalize_real_tick(...)` (new)
- `run_real_transport_tick(...)`

### Explicit no-change assertions

- tx-path/tx guards: no changes
- HTTP routes: no changes
- response fields: no changes
- error messages: no changes
- new API fields: not added
- scheduler/backoff/churn semantics: unchanged
- peer status transition semantics: unchanged

### Code delta (manifest policy)

- Сводка по коду — в `tasks/20260424-sprint6-optimization.json` → `review_evidence_manifest_slice21.scoped_diff_stat` (только `crates/**`).

---

## Slice #21 Review Gate

### Verdict

**PASS** (semantic), process review-sync completed for slice #21

### Findings by severity

#### High

- Семантических регрессий не выявлено: batched helper extraction сохраняет прежние ветки и значения в real transport tick orchestration.

#### Medium

- None.

#### Low

- Рефакторинг затрагивает несколько соседних участков сразу (батч 4 правок), что повышает важность дисциплины regression gate; текущий full suite зелёный.

### Recommendation

- Зафиксировать slice #21 отдельным коммитом; продолжать batched micro-slices по 3–4 правки до точки убывающей отдачи, затем переключаться на отдельный sprint декомпозиции `lib.rs`.

## Slice #22 Scope Proof (pre-review, optimization-only, cleanup)

### Allowed files (strict list)

- `crates/pwmd/src/lib.rs`
- `docs/reviews/sprint-6-checklist.md`
- `docs/reviews/sprint-6-status-note.md`
- `docs/reviews/sprint-6-review-report.md`
- `docs/reviews/sprint-6-test-report.md`
- `tasks/20260424-sprint6-optimization.json`

### Touched symbols (slice-local)

- `refresh_native_health(...)`
- `record_transport_attempt(...)`
- `collect_due_seed_attempts(...)`
- removed `compose_class_state_key(...)`

### Explicit no-change assertions

- tx-path/tx guards: no changes
- HTTP routes: no changes
- response fields: no changes
- error messages: no changes
- new API fields: not added
- scheduler/backoff/churn semantics: unchanged
- peer status transition semantics: unchanged

### Code delta (manifest policy)

- Сводка по коду — в `tasks/20260424-sprint6-optimization.json` → `review_evidence_manifest_slice22.scoped_diff_stat` (только `crates/**`).

---

## Slice #22 Review Gate

### Verdict

**PASS** (semantic), process review-sync completed for slice #22

### Findings by severity

#### High

- Семантических регрессий не выявлено: cleanup не меняет ветвления/пороги/формулы, только упрощает локальные state-updates.

#### Medium

- None.

#### Low

- Низкоуровневый cleanup (inline refs/locals/remove passthrough helper) требует дисциплины regression gate; текущий suite зелёный.

### Recommendation

- Зафиксировать slice #22 отдельным коммитом; продолжать batched micro-slices по 3–4 правки до точки убывающей отдачи, затем переключаться на отдельный sprint декомпозиции `lib.rs`.

## Slice #23 Scope Proof (pre-review, optimization-only, cleanup)

### Allowed files (strict list)

- `crates/pwmd/src/lib.rs`
- `docs/reviews/sprint-6-checklist.md`
- `docs/reviews/sprint-6-status-note.md`
- `docs/reviews/sprint-6-review-report.md`
- `docs/reviews/sprint-6-test-report.md`
- `tasks/20260424-sprint6-optimization.json`

### Touched symbols (slice-local)

- `class_label(...)`
- `ClassLabel::from_peer_class(...)` removed
- `is_native_for_local(...)`
- `enqueue_seed_by_last_peer_class(...)`
- `collect_due_seed_attempts(...)`

### Explicit no-change assertions

- tx-path/tx guards: no changes
- HTTP routes: no changes
- response fields: no changes
- error messages: no changes
- new API fields: not added
- scheduler/backoff/churn semantics: unchanged
- peer status transition semantics: unchanged

### Code delta (manifest policy)

- Сводка по коду — в `tasks/20260424-sprint6-optimization.json` → `review_evidence_manifest_slice23.scoped_diff_stat` (только `crates/**`).

---

## Slice #23 Review Gate

### Verdict

**PASS** (semantic), process review-sync completed for slice #23

### Findings by severity

#### High

- Семантических регрессий не выявлено: cleanup не меняет ветвления/пороги/формулы, только устраняет лишние преобразования/clone в helper-уровне.

#### Medium

- None.

#### Low

- `is_native_for_local(...)` использует прямое сравнение `domain_hi`, что эквивалентно текущей классификации; при эволюции правил классификации потребуется синхронизировать источник истины.

### Recommendation

- Зафиксировать slice #23 отдельным коммитом; выполнить ещё один batched micro-slice и затем подвести итоги Sprint 6 с планом следующего optimization спринта.

## Slice #24 Scope Proof (pre-review, optimization-only, cleanup)

### Allowed files (strict list)

- `crates/pwmd/src/lib.rs`
- `docs/reviews/sprint-6-checklist.md`
- `docs/reviews/sprint-6-status-note.md`
- `docs/reviews/sprint-6-review-report.md`
- `docs/reviews/sprint-6-test-report.md`
- `tasks/20260424-sprint6-optimization.json`

### Touched symbols (slice-local)

- `prioritize_peer_candidates(...)`
- `peer_priority_rank(...)`
- `refresh_native_health(...)`
- `compose_class_result_key(...)`
- `increment_string_u64_bucket(...)`
- `increment_reject_reason_total(...)`
- `increment_class_bucket(...)`
- `record_transport_attempt(...)`
- `record_churn_attempt(...)`

### Explicit no-change assertions

- tx-path/tx guards: no changes
- HTTP routes: no changes
- response fields: no changes
- error messages: no changes
- new API fields: not added
- scheduler/backoff/churn semantics: unchanged
- peer status transition semantics: unchanged

### Code delta (manifest policy)

- Сводка по коду — в `tasks/20260424-sprint6-optimization.json` → `review_evidence_manifest_slice24.scoped_diff_stat` (только `crates/**`).

---

## Slice #24 Review Gate

### Verdict

**PASS** (semantic), process review-sync completed for slice #24

### Findings by severity

#### High

- Семантических регрессий не выявлено: изменения ограничены helper-уровнем, порядок приоритизации и state transitions сохранены.

#### Medium

- None.

#### Low

- Отдельный performance baseline не фиксировался; верифицирована функциональная parity и отсутствие drift по текущему suite.

### Recommendation

- Зафиксировать slice #24 отдельным коммитом; перейти к подведению итогов Sprint 6 и формированию плана следующего optimization спринта.
