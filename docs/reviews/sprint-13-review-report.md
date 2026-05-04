# Sprint 13 Review Report — Inter-Shard MVP Cut

Статус: `CLOSED` (Slice 0..7 consolidated closeout complete)

## Slice 0 decision log (design freeze)
- Execution freeze утверждён: фиксированные 8 slices (`0..7`).
- Acceptance criteria зафиксированы как неизменяемый review baseline для Sprint 13.
- Out-of-scope lock утверждён: без admission/advanced policy.
- Conveyor делегирования утверждён: `pwm-coding -> pwm-testing -> pwm-review`.

## Review scope
- Соответствие реализации Sprint 13 фиксированному scope (8 slices).
- Проверка соответствия behavior и acceptance критериям inter-shard MVP.
- Анализ рисков регрессии для текущих local/domain-first flow.

## Review checkpoints
- [x] `pwm-core`: корректность export/import state transition и idempotency.
- [x] `pwmd`: API/status/error contract согласован и воспроизводим (Slice 3 baseline).
- [x] `pwm-cli`/`pwm-tui`: operator UX минимален, но достаточен для e2e.
- [x] Docs/runbook совпадают с фактическим поведением (минимум: `docs/pwmd.md` для `POST /v1/tx` + sprint-13 notes).
- [x] Out-of-scope lock соблюдён (без admission scope creep).

## Slice 3 review notes (`pwmd` RPC boundary for roaming)
- Наблюдаемость: `EXPORT/IMPORT` больше не “проглатываются” mempool-only путём; ошибки домена/state-machine доходят до HTTP на `POST /v1/tx`.
- Контракт: `TxError` маппится в стабильные HTTP-коды; `IMPORT` имеет ранний provenance/replay prefilter + последующий `apply_tx` guard (дублирование ок для baseline).
- Риски/tech debt (не блокер MVP cut, но зафиксировать): синхронный `apply_tx+seal([])` на HTTP hot-path (latency/конкуренция) — ожидаемо для devnet baseline; дальнейшая оптимизация вне Sprint 13 scope без расширения policy.

## Operator review runbook (CY->DO)
- Валидировать сквозной `CY -> DO` перенос: source debit (`EXPORT`) и destination credit (`IMPORT`).
- Проверить уникальность применения `IMPORT` по `export_id` (duplicate reject обязателен).
- Проверить восстановление replay guard после restart/snapshot restore.
- Проверить, что `pwmd`, `pwm-cli`, `pwm-tui` отдают согласованные статусы/ошибки на happy-path и negative-path.

## Severity rubric
- P0: блокирует inter-shard MVP cut.
- P1: не блокирует полностью, но критичен для closeout.
- P2: улучшения post-cut.

## Slice 7 consolidated closeout notes
- Проведена минимальная stabilization-сверка quartet артефактов Sprint 13: `checklist`, `test-report`, `status-note`, `review-report`.
- Все checkpoints финального closeout переведены в closed-state по факту закрытых Slice 0..6 и завершения Slice 7.
- Подтверждено отсутствие scope creep: новых feature/policy/admission изменений не добавлено, plan-файлы не менялись.

## Post-cut notes (residual, non-blocking)
- P2: sync `apply_tx + seal([])` на HTTP hot-path допустим для MVP/devnet baseline, но остается кандидатом на последующую оптимизацию latency/concurrency.
- P2: e2e маршрут валиден при текущем operator handoff (runbook/manual provenance transfer); автоматизация handoff не входит в Sprint 13.

## Final verdict
`APPROVE` — Sprint 13 consolidated closeout был завершен в рамках фиксированного scope (`0..7`); финальные независимые `pwm-testing`/`pwm-review` шаги были выполнены, closed-state зафиксирован.

## Post-freeze review addendum (Slice 13.8 coding)
- Delta scope ограничен backend runtime (`pwmd`) и не нарушает freeze-историю Sprint 13 (`0..7` остаётся закрытым).
- Добавлен federated roaming intent pool c lifecycle/TTL/locks без удаления существующего local mempool path.
- Lock семантика реализует mempool-level anti-double-spend для source funds во время активного roaming intent.
- API расширение минимальное и операторски понятное: create intent + query intent status; legacy `POST /v1/tx` путь остаётся совместим.
- Snapshot boundary обновлён: roaming intent state и lock-state переживают restart/restore.
- Primary risk после coding фазы: требуется отдельная независимая валидация race/interop сценариев в `pwm-testing` (особенно mixed local+roaming load).

