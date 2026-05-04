# Sprint 15 Slice 0: Architecture Freeze Contract

Статус: `frozen`  
Slice ID: `S15-S0-ARCH-FREEZE`  
Назначение: зафиксировать обязательные инварианты перед стартом `S15-S1..S15-S7`.

## 1) Frozen balance semantics contract

Термины обязательны и используются без переопределений:

- `local_state_balance` — локальный баланс адреса в state конкретного шарда/ноды на текущем height; не является автоматически authoritative для foreign-адресов.
- `authoritative_home_balance` — баланс, подтверждённый home-shard адреса (источник истины для адреса), получаемый через валидированный межшардовый контур/доказательство.
- `spendable_on_this_shard` — сумма, которую протокол разрешает тратить на данном шарде в текущем состоянии; не может выводиться из `local_state_balance` для foreign-адреса без явного authoritative подтверждения.

Обязательные правила:

1. Нельзя смешивать `local_state_balance` и `authoritative_home_balance` в одном неразмеченном числовом поле.
2. Для foreign-адреса `local_state_balance` трактуется как local-view-only, не как spendable truth.
3. `spendable_on_this_shard` публикуется отдельно и только по протокольным правилам доступности средств на шарде.

## 2) Frozen cross-shard readiness contract (EXPORT preconditions)

`EXPORT` разрешён только при одновременном выполнении:

1. Source-side preflight выполнен и вернул `ready`.
2. Target подтверждает readiness получателя/импорта по согласованному контракту.
3. Нет terminal-состояний по intent/provenance для этого перевода.
4. Диагностика preflight доступна оператору (причина reject + hint восстановления).

Freshness/binding (обязательно):
- preflight имеет ограниченное окно валидности `readiness_ttl_sec` (по умолчанию 30с, может быть ужесточено в реализации);
- preflight привязывается к intent-контексту (`from`, `to`, `amount`, `target_domain`, `source_height_or_nonce_hint`);
- если к моменту submit изменился любой binding-параметр или истёк TTL, preflight считается недействительным и `EXPORT` должен быть отклонён/пере-проверен;
- запрещено использовать cached readiness вне окна TTL (TOCTOU-guard).

Если любой пункт не выполнен:
- `EXPORT` отклоняется до списания средств;
- возврат — явная ошибка readiness, без silent fallback.

## 3) Frozen genesis/hash join guard contract

Перед join/приёмом пользовательских tx узел обязан:

1. Сверить effective genesis bundle/hash с кластерным ожидаемым значением.
2. Публиковать effective genesis/hash в статус-контракте (`/v1/status` или эквивалент).
3. При mismatch перейти в blocked-состояние для user tx (false healthy запрещён).
4. Поддерживать recovery path: detect -> stop -> fix bundle -> restart verify.

## 4) Frozen snapshot storage contract for S15

Базовый backend:
- `JsonFile` — обязательный baseline по умолчанию, эталон replay-determinism.

Опциональный backend:
- `Db` — только через явный backend selector и только как абстракция над тем же snapshot contract.

Ограничения на `Db` в S15:

1. Никакого неявного переключения default-path с `JsonFile` на `Db`.
2. Одинаковый сериализационный контракт для replay-инвариантов.
3. Невалидный selector -> явная ошибка (без silent fallback).
4. Отказ `Db` не должен приводить к silent auto-fallback внутри одного запроса/операции.
5. Политика отказа `Db`:
   - если runtime запущен с selector=`Db`, ошибка записи/чтения `Db` => явная ошибка commit/read и degraded state;
   - переключение на `JsonFile` делается только явным операторским действием (restart/reconfigure), затем подтверждается status/логом.
6. `JsonFile` baseline остаётся операционно доступным как explicit rollback path, но не как скрытая автозамена.

## 5) In-scope / out-of-scope / no-go boundaries

In-scope для S15:
- Явная семантика балансов, readiness и genesis guardrails.
- Backend abstraction `JsonFile + Db interface`.
- Прототип DB backend и parity-проверки в последующих слайсах.

Out-of-scope:
- Production rollout DB backend.
- Изменение модели консенсуса/финалити.
- Расширение UI/explorer сверх sprint acceptance.

No-go boundaries:
- Нельзя принимать решение go при block-level провале readiness/genesis/replay.
- Нельзя вводить новые трактовки balance-терминов в downstream slices.
- Нельзя убирать `JsonFile` как baseline fallback в S15.

## 6) Acceptance gate для входа в S15-S1

`S15-S1-XSHARD-HARDEN` может стартовать только если:

1. Этот freeze-документ опубликован и имеет статус `frozen`.
2. `tasks/20260429-s15-sprint-kickoff-and-slices.json` обновлён:
   - `S15-S0-ARCH-FREEZE = completed`;
   - `S15-S1-XSHARD-HARDEN = in_progress`;
   - добавлена ссылка на этот артефакт.
3. В kickoff note есть ссылка на freeze как на governing contract.
4. Есть entry evidence pack (минимум 3 пункта, PASS обязателен):
   - `E1`: документирован и проверяем readiness contract с TTL/binding;
   - `E2`: документирован отказной контракт selector=`Db` (no silent fallback);
   - `E3`: документирован и проверяем blocked-state контракт при genesis/hash mismatch.
