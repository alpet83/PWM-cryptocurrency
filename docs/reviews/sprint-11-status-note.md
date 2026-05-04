# Sprint 11 Status Note

Дата: 2026-04-26  
Этап: Sprint 11 closeout completed (final independent testing/review accepted)  
Статус: **SPRINT 11 CLOSED — HANDOFF TO SPRINT 12 (FINAL OPTIMIZATION)**

## Current State

- Sprint 11 закреплен как migration-трек: `domain_hi` + relay-by-default.
- Legacy `--shard A|B` закреплен как deprecated compat alias (soft-break, warning, без removal в этом спринте).
- Optimization формально вынесен в Sprint 12.
- Checklist переведен из draft в execution-документ со структурой slices `0..6`.

## Slice Progress

- `Slice 0/6`: завершен (scope freeze + pre-task completion notes + global gates).
- `Slice 1/6`: completed (runtime semantics зафиксирован в коде/CLI/help: relay baseline = default; shard-enforced guards только при explicit domain config).
- `Slice 2/6`: completed (domain-first CLI/help/config contract, explicit `--shard` deprecation warning, soft-break compat preserved).
- `Slice 3/6`: completed (mode-bound guard policy: baseline recipient prefilter always-on; shard-enforced local guards only in explicit mode, подтверждено targeted checks).
- `Slice 4/6`: completed (storage namespace policy: explicit identity uses domain-based namespace target, alias mode keeps deterministic legacy mapping `shard-a|shard-b`).
- `Slice 5/6`: completed (conformance docs/test baseline синхронизирован с фактической runtime policy Sprint 11; README drift закрыт).
- `Slice 6/6`: completed (coding-pass closeout prep + independent testing/review closeout pass accepted).

## Active Gates Snapshot

- Policy gate: relay mode = default (зафиксировано).
- Domain gate: shard-support only via explicit domain config (зафиксировано).
- Compat gate: `--shard` = deprecated alias (зафиксировано).
- Scope gate: optimization не входит в Sprint 11 (зафиксировано).
- Artifact gate: checklist/status/review/test синхронизированы с `N=6` и единым phase naming.

## Final Closeout Verdict

- Independent `pwm-testing` final closeout pass: PASS по policy-critical assertions и sanity subset.
- Independent `pwm-review` final verdict: **SPRINT 11 CLOSED** (blocking drift не найден).
- Sprint 11 migration scope закрыт; следующий этап roadmap — Sprint 12 optimization.

## Guardrails

- Без product-code изменений на этапе ритуала.
- Без wire/API drift в `pwmd`.
- Любая двусмысленность `relay` vs `shard-enforced` устраняется в docs до coding-pass.

## Slice 1 Coding Evidence

- `--shard` сохранен как legacy compat selector (soft-break policy без hard removal).
- `--cluster-domain-hi`/`--domain-cluster`/`--domain_cluster` формализован как вход в explicit domain semantics.
- Runtime guard-поведение: shard-enforced проверки (`/v1/tx` local guards) выполняются только в explicit mode; alias/baseline режим остается relay-compatible.
- Закрыт блокирующий regression по recipient prefilter: invalid recipient classes (`reserve`/`witness`/`unknown`) снова возвращают `400 BAD_REQUEST` в `/v1/tx` независимо от режима.

## Slice 3 Coding Evidence

- Runtime policy в `/v1/tx` зафиксирована как mode-bound:
  - baseline recipient prefilter вызывается независимо от режима;
  - shard-enforced local guards вызываются только в explicit mode.
- Targeted checks подтверждают обе стороны policy:
  - explicit mode отклоняет wrong-shard sender (`CONFLICT`);
  - relay baseline допускает same tx-path без shard-enforced reject;
  - explicit mode сохраняет `400 BAD_REQUEST` prefilter reject для invalid recipient class.

## Slice 4 Coding Evidence

- Введен единый runtime helper namespace construction для storage path:
  - explicit mode -> domain target (`domain-hi-0xNN`);
  - alias mode -> legacy compat mapping (`shard-a|shard-b`).
- Default `data_file` path теперь строится от effective runtime identity (после identity resolution), без wire/API расширения.
- `/v1/status` и startup log (`state_ns=...`) синхронизированы с новой policy и показывают effective namespace.
- Backward-compat по alias path сохранен: legacy `--shard` сценарий остается на `shard-a|shard-b` без hard-break.

## Slice 2 Coding Evidence

- CLI/help wording зафиксирован как domain-first контракт: primary вход через explicit identity tuple (`--network-id`, `--cluster-domain-hi`, `--cluster-id`, `--node-id`).
- Legacy `--shard` явно помечен как deprecated compat path (soft-break policy, без hard removal в Sprint 11).
- Добавлен runtime warning при explicit использовании `--shard` (для операторского deprecation сигнала без поведенческого hard-break).
