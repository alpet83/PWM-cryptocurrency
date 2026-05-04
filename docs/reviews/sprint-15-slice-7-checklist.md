# Sprint 15 — Slice 7: wave checklist (pre-implementation gates)

Отмечать `[x]` только после соответствующего gate (`pwm-testing`/`pwm-review`) и фиксации артефакта в тикете.

## Wave 0 — Design lock

- [ ] Зафиксирован контракт `1 row = 1 block` для CH, без full-chain blob insert.
- [ ] Зафиксирован checkpoint cadence `100`.
- [ ] Зафиксирован контракт `DB per network + table per cluster/domain`.
- [ ] Утверждён минимум полей `blocks__0xHH` и `checkpoints__0xHH`.
- [ ] Утверждён формат `validators_accept` (append-only).
- [ ] Утверждён deterministic `checkpoint_digest` для подписей.
- [ ] Утверждён bootstrap/replay алгоритм `checkpoint + tail replay`.
- [ ] `pwm-review` по design lock: PASS/PARTIAL с закрытыми HIGH-замечаниями.

## Wave 1 — Json epochs + fallback

- [ ] Добавлен epoch формат `block_e{num}.json` (1000 блоков на эпоху).
- [ ] `pwm-data.json` переведён в summary/state формат (без полного `blocks[]`).
- [ ] Сохранён fallback load legacy snapshot (`blocks[]` внутри `pwm-data.json`).
- [ ] Реализован atomic publish (`tmp -> fsync -> rename -> manifest update`).
- [ ] Реализован recovery после краша (`.tmp` cleanup + range continuity check).
- [ ] `pwm-testing`: unit/integration тесты epochs + fallback + crash recovery.
- [ ] `pwm-review`: нет критических рисков по atomicity/corruption path.

## Wave 2 — Memory bound and runtime correctness

- [ ] In-memory cache блоков ограничен `<=1000`.
- [ ] Введены отдельные `canonical_height`/`tip_hash` (не от `len(cache)`).
- [ ] Seal/producer rotation/prev-hash flow остаются корректными после eviction.
- [ ] API/runtime, читающие head/tip, работают без регрессий.
- [ ] `pwm-testing`: regression suite на высотах `>1000` проходит.
- [ ] Bench: RSS/latency для bounded cache зафиксированы.

## Wave 3 — ClickHouse incremental path

- [ ] Реализован per-block insert в `blocks__0xHH`.
- [ ] Реализован checkpoint insert каждые `100` блоков в `checkpoints__0xHH`.
- [ ] Реализован pre-write хвостовой consistency check (до 99 блоков).
- [ ] При mismatch блок/checkpoint не пишутся, событие уходит в diagnostics.
- [ ] Реализована таблица `validators_accept` и запись подписи checkpoint digest.
- [ ] Убрана зависимость от full-chain overwrite path.
- [ ] `pwm-testing`: integration тесты single-node + multi-node same-shard.
- [ ] `pwm-review`: подтверждена отсутствие O(H^2 x N) path в runtime.

## Wave 4 — Benchmarks, explorer fields, closeout

- [ ] Вынесены first-class поля для explorer минимум: `tx_count`.
- [ ] Для `shard_balance` зафиксирована формула и момент расчёта (checkpoint-level).
- [ ] Benchmark report: old snapshot path vs incremental JSON/CH path.
- [ ] Benchmark report: cold start (`checkpoint + replay tail`) vs full replay baseline.
- [ ] Benchmark report: CH write pressure (parts/latency) в multi-node сценарии.
- [ ] Обновлены docs/runbook по новым storage contracts.
- [ ] Финальный `pwm-review` на интегрированный дифф.
- [ ] Тикет Slice 7 переведён в `done_conveyor`.

## Обязательные стоп-условия (не продолжать волну)

- [ ] Если `canonical_height` связан с длиной bounded cache.
- [ ] Если checkpoint подписывается по недетерминированному JSON.
- [ ] Если bootstrap продолжает работу при обнаруженной дыре epoch range.
- [ ] Если CH path снова пишет full-chain blob per block.
