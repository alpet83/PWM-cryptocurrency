# Sprint 11 Checklist: DomainHi Migration / Relay-by-Default / Shard Alias Deprecation

Дата старта: 2026-04-26  
Sprint тип: migration track (официальный reset)  
Фокус: перейти на `domain_hi` + default relay mode, оставить `--shard` только как deprecated compat alias (soft-break, без hard removal).

## Scope Freeze (финальный)

### In Scope (Sprint 11)

- Runtime mode policy: relay mode по умолчанию.
- Явная shard-support логика только через domain config (`domain_hi` path).
- Миграция CLI/config на domain-first вход, с deprecated alias `--shard`.
- Storage namespace policy под domain-oriented keying + compat mapping.
- Conformance-обновления docs/reviews/test artifacts под новую policy.

### Non-Goals (Sprint 11)

- Удаление `--shard` (в Sprint 11 только deprecation).
- Любое расширение wire/API контрактов `pwmd` вне migration policy.
- Optimization-задачи (перенесены в Sprint 12).
- Полная тест-матрица вместо целевого migration coding-pass.

## Pre-Task Completion Notes (закрыто)

- [x] Baseline-артефакты Sprint 11 созданы.
- [x] Выполнена финальная нарезка Sprint 11 на execution slices `0..6` (N=6).
- [x] Draft-секции заменены рабочей slice-структурой с acceptance/gates.
- [x] Зафиксирована policy формулировка:
  - relay mode = default;
  - shard-support = explicit domain config only;
  - `--shard` = deprecated compat alias (soft-break).

## Execution Slices (рабочая структура, N=6)

### Slice 0/6 - Freeze and alignment

Цель: утвердить неизменяемую рамку migration-трека и критерии входа в coding-pass.  
Границы: только docs/ритуал, без product-code.

- [x] Scope/non-goals закреплены в этом checklist.
- [x] Зафиксирован перенос optimization в Sprint 12.
- [x] Зафиксирована deprecation policy для `--shard`.

Acceptance:
- policy формулировки не конфликтуют между review-артефактами Sprint 11.

Gate:
- `docs/reviews/sprint-11-checklist.md` содержит финальный freeze и pre-task completion notes.

### Slice 1/6 - Runtime mode semantics

Цель: формализовать runtime semantics для coding-pass.  
Границы: модель поведения, без реализации.

- [x] Первый migration шаг выполнен в коде: добавлен CLI alias `domain_cluster`/`domain-cluster` для `cluster_domain_hi`.
- [x] Описан `relay` как default mode.
- [x] Описан `shard-enforced` только при explicit domain config.
- [x] Исключена трактовка fixed `A/B` как primary runtime модели.

Acceptance:
- нет двусмысленности `relay` vs `shard-enforced` в sprint-11 docs.

Gate:
- status-note фиксирует текущую execution-фазу и policy без drift.
- coding-pass evidence зафиксирован: `pwmd` включает shard-guards только в explicit mode; baseline alias режим остается relay-compatible.
- regression-fix зафиксирован: baseline recipient prefilter reject (`400`) восстановлен для invalid recipient classes в `/v1/tx` независимо от режима.

### Slice 2/6 - CLI/config migration contract

Цель: зафиксировать migration-контракт входных параметров.  
Границы: contract + deprecation wording + targeted CLI/help coding-pass (без wire/API расширения).

- [x] Domain-first конфиг описан как целевой вход.
- [x] `--shard` описан как deprecated alias с warning.
- [x] Зафиксировано, что alias не удаляется в этом спринте.

Acceptance:
- migration contract консистентен с soft-break policy.

Gate:
- review-report отмечает отсутствие hard-break на Sprint 11.
- coding-pass evidence фиксирует runtime warning на explicit `--shard` и сохранение compat path.

### Slice 3/6 - Mode-bound guard policy

Цель: зафиксировать policy привязки guard-поведения к режиму.  
Границы: policy-level, без реализации guard-логики.

- [x] Guard policy указывает применение shard-enforced ограничений только в explicit domain режиме.
- [x] Для default relay режима зафиксировано отсутствие shard-enforced требований.

Acceptance:
- policy совместима с backward-compat alias трактовкой.

Gate:
- checklist и test-report используют одинаковую терминологию режимов.
- coding-pass evidence зафиксирован: baseline recipient prefilter активен всегда, shard-enforced local guards выполняются только в explicit mode.
- targeted checks для mode-bound policy задокументированы в sprint-11 test-report.

### Slice 4/6 - Storage namespace migration policy

Цель: определить рамку namespace-перехода на domain-oriented path.  
Границы: migration policy и compat mapping.

- [x] Зафиксирован domain-based namespace как target.
- [x] Зафиксирован compat mapping для legacy alias-сценариев.
- [x] Уточнено, что это migration-политика Sprint 11, не optimization-трек.

Acceptance:
- policy не вводит новый wire/API contract.

Gate:
- review-report отражает storage policy как migration-only step.
- coding-pass evidence фиксирует runtime namespace construction: explicit mode -> `domain-hi-0xNN`, alias mode -> legacy `shard-a|shard-b`.
- targeted checks подтверждают отсутствие hard-break по compat alias path (`--shard`).

### Slice 5/6 - Conformance docs and test baseline

Цель: синхронизировать review/test baseline под новую slice-модель.  
Границы: docs/reviews артефакты.

- [x] `sprint-11-status-note.md` синхронизирован по slices `0..6`.
- [x] `sprint-11-review-report.md` содержит baseline verdict по новой структуре.
- [x] `sprint-11-test-report.md` содержит baseline test-группы под migration policy.
- [x] Final docs hardening pass завершен: `docs/pwmd.md`, `README.md`, `docs/tester-guide-cli-tui-scenarios.md` приведены к domain-first operator contract (relay default + explicit shard-enforced + `--shard` deprecated compat alias).

Acceptance:
- все Sprint 11 review-артефакты используют одинаковый N и phase naming.

Gate:
- нет ссылок на draft-структуру в sprint-11 docs.
- coding-pass evidence фиксирует sync-фазу: README и sprint-11 review artifacts приведены к фактическому runtime Sprint 11 (relay default, explicit shard-enforced, domain namespace target + alias compat mapping).

### Slice 6/6 - Coding-pass readiness gate

Цель: подтвердить readiness для Slice 6 review/testing pass и handoff следующего implementation шага.  
Границы: readiness-check, без product-code изменений.

- [x] Проверен набор gates для migration coding-pass.
- [x] Зафиксирован handoff на final testing/review verdict.

Acceptance:
- checklist пригоден как execution документ для Slice 6 verification/handoff шага.

Gate:
- статус Sprint 11 в status-note установлен как review/testing pass phase для Slice 6.
- финальный coding-pass check set зафиксирован в test-report (policy consistency + targeted runtime smoke).

## Global Sprint Gates (для начала coding-pass)

- [x] Policy gate: relay mode указан как default везде, без исключений.
- [x] Domain gate: shard-support описан как explicit domain config only.
- [x] Compat gate: `--shard` отмечен как deprecated alias (warning, soft-break).
- [x] Scope gate: optimization явно вынесен в Sprint 12.
- [x] Artifact gate: checklist/status/review/test синхронизированы по slices `0..6`.
