# Sprint 9 Review Report (Slice 0-5 Consolidated Closeout)

Дата: 2026-04-26  
Исполнитель: coding-pass wrap-up

## Review Scope

- Slice 0/6: planning/freeze baseline.
- Slice 1/6: two-shard demo guide finalization + helper scripts (`tools/demo-two-shard.ps1`, `tools/demo-two-shard.sh`), без изменений product Rust semantics.
- Slice 2/6: CLI ergonomics для `tx-burn-mark` (`pwm-cli`) — help/error hint improvements без изменения wire behavior.
- Slice 3/6: TUI RPC/shard context + F5 not-wired operator message (with CLI handoff), включая unit coverage.
- Slice 4/6 (gated): EXPORT/IMPORT UX defer decision, docs-only фиксация без stub-команд и псевдо-функциональности.
- Slice 5/6: consolidated closeout artifacts, quality gates summary, residual risks, Sprint 10 hardening handoff.

## Scope Proof Verdict

**PASS (consolidated closeout)**

- Scope Sprint 9 формализован в `sprint-9-checklist.md` с явным gate на EXPORT/IMPORT до появления ядра.
- Touched zones для execution slices 1-5 указаны в чеклисте.
- Acceptance pack для оператора зафиксирован в `docs/tester-guide-cli-tui-scenarios.md`.

## No-Change Guardrails (explicit)

- Не менять `pwmd` HTTP контракты и error map в рамках Sprint 9 без отдельного согласования.
- Не добавлять `TxBody` варианты в `pwm-core` под видом «CLI sprint».
- Сохранить обратную совместимость существующих подкоманд `pwm` и текущего TUI lifecycle.

## Change Surface (Slice 0)

- Added: `docs/reviews/sprint-9-checklist.md`
- Added: `docs/reviews/sprint-9-status-note.md`
- Added: `docs/reviews/sprint-9-review-report.md`
- Added: `docs/reviews/sprint-9-test-report.md`
- Added: `docs/tester-guide-cli-tui-scenarios.md`

## Slice 1 Scope Proof

### Touched zones

- `docs/tester-guide-cli-tui-scenarios.md`
- `tools/demo-two-shard.ps1`
- `tools/demo-two-shard.sh`
- `docs/reviews/sprint-9-checklist.md`
- `docs/reviews/sprint-9-status-note.md`
- `docs/reviews/sprint-9-test-report.md`
- настоящий файл

### Explicit no-change assertions

- `pwmd` wire API routes/DTO/errors: no changes
- `pwm-core::TxBody` surface: no changes
- Existing CLI/TUI command behavior: no semantic changes in code (docs/scripts only)

## Slice 2 Scope Proof

### Touched zones

- `crates/pwm-cli/src/main.rs`
- `docs/reviews/sprint-9-checklist.md`
- `docs/reviews/sprint-9-status-note.md`
- `docs/reviews/sprint-9-test-report.md`
- настоящий файл

### Explicit no-change assertions

- `pwmd` HTTP routes/DTO/error-map: no changes
- `pwm-core` tx/state contracts: no changes
- `tx-burn-mark` wire payload (`TxBody::BurnMark`) and submit path: unchanged

### Ergonomics delta

- Added operator-facing RPC target hints in `tx-burn-mark` help and beneficiary validation errors.

## Slice 3 Scope Proof

### Touched zones

- `crates/pwm-tui/src/main.rs`
- `docs/tester-guide-cli-tui-scenarios.md`
- `docs/reviews/sprint-9-checklist.md`
- `docs/reviews/sprint-9-status-note.md`
- `docs/reviews/sprint-9-test-report.md`
- настоящий файл

### Explicit no-change assertions

- `pwmd` routes/DTO/error contracts: no changes
- `pwm-core` tx/state contracts: no changes
- hotkeys безопасности/выхода (F3/F4/F10/q): unchanged

### Review findings

- High: none
- Medium: none
- Low:
  - shard hint реализован эвристикой по порту URL (`3030/3031`)
  - нет отдельного теста event-loop wiring для `KeyCode::F(5)` (есть unit test helper/message)

## Slice 4 Scope Proof (gated defer)

### Touched zones

- `docs/reviews/sprint-9-checklist.md`
- `docs/reviews/sprint-9-status-note.md`
- `docs/reviews/sprint-9-test-report.md`
- `docs/tester-guide-cli-tui-scenarios.md`
- настоящий файл

### Explicit no-change assertions

- `pwmd`/`pwm-core` code and wire contracts: no changes
- `pwm-cli`/`pwm-tui` code paths: no new export/import stubs added
- deferred state documented explicitly to prevent operator confusion

### Review findings

- High: none
- Medium: none
- Low:
  - artifact consistency nit fixed: Slice 4 reflected as current gate/review stage

## Slice 5 Scope Proof (wrap-up)

### Touched zones

- `docs/reviews/sprint-9-checklist.md`
- `docs/reviews/sprint-9-status-note.md`
- `docs/reviews/sprint-9-review-report.md`
- `docs/reviews/sprint-9-test-report.md`
- настоящий файл

### Explicit no-change assertions

- Product code (`pwm-core`, `pwmd`, `pwm-cli`, `pwm-tui`): no changes in Slice 5
- Wire/API contracts: no changes
- Wrap-up выполнен docs-only, без подмены testing-pass и без финального release verdict

### Quality gates summary

- Coding gate: `cargo fmt --check` -> PASS
- Smoke gate (docs-only rationale): `cargo check -p pwm-cli` -> PASS
- Artifact gate: consolidated Sprint 9 closeout -> PASS

## Residual risks (carry to Sprint 10)

- shard-hint в TUI остаётся эвристическим по URL/порту, возможны неоднозначности вне стандартных портов.
- F5 path остаётся информационным (`not wired yet`), с зависимостью от CLI handoff и operator discipline.
- EXPORT/IMPORT остаётся gated до появления соответствующих `TxBody` в ядре.

## Slice 0-5 Review Gate

### Verdict

**APPROVE WITH NITS (docs-only wrap-up)**

### Recommendation

- Перейти к Sprint 10 hardening: UX reliability и подготовка к post-core EXPORT/IMPORT enablement.
