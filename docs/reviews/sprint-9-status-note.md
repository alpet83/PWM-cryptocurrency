# Sprint 9 Status Note

Дата: 2026-04-26  
Этап: Slice 5/6 completed (Sprint 9 wrap-up)  
Статус: **SPRINT 9 CLOSED (HANDOFF TO SPRINT 10 HARDENING)**

## Slice 0 Start/Ready State

- Sprint 9 scope зафиксирован: two-shard operator demo через переключение `PWM_RPC`, burn quota path в CLI/TUI, документированные сценарии.
- Non-goals: без изменения `pwmd` wire API; без новых `TxBody` в ядре; EXPORT/IMPORT CLI — gated до появления ядра или явные defer-артефакты.
- Зависимость задокументирована: `TxBody` не содержит EXPORT/IMPORT в текущей кодовой базе.
- Созданы: `sprint-9-checklist.md`, `sprint-9-status-note.md`, `sprint-9-review-report.md`, `sprint-9-test-report.md`, `docs/tester-guide-cli-tui-scenarios.md`.

## Current Gates (Slice 5 wrap-up)

- Coding gate (`cargo fmt --check`): **PASS**
- Testing gate (coding-pass smoke for docs-only closeout): **PASS** (`cargo check -p pwm-cli`)
- Review gate: **APPROVE WITH NITS** (low-only, deferred hardening scope)
- Artifact closeout: **PASS** для Slice 5 / Sprint 9

## Next Step

- Sprint 10 (hardening): reliability/UX hardening и подготовка к будущему EXPORT/IMPORT после ядра.

## Slice 5 Update (consolidated closeout)

- Закрыты итоговые артефакты Sprint 9:
  - `sprint-9-checklist.md`
  - `sprint-9-status-note.md`
  - `sprint-9-review-report.md`
  - `sprint-9-test-report.md`
- Подтверждено выполнение slices 0..5, включая Slice 4 gated defer.
- Подведен summary изменений по продуктовым зонам Sprint 9:
  - Slice 2: CLI hints/errors для `tx-burn-mark`;
  - Slice 3: TUI footer context + F5 `not wired yet` UX.
- Зафиксированы residual risks и handoff-гипотезы для Sprint 10 hardening.

## Slice 4 Update (gated defer)

- Аудит CLI/TUI: отдельные user-facing `export/import` подкоманды отсутствуют.
- Уточнение по UX-границе: `wallet import-seed` в `pwm-cli` — это импорт seed для кошелька, не cross-shard EXPORT/IMPORT wire-flow.
- Принят и зафиксирован вариант **defer** без добавления ложных stub-команд:
  - `pwm-core::TxBody` по-прежнему не содержит EXPORT/IMPORT;
  - `pwmd` wire/API контракты не менялись;
  - операторская фиксация defer обновлена в `docs/tester-guide-cli-tui-scenarios.md` и `docs/reviews/sprint-9-checklist.md`.

## Slice 1 Update

- Финализирован `docs/tester-guide-cli-tui-scenarios.md` для двухшардового сценария (A/B, `PWM_RPC` switch, `v1/head` checks, acceptance).
- Добавлены скрипты-подсказки:
  - `tools/demo-two-shard.ps1`
  - `tools/demo-two-shard.sh`
- Выполнены полные regression suites для UX-crates:
  - `cargo test -p pwm-cli`: 62 passed
  - `cargo test -p pwm-tui`: 51 passed

## Slice 2 Update

- В `crates/pwm-cli/src/main.rs` для `tx-burn-mark` улучшены operator hints:
  - help-текст команды и аргументов (`mark_amount`, `beneficiary`) теперь явно напоминает про `--rpc` / `PWM_RPC`;
  - ошибки парсинга/валидации `--beneficiary` дополнены подсказкой проверить source-shard RPC target.
- Семантика wire/API не менялась: tx body и отправка остались прежними.
- Regression:
  - `cargo test -p pwm-cli`: 62 passed

## Slice 3 Update

- В `crates/pwm-tui/src/main.rs` добавлен явный footer context `RPC=<url> (<shard hint>)`.
- Обновлен F5 UX: вместо общего TODO — явное `not wired yet` с подсказкой использовать `pwm --rpc <url> tx-burn-mark ...` и ссылкой на гайд.
- Добавлены/уточнены unit-тесты под новый UX, включая сообщение F5 и shard-hint mapping.
- Обновлен `docs/tester-guide-cli-tui-scenarios.md` по факту поведения F5.

### Low nits (из review-pass)

- shard-hint пока эвристический (по порту URL);
- нет отдельного event-path теста именно на routing `KeyCode::F(5) -> info_modal` (есть unit coverage текстового helper-а).

## Handoff from Sprint 8

- Ядро/pwmd: `marks_quota`, `BURN_MARK`, zero-fee baseline, source-only burn context — закрыто в Sprint 8; CLI уже содержит `tx-burn-mark`.
