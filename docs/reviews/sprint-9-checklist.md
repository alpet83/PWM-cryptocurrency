# Sprint 9 Checklist: CLI/TUI Integration for Two-Shard Demo Ops

Дата старта: 2026-04-25  
Фокус: операторский UX (`pwm-cli`, `pwm-tui`) и воспроизводимые сценарии без изменения контрактов `pwmd`/`pwm-core`, кроме согласованных точек интеграции.

## Scope Freeze (Slice 0 baseline)

### In Scope

- Два независимых `pwmd` (**shard A / shard B**) с разными `--listen` и изолированными `--state-root`; переключение клиента через `PWM_RPC` / флаг `--rpc` у `pwm`.
- Документированный **scripted demo** для оператора (см. `docs/tester-guide-cli-tui-scenarios.md`).
- **Burn quota path:** довести до demo-ready связку CLI ↔ уже существующий `tx-burn-mark` ↔ `pwmd` (при необходимости — подсказки, ошибки, TUI).
- **TUI:** явный **shard / RPC context** в интерфейсе; локальная **история операций** (уже частично есть — расширить/связать с cross-shard сценарием переключения RPC).
- Минимальные **operator checklists** в гайде (happy + 2 negative на уровне UX: неверный RPC, отказ ноды).

### Non-Goals (Sprint 9)

- Изменение HTTP API `pwmd`, DTO полей и error map (кроме явно согласованного микро-текста help в CLI, не влияющего на wire).
- Новые типы `TxBody` в `pwm-core` (в т.ч. полноценный **EXPORT/IMPORT wire**): это отдельные roadmap-спринты по ядру; в Sprint 9 допускаются только **документированный defer**, UX-заглушки или подготовка фасада CLI **после** появления ядра.
- Рефакторинг `pwmd` module layout (наследие Sprint 7) и optimization backlog Sprint 11.

### Зависимость (явная)

- В текущем дереве `pwm-core::TxBody` **нет** вариантов EXPORT/IMPORT; сообщения `pwmd` уже ссылаются на explicit EXPORT/IMPORT flow как на будущий путь. Слайсы Sprint 9 по export/import CLI помечаются в чеклисте как **gated** до появления соответствующего ядра **или** переносятся в backlog с ссылкой на `docs/reviews/v1-testnet-decision-options-20260423.md`.

## Pre-Task (обязательный старт)

- [x] Подтверждён scope и non-goals для Sprint 9 (Slice 0 freeze).
- [x] Сверены целевые модули roadmap: `crates/pwm-cli/src/main.rs`, `crates/pwm-tui/src/main.rs`, `docs/tester-guide-cli-tui-scenarios.md`.
- [x] Зафиксирован baseline acceptance pack (1 happy + 2 negative) на уровне оператора — см. гайд § «Acceptance».

## Sprint 9 Guardrails

- [x] Не ломать существующие подкоманды `pwm` и флаги без миграционной заметки в гайде.
- [x] `pwm-tui`: сохранить текущие горячие клавиши выхода/безопасности; новые действия — за feature-флагами или отдельным экраном, если иначе риск регрессии UX.
- [x] Любой новый RPC-вызов из CLI/TUI — с таймаутом и понятным текстом ошибки (как сейчас в TUI).

## Slices (план исполнения)

### Slice 0/6: Planning + Freeze

- [x] Этот чеклист, `sprint-9-status-note.md`, `sprint-9-review-report.md` (baseline), `sprint-9-test-report.md` (sanity), `docs/tester-guide-cli-tui-scenarios.md` (черновик с двумя шардами).

### Slice 1/6: Two-Shard Demo Scripts + Гайд

- [x] Завершить гайд: пошаговый сценарий A/B, переключение `PWM_RPC`, проверка `v1/head` на обоих портах.
- [x] Добавлены обёртки запуска: `tools/demo-two-shard.ps1` и `tools/demo-two-shard.sh`.

### Slice 2/6: CLI — Burn + Shard Operator Ergonomics

- [x] Аудит `tx-burn-mark` help/ошибок; добавлены подсказки про `--rpc` / shard без смены wire API.
- [x] Регрессия: `cargo test -p pwm-cli`.

### Slice 3/6: TUI — Shard Context + History

- [x] Отображение активного RPC / shard hint в статус-строке.
- [x] Уточнён MVP TODO по burn/send (F5): явный `not wired yet` с направлением на CLI (`tx-burn-mark`) и гайд.

### Slice 4/6: Export/Import UX (Gated)

- [x] Принят вариант **defer**: user-facing `export/import` подкоманды в CLI/TUI отсутствуют, добавлена явная операторская фиксация defer в Sprint 9 артефактах и гайде; ложная функциональность не добавлялась.

### Slice 5/6: Wrap-Up

- [x] Consolidated closeout выполнен: test/review evidence, residual risks, handoff в Sprint 10 (hardening) зафиксированы в review/status/test артефактах.

## Sprint 9 Closeout (0..5)

- [x] Slice 0: scope freeze и guardrails подтверждены.
- [x] Slice 1: two-shard demo guide + helper scripts зафиксированы.
- [x] Slice 2: CLI ergonomics (`tx-burn-mark` hints/errors) без wire/API drift.
- [x] Slice 3: TUI context (`RPC/shard`) и F5 UX (`not wired yet` + CLI handoff) закрыты.
- [x] Slice 4: EXPORT/IMPORT оформлен как gated defer без ложных product-stubов.
- [x] Slice 5: consolidated closeout, residual risks и handoff в Sprint 10 оформлены.

## Sprint 10 Handoff (hardening focus)

- [x] Приоритет: hardening operator UX и reproducibility для two-shard сценариев.
- [x] Отдельно от Sprint 9: вернуться к EXPORT/IMPORT только после появления соответствующих `TxBody` в `pwm-core`.

## Gates Per Slice

- [x] Coding gate: `cargo fmt --check`, `cargo check -p pwm-cli -p pwm-tui` (и `pwmd` при касании контрактов).
- [x] Testing gate: `cargo test -p pwm-cli`, `cargo test -p pwm-tui` (и при изменениях общей логики — `cargo test -p pwmd` по решению слайса).
- [x] Review gate: нет нежелательного drift API ноды; сценарии гайда воспроизводимы.
- [x] Artifact closeout: обновление review/checklist/status-note/test-report по слайсу.

Примечание Slice 5 (docs-only closeout): применено исключение testing gate — smoke `cargo check -p pwm-cli` вместо полного regression pack.
