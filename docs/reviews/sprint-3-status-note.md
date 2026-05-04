# Sprint 3 Status Note (Closeout Snapshot)

**Sprint:** Sprint 3 - Evidence Hardening and Gate Formalization  
**Window:** 2026-05-09 .. 2026-05-22  
**Current phase:** closeout

## Closeout Summary

- Sprint 3 запущен как следующий инкремент после завершенного Sprint 2 без изменения протокольных правил.
- Цель текущего инкремента: повысить воспроизводимость gate-решения через четкие acceptance/negative сценарии и строгую последовательность ролей.
- Базовые инварианты, унаследованные из Sprint 2, зафиксированы без изменений:
  - без эвристики `0x80`,
  - разделение process-shard mapping и protocol routing,
  - local `TRANSFER` gate: `domain_hi(sender) == domain_hi(receiver)`,
  - recipient prefilter (`reserve/witness/unknown`) как отдельный pre-mempool слой.
- Ролевая модель сохранена:
  - implementation: `pwm-coding`,
  - verification: `pwm-testing`,
  - coherence gate: `pwm-review`,
  - final decision: orchestrator.
- Выполнен implementation pass по устранению концептуальной двусмысленности shard semantics:
  - закреплено нормативное определение spec-level geo-shard как фиксированного `domain_hi` кластера;
  - явно отделен текущий `pwmd --shard A|B` как dev/test process partition;
  - добавлен критический запрет на диапазонные эвристики вида `domain_hi < 0x80` vs `>= 0x80`;
  - добавлено допущение островизации на уровне доменного кластера без протокольного drift.

## Strict Gate Order

`pwm-coding -> pwm-testing -> pwm-review -> orchestrator decision`

Нарушение порядка gate считается блокером для финального решения Sprint 3.

## Kickoff Artifacts

- `docs/reviews/sprint-3-checklist.md` — scope, role matrix, acceptance criteria, negative scenarios, risk register.
- `docs/reviews/sprint-3-status-note.md` — текущий kickoff snapshot.
- `tasks/20260424-sprint3-orchestrated.json` — отражает факт делегирования и mini-report.

## Initial Risks (Kickoff)

- Возможен дрейф формулировок между checklist/status/task note, что ухудшает трассируемость решения.
- Возможна недетерминированность message-контрактов в negative-путях между шардами.
- Возможна подмена цели Sprint 3 (evidence hardening) на протокольные изменения вне scope.

## Final Gate State

- Coding gate: `pass` (implementation pass завершен, shard semantics ambiguity hardened).
- Testing gate: `pass` (`docs/reviews/sprint-3-test-report.md` опубликован).
- Review gate: `pass` (`docs/reviews/sprint-3-review-report.md` опубликован).
- Orchestrator decision: `ready_for_next_sprint`.

## Carry-over (non-blocking)

- Runtime-переход от dev/test process partition (`--shard A|B`) к явному запуску по конкретному доменному кластеру (`fixed domain_hi`) остаётся отдельным implementation шагом следующего спринта.
- Perf/load hardening и cross-shard finality остаются вне Sprint 3 scope.
