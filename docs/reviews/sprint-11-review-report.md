# Sprint 11 Review Report (Slice 6 closeout prep, coding-pass side)

Дата: 2026-04-26  
Исполнитель: pwm-coding

## Review Scope

### Slice 0 + Slice 1 + Slice 2 + Slice 3 + Slice 4 + Slice 5 + Slice 6

- Зафиксирован перенос roadmap: Sprint 11 = domain migration, Sprint 12 = optimization.
- Созданы sprint-11 baseline артефакты (`checklist`/`status-note`/`review-report`/`test-report`).
- Выполнен Slice 1 coding-pass: runtime semantics закреплен как relay baseline по умолчанию и shard-enforced только в explicit domain mode.
- Legacy `--shard` сохранен как compat path (без hard-break).
- Выполнен Slice 2 coding-pass: CLI/help/config migration contract закреплен как domain-first, `--shard` помечен deprecated compat path с runtime warning при explicit использовании.
- Выполнен Slice 3 coding-pass: mode-bound guard policy закреплен как always-on baseline prefilter + explicit-only shard-enforced local guards.
- Выполнен Slice 4 coding-pass: storage namespace policy переведена на domain-based target в explicit mode с сохранением legacy alias mapping в compat mode.
- Выполнен Slice 5 coding-pass: conformance docs/test baseline синхронизирован с фактическим Sprint 11 runtime behavior; README обновлен под migration policy.
- Выполнен Slice 6 coding-pass closeout prep: consolidated evidence в sprint-11 review artifacts подтверждает финальную согласованность policy и готовность к независимому final testing/review verdict.

## Findings / Verdict

- **Slice 6 closeout prep (coding-pass side):** APPROVE.
- **Primary update:** `pwmd` runtime guard policy синхронизирована с migration policy (default relay baseline; explicit domain => shard-enforced).
- **Contract update:** CLI/help/config формулировки синхронизированы под domain-first контракт; explicit `--shard` теперь дает deprecation warning, alias при этом сохранен (soft-break, без hard removal).
- **Regression fix:** baseline recipient prefilter reject (`400`) восстановлен для invalid recipient classes (`reserve`/`witness`/`unknown`) независимо от shard-enforced режима.
- **Mode-bound evidence:** targeted test path подтверждает, что в explicit mode prefilter остается активным, а shard-enforced reject применим только в explicit mode.
- **Storage policy evidence:** namespace construction строится от runtime identity: explicit mode -> `domain-hi-0xNN` (target), alias mode -> `shard-a|shard-b` (compat mapping).
- **Conformance evidence:** sprint-11 checklist/status/review/test и `README.md` синхронизированы по одной policy формулировке (relay default, explicit shard-enforced, domain namespace target + alias compat mapping).
- **Docs hardening evidence:** финальный docs-pass вычистил устаревший primary UX акцент на `shard A/B`; операторские инструкции в `docs/pwmd.md`, `README.md`, `docs/tester-guide-cli-tui-scenarios.md` закреплены как domain-first с compat-заметкой по `--shard`.
- **Closeout evidence:** checklist/status/review/test обновлены до Slice 6 completion на coding-pass стороне; финальный handoff на независимый verdict зафиксирован.
- Риск/блокер: отсутствуют для Slice 6 closeout prep в рамках заданного scope (без wire/API расширения и без hard-break по `--shard`).

## Final Independent Closeout

- Independent testing (`pwm-testing`) final closeout pass: PASS по policy-critical assertions:
  - relay baseline default behavior,
  - explicit shard-enforced behavior,
  - always-on recipient prefilter,
  - storage namespace domain-target + alias compat mapping.
- Independent review (`pwm-review`) final audit verdict: blocking drift не найден, migration-only scope соблюден.
- **Sprint closeout verdict: SPRINT 11 CLOSED.**

## Baseline Gate Notes (post-ritual)

- Docs gate: sprint-11 checklist/status/review/test приведены к единой структуре `0..6`.
- Scope gate: optimization явно исключен из Sprint 11 и перенесен в Sprint 12.
- Compat gate: hard-break по `--shard` не допускается в рамках Sprint 11.
