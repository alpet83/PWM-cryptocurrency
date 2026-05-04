# Sprint 13 Status Note

## Sprint
- ID: Sprint 13
- Theme: Inter-Shard MVP Cut (EXPORT/IMPORT)
- Status: `CLOSED` (Slice 0/1/2/3/4/5/6/7 closed)

## Goal
Выпустить минимально рабочий межшардовый путь `EXPORT -> IMPORT` для MVP-testnet с replay-защитой и воспроизводимым 2-node сценарием.

## In-scope
- `pwm-core`: export/import tx path, deterministic `export_id`, replay guard persistence.
- `pwmd`: runtime/API path для export/import, согласованный статус/error contract.
- `pwm-cli`/`pwm-tui`: минимальный операторский flow и понятные сообщения.
- E2E: 2-node smoke + negative (duplicate import, invalid proof).

## Out-of-scope (locked)
- Advanced policy/admission engine.
- Расширенные roaming optimization/finality профили beyond minimum.
- Нефункциональные крупные рефакторы вне inter-shard path.

## Execution freeze contract (Slice 0)
- Fixed execution plan: 8 slices (`0..7`) без re-scope.
- Acceptance criteria заморожены на уровне inter-shard MVP cut.
- Out-of-scope lock обязателен: без admission/advanced policy.
- Conveyor: реализация через `pwm-coding`, проверка через `pwm-testing`, финальный audit через `pwm-review`.

## Current status (post Slice 7 closeout)
- Slice 0/1/2/3/4/5/6/7 закрыты в рамках freeze-плана (`0..7`) без rescope.
- P0 closure зафиксирован: import provenance guard в `pwm-core` валидирует `export_id` + `to/amount/target_domain` против `exported_registry`; произвольный import material отклоняется (`InvalidImport`).
- P0 closure зафиксирован: snapshot persistence/recovery для replay/provenance включён (`imported_set` + `exported_registry` сериализуются и восстанавливаются в `pwmd` snapshot cycle).
- Duplicate import guard стабильно закрыт через `imported_set` (`DuplicateImport`).
- Slice 3 (`pwmd`): `EXPORT/IMPORT` больше не “теряются” в mempool на RPC boundary — применяются синхронно (`apply_tx` + `seal([])`), ошибки маппятся в HTTP, `IMPORT` prefilter держит ранний provenance/replay контракт, `/v1/status` публикует bridge counters.
- Slice 4/5 closure зафиксирован: минимальный operator UX для inter-shard flow закрыт в `pwm-cli`/`pwm-tui` (базовый `EXPORT/IMPORT` сценарий и понятные статусы/сообщения без расширения policy scope).
- Slice 6 closure зафиксирован: добавлен воспроизводимый automated 2-node smoke `CY -> DO` в `pwmd` test harness с mandatory negative suite (`duplicate import` reject + `unknown export_id` reject) без добавления новых протокольных правил.
- Slice 7 closure зафиксирован: выполнена минимальная stabilization-синхронизация quartet артефактов Sprint 13 и consolidated closeout verdict без добавления нового scope.

## Closeout confirmation
Sprint 13 пакет был передан на финальную независимую проверку и итоговый review; consolidated closeout зафиксирован в closed-state.

Spec cross-links (post-closeout sync):
- `docs/WHITE_SPEC_v0.md` — раздел Sprint 13 MVP cut (baseline / out-of-scope / pitfalls).
- `docs/rfc/9-crossdomain-roaming.md` — сжатый API/runtime/operator контракт текущего roaming MVP.

## Post-cut notes (non-blocking)
- P2: текущий baseline использует синхронный `apply_tx + seal([])` на HTTP пути `POST /v1/tx`; для devnet MVP это принято, но latency/concurrency оптимизации остаются post-cut задачей.
- P2: operator handoff provenance между нодами остаётся manual/runbook-driven в рамках MVP cut и может быть автоматизирован в будущих спринтах вне Sprint 13 scope.

## Post-freeze extension proposals (historical, not part of Sprint 13 freeze)

Freeze-факт Sprint 13 не меняется: закрытие остаётся по Slice `0..7`.

- **Slice 13.8 (proposal): TUI EXPORT assist**
  - Goal: добавить компактный TUI flow для `EXPORT` с явным выводом `export_id` и handoff-пакета.
  - Acceptance: оператор делает `EXPORT` в TUI и копирует полный provenance-пакет без ручной сборки.
  - Risk: UI/валидация могут неочевидно скрыть mismatch до этапа `IMPORT`.

- **Slice 13.9 (proposal): one-window CLI/TUI flow over roaming intents**
  - Goal: сделать cross-domain send единым пользовательским действием через home-shard (`POST /v1/roaming-intents` + lifecycle status).
  - Acceptance: оператор/пользователь видит предсказуемый lifecycle (`queued/exported/relayed/imported/expired/failed`) без ручного split `EXPORT/IMPORT`.
  - Risk: неудачный UX retry может увеличить duplicate-попытки (`409`) и операторскую путаницу.

## Post-freeze extension implementation note (Slice 13.8, outside Sprint 13 freeze)
- Slice 13.8 реализован в backend (`pwmd`) как минимальный federated intent layer поверх Sprint 13 baseline (без изменения freeze-факта по Slice `0..7`).
- Добавлены dual pools: локальный tx mempool сохранён; roaming intents вынесены в отдельный `roaming_pool`.
- Введён lifecycle intents: `queued/exported/relayed/imported/expired/failed`.
- Добавлен TTL по высоте блока (`expires_at_height`) с авто-expire при превышении.
- Включён lock по source funds: при активном roaming-intent конкурирующие локальные tx отклоняются детерминированно (`409 CONFLICT`).
- API расширен минимально: `POST /v1/roaming-intents` (create), `GET /v1/roaming-intents/:id` (status).
- Snapshot persistence/restore теперь сохраняет и восстанавливает roaming intent state + lock-state.

## Post-freeze extension implementation note (Slice 13.9, outside Sprint 13 freeze)
- CLI `tx-send` переведён в one-window flow для cross-domain: home-shard create intent (`POST /v1/roaming-intents`) + lifecycle polling/inspect.
- `tx-export`/`tx-import` сохранены как backward-compatible fallback/debug команды.
- TUI `F6 send` для cross-domain теперь инициирует roaming-intent и показывает lifecycle статусы (`queued/exported/relayed/imported/expired/failed`).
- Error UX в TUI стабилизирован: детерминированные пользовательские сообщения для duplicate/invalid/expired, local send path без регрессии.

