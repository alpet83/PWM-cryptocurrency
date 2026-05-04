# Sprint 12 Status Note

Дата: 2026-04-26  
Этап: Slice 5 closeout coding-pass completed  
Статус: **SPRINT 12 CODING-PASS CLOSEOUT COMPLETE — READY FOR FINAL TESTING/REVIEW VERDICT**

## Current State

- Sprint 11 закрыт (`edf48a9`); domain-first migration считается завершенным baseline.
- Sprint 12 открыт как final optimization sprint в fixed-volume формате.
- Execution slices зафиксированы как `0..5` (N=5), без scope expansion.
- Slice 1 выполнен как узкий optimization-only micro-slice в `pwmd` без изменения контрактов.
- Slice 2 выполнен как узкий hot-path optimization micro-slice в `pwmd` без изменения контрактов.
- Slice 3 выполнен как узкий readability/perf-hygiene micro-slice в `pwmd` без изменения контрактов.
- Slice 4 выполнен как узкий cleanup/conformance micro-slice в `pwmd` без изменения контрактов.
- Post-closeout follow-up: default launch semantics уточнены до neutral relay-default без `A|B` affinity при запуске без `--shard`.

## Handoff Baseline from Sprint 11

- Relay-by-default policy сохраняется без переинтерпретации.
- Explicit shard-enforced semantics остаются только для explicit domain mode.
- `--shard` остается deprecated compat alias в рамках текущих guardrails.
- Sprint 12 работает поверх post-migration состояния, а не как новый migration-трек.

## Slice Progress

- `Slice 0/5`: completed (scope freeze, non-goals, gates, pre-task completion criteria).
- `Slice 1/5`: completed (behavior-preserving duplication/readability micro-slice в `crates/pwmd/src/transport.rs`).
- `Slice 2/5`: completed (hot-path micro-optimization: allocation hygiene в transport helper без API drift).
- `Slice 3/5`: completed (snapshot map update hardening: меньше лишних allocations/clone, behavior-preserving).
- `Slice 4/5`: completed (startup logging mode-string dedup + artifact conformance sync, behavior-preserving).
- `Slice 5/5`: completed (closeout readiness, evidence consolidation, final independent handoff).

## Slice 1 Coding Evidence

- В `record_transport_attempt(...)` устранено дублирование записи snapshot state через helper `update_last_attempt_snapshot(...)`.
- Изменение строго behavior-preserving: только consolidation локальной логики записи ключа/результата, без API/wire/runtime contract drift.
- Targeted checks: `cargo fmt`, `cargo check -p pwmd`, `cargo test -p pwmd transport` — PASS.

## Slice 2 Coding Evidence

- В `dial_attempt_class_key(...)` убрана лишняя аллокация `String`: helper теперь возвращает `&'static str` (native/foreign/unknown labels).
- Hot-path hygiene: в `run_real_transport_tick(...)` на каждый seed-attempt больше не создается временная строка класса для метрик transport snapshot.
- Изменение behavior-preserving: wire/API semantics не менялись, migration boundary Sprint 11 не затронут.
- Targeted checks: `cargo fmt`, `cargo check -p pwmd`, `cargo test -p pwmd transport` — PASS.

## Slice 3 Coding Evidence

- В `update_last_attempt_snapshot(...)` заменен unconditional clone/insert путь на branch-first update:
  - при наличии ключа используется `get_mut` и in-place update,
  - insert + allocation выполняется только на miss.
- Изменение строго behavior-preserving: сохраняется тот же snapshot contract (`last_attempt_ms_by_class`, `last_result_by_class`) без wire/API drift.
- Targeted checks: `cargo fmt`, `cargo check -p pwmd`, `cargo test -p pwmd transport` — PASS.

## Slice 4 Coding Evidence

- В `crates/pwmd/src/lifecycle.rs` добавлен helper `runtime_mode_summary(...)` для единого формирования startup mode-строки.
- Убрано дублирование match-блока в двух startup логах (`info!` + `eprintln!`), сохранив тот же текстовый contract вывода.
- Изменение строго behavior-preserving: без новых режимов/контрактов, без wire/API drift, без изменения migration boundary Sprint 11.
- Targeted checks: `cargo fmt`, `cargo check -p pwmd`, `cargo test -p pwmd transport` — PASS.

## Active Gates Snapshot

- Fixed-volume gate: `N=5`, execution only within slices `0..5`.
- No-scope-expansion gate: новые фичи/режимы/API контракты запрещены.
- Contract gate: wire/API behavior `pwmd` не расширяется.
- Migration-boundary gate: Sprint 11 migration closure не переоткрывается.
- Artifact gate: quartet Sprint 12 baseline артефактов создан.
- Process gate: optimization implementation стартует только после закрытого pre-task ритуала.

## Readiness

- Sprint 12 coding-pass закрыт в fixed-volume рамках `0..5` без scope expansion.
- Guardrails re-check закрыт: no-scope-expansion, no wire/API drift, migration boundary Sprint 11 соблюден.
- Следующий шаг: финальный независимый closeout verdict от `pwm-testing` и `pwm-review`.

## Follow-up Contract Sync (relay-neutral default)

- `crates/pwmd/src/main.rs`: убран implicit default `--shard A`; neutral relay baseline включается при запуске без explicit identity и без `--shard`.
- `crates/pwmd/src/identity.rs`: добавлен neutral runtime identity mode (`RuntimeIdentityMode::Neutral`) и namespace `neutral`; explicit alias path (`--shard A|B`) сохранен как deprecated compat mapping.
- `crates/pwmd/src/api.rs` + `crates/pwmd/src/lifecycle.rs`: status/startup логи отражают neutral shard label в default режиме (без alias `A|B`).
- `README.md` и `docs/pwmd.md`: docs-семантика default запуска синхронизирована с neutral relay-default контрактом.

## Slice 5 Closeout Evidence

- Консолидированы Sprint 12 evidence/verdict секции в `sprint-12-checklist`, `sprint-12-review-report`, `sprint-12-test-report`, `sprint-12-status-note`.
- Подтверждено, что sprint-10/11 артефакты не менялись в рамках Slice 5.
- Финальный coding-pass check set выполнен целево (без расширения тест-матрицы), результат: PASS.
