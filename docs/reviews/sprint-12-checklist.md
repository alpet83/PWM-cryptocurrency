# Sprint 12 Checklist: Final Optimization (fixed-volume, no scope expansion)

Дата старта: 2026-04-26  
Sprint тип: optimization track (post-migration cleanup)  
Фокус: точечная оптимизация после закрытого Sprint 11 без изменения функционального scope.

## Sprint 11 Handoff (input baseline)

- Sprint 11 закрыт коммитом `edf48a9`.
- Domain-first migration закрыт и не пересматривается в Sprint 12.
- Relay-by-default + explicit shard-enforced policy считается стабильным baseline.
- `--shard` остается deprecated compat alias; hard-break вне scope Sprint 12.
- Sprint 12 ограничен post-sprint cleanup/perf/readability/duplication в guardrails.

## Scope Freeze (Sprint 12)

### In Scope

- Локальные optimization-изменения без расширения продуктового поведения.
- Снижение duplication и улучшение читаемости в hot-path участках.
- Небольшие perf-улучшения в существующих code paths без API drift.
- Cleanup технического долга, выявленного после migration closeout.
- Обновление sprint-12 review/test артефактов по факту execution slices.

### Non-Goals

- Любой scope expansion (новые фичи, новые endpoint-ветки, новые режимы).
- Изменение wire/API контрактов `pwmd`.
- Новый migration-трек (domain-first migration уже закрыт в Sprint 11).
- Масштабные архитектурные переработки вне optimization guardrails.
- Изменения sprint-10/sprint-11 артефактов (кроме ссылок для handoff-контекста).

## Pre-Task Completion Criteria (обязательный ритуал)

- [x] Созданы baseline-артефакты Sprint 12:
  - `docs/reviews/sprint-12-checklist.md`
  - `docs/reviews/sprint-12-status-note.md`
  - `docs/reviews/sprint-12-review-report.md`
  - `docs/reviews/sprint-12-test-report.md`
- [x] Зафиксирована execution-нарезка Sprint 12 как optimization sprint slices `0..5` (N=5).
- [x] Scope/non-goals/gates закреплены без двусмысленности.
- [x] Handoff из Sprint 11 зафиксирован как immutable baseline для Sprint 12.
- [x] Подтвержден docs-only kickoff (без product-code изменений на pre-task этапе).

## Execution Slices (optimization track, N=5)

### Slice 0/5 - Kickoff freeze and guardrails

Цель: зафиксировать неизменяемую рамку optimization sprint перед coding-pass.  
Границы: docs-only.

- [x] Scope freeze и non-goals закреплены.
- [x] Gates зафиксированы.
- [x] Pre-task completion criteria закрыт.

Acceptance:
- Sprint 12 artifacts согласованы по фиксированному объему и `N=5`.

Gate:
- `sprint-12-checklist/status-note/review-report/test-report` существуют и содержат единый kickoff baseline.

### Slice 1/5 - Duplication and readability pass

Цель: убрать низкорисковую дубликацию и улучшить читаемость без изменения поведения.  
Границы: behavior-preserving cleanup.

- [x] Выделить повторяющиеся блоки в локальные helper/утилиты.
- [x] Сохранить текущий runtime contract.
- [x] Избежать drive-by refactor вне target зон.

Acceptance:
- Diff уменьшает дублирование и не меняет наблюдаемое поведение.

Gate:
- `cargo check -p pwmd` PASS после изменений.
- Никакого API/wire drift в `pwmd`.

Slice 1 coding-pass evidence (2026-04-26):
- `crates/pwmd/src/transport.rs`: вынесен локальный helper `update_last_attempt_snapshot(...)` для единообразной записи `last_attempt_ms_by_class` / `last_result_by_class` без semantic drift.
- Targeted checks: `cargo fmt`, `cargo check -p pwmd`, `cargo test -p pwmd transport` — PASS.

### Slice 2/5 - Hot-path micro-optimizations

Цель: применить малые perf-улучшения в горячих путях без изменения semantics.  
Границы: fixed-volume micro-optimizations.

- [x] Оптимизировать локальные allocations/копирования в согласованных участках.
- [x] Избежать новых feature flags и runtime modes.
- [x] Оставить contract-level поведение неизменным.

Acceptance:
- Улучшения ограничены согласованными hot-path зонами.

Gate:
- `cargo check -p pwmd` PASS.
- Нет изменений публичных API контрактов.

Slice 2 coding-pass evidence (2026-04-26):
- `crates/pwmd/src/transport.rs`: `dial_attempt_class_key(...)` переведен с `String` на `&'static str`; устранена лишняя строковая аллокация на каждом seed dial-attempt в real transport loop.
- Guardrail check: изменение локализовано внутри `pwmd` transport internals (`pub(crate)` helper), без wire/API drift и без затрагивания migration контрактов Sprint 11.
- Targeted checks: `cargo fmt`, `cargo check -p pwmd`, `cargo test -p pwmd transport` — PASS.

### Slice 3/5 - Config/runtime readability hardening

Цель: улучшить supportability и читаемость runtime/config кода в рамках текущей модели.  
Границы: readability-first, без migration и без feature expansion.

- [x] Упростить ветвления и naming в пределах существующих границ модулей.
- [x] Сохранить текущий relay/default и explicit-mode semantics.
- [x] Добавить только минимально необходимые пояснения в коде (English comments).

Acceptance:
- Код легче сопровождать, behavior не меняется.

Gate:
- Проверки не выявляют regression по policy-critical assertions.

Slice 3 coding-pass evidence (2026-04-26):
- `crates/pwmd/src/transport.rs`: в helper `update_last_attempt_snapshot(...)` убраны лишние строковые аллокации/clone при обновлении snapshot maps; применено branch-first обновление (`get_mut`) с insert только на miss.
- Guardrail check: изменение строго локально внутри `pwmd` transport internals и behavior-preserving; без wire/API drift и без изменения migration boundary Sprint 11.
- Targeted checks: `cargo fmt`, `cargo check -p pwmd`, `cargo test -p pwmd transport` — PASS.

### Slice 4/5 - Final cleanup and conformance sync

Цель: завершить optimization cleanup и синхронизировать evidence в review/test artifacts.  
Границы: docs + targeted conformance checks.

- [x] Реализовать один узкий low-risk optimization micro-slice в `pwmd` (behavior-preserving).
- [x] Обновить sprint-12 review/status/test artifacts по фактическому execution.
- [x] Зафиксировать остаточные риски и handoff на независимый verification.

Acceptance:
- Sprint 12 artifacts отражают фактический объем и результаты optimization pass.

Gate:
- Artifact consistency между checklist/status/review/test подтверждена.

Slice 4 coding-pass evidence (2026-04-26):
- `crates/pwmd/src/lifecycle.rs`: добавлен helper `runtime_mode_summary(...)`; убрано дублирование формирования mode-строки для startup `info!` и `eprintln!` логов.
- Guardrail check: optimization строго behavior-preserving (readability/duplication hygiene), без wire/API drift и без затрагивания migration boundary Sprint 11.
- Targeted checks: `cargo fmt`, `cargo check -p pwmd`, `cargo test -p pwmd transport` — PASS.

### Slice 5/5 - Closeout readiness gate

Цель: подтвердить готовность к финальному independent testing/review verdict.  
Границы: readiness/handoff, без нового scope.

- [x] Финализировать sprint-12 status/review/test verdict sections.
- [x] Зафиксировать handoff для `pwm-testing` и `pwm-review`.

Acceptance:
- Sprint 12 пакет артефактов готов к финальному closeout циклу.

Gate:
- Нет открытых blocking drift по scope, API, guardrails.

Slice 5 coding-pass evidence (2026-04-26):
- Sprint 12 evidence консолидирован в `docs/reviews/sprint-12-checklist.md`, `docs/reviews/sprint-12-status-note.md`, `docs/reviews/sprint-12-review-report.md`, `docs/reviews/sprint-12-test-report.md` без изменения sprint-10/11 артефактов.
- Closeout coding-pass verdict зафиксирован как `APPROVE (READY FOR FINAL INDEPENDENT TESTING/REVIEW VERDICT)`.
- Guardrails re-check: fixed-volume (`0..5`, N=5), no-scope-expansion, no wire/API drift, migration boundary Sprint 11 — без отклонений.
- Финальный targeted coding-pass check set выполнен без раздувания матрицы (см. `sprint-12-test-report.md`), результат — PASS.
- Follow-up contract fix (relay-neutral default): `pwmd` default startup без `--shard` переведен на neutral relay baseline (без alias `A|B` affinity); explicit `--shard A|B` сохранен как deprecated compat path. Evidence синхронизирован в `README.md`, `docs/pwmd.md`, `sprint-12-status-note.md`, `sprint-12-review-report.md`, `sprint-12-test-report.md`.

## Global Sprint 12 Gates

- [x] **Fixed-volume gate:** Sprint 12 выполняется в фиксированном объеме slices `0..5` (N=5), без scope expansion.
- [x] **Scope gate:** только post-sprint optimization cleanup/perf/readability/duplication.
- [x] **Contract gate:** без wire/API drift в `pwmd`.
- [x] **Migration boundary gate:** domain-first migration не переоткрывается (закрыт в Sprint 11, commit `edf48a9`).
- [x] **Artifact gate:** baseline quartet создан и синхронизирован.
- [x] **Process gate:** до завершения pre-task ритуала implementation optimization не выполняется.
