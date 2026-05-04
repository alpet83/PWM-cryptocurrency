# Sprint 15 S3.3 live diagnostics (active nodes 3030/3031)

Дата: 2026-04-29  
Контур: live node source `http://127.0.0.1:3030`, target `http://127.0.0.1:3031`  
Входы: wallet `tmp/genesis.yaml`, passphrase `1234`, amount `0.1 PWM = 100000 raw`

## Итоговый вердикт

**FAIL**

Причина: сквозной цикл `preflight -> export -> finalize(relayed) -> handoff register -> import` на активных нодах не завершён. На source intent уже в terminal-статусе `expired`, `finalize` не переводит в `relayed`, target отклоняет handoff register.

## Жёсткие доказательства по этапам

1) **Readiness**
- `GET /v1/status` source: `phase=ready`, `ready=true`, `shard=CY`.
- `GET /v1/status` target: `phase=ready`, `ready=true`, `shard=DO`.
- Оба отвечают, но genesis-хэши отличаются:
  - source `effective_genesis_hash=9ab080cbfc8a...5453d`
  - target `effective_genesis_hash=678c973671ef...03f46`

2) **Export / intent created**
- На source `GET /v1/flow/recent` есть цепочка для `export_id=intent_id=24d94280...77e55`:
  - `checked:export_readiness:export`
  - `accepted:export` (roaming intent created)
  - `applied:export`, `exported:export`, `roaming_status:export`, `sealed:export`
- Это подтверждает, что preflight+submit в live-контуре действительно выполнялись.

3) **Finalize / handoff availability**
- `GET /v1/roaming-intents/24d94280...77e55`:
  - `status=expired`
  - `last_error="intent ttl exceeded at current height"`
- `POST /v1/roaming-intents/24d94280...77e55/finalize`:
  - `status=expired`
  - `changed=false`
  - `message="intent expired before finalize; create a new roaming intent"`
  - handoff JSON получен, но с `handoff.status=expired` (не `relayed`).

4) **Provenance register on target**
- `pwm --rpc http://127.0.0.1:3031 tx-handoff-register --handoff-json tmp/s15-s3-3-handoff.json`
- Ответ: `HTTP 400 Bad Request`
- Текст: `export handoff must be finalized with status=relayed`

5) **Import submit**
- Попытка import с тем же `export_id` не дошла до успешного `204`:
  - `pwm --rpc http://127.0.0.1:3031 tx-import ... --export-id 24d94280...77e55`
  - Ошибка: `HTTP 409 Conflict: tx sender domain_hi=0x2C does not match node domain_hi=0x32`
- На target не наблюдается реакция import lifecycle:
  - `GET /v1/flow/recent` на target -> `rows: []`
  - `bridge_imported_set_size` остаётся `0`

## Проверка запуска текущих нод (из активных terminal outputs)

Найдены несоответствия, которые влияют на relay/согласованность окружения:

1) **Разный genesis у source/target**
- Source стартовал с `--genesis-file ./tmp/genesis-custom.json --genesis-passphrase 12345`.
- Target в текущем старте без этого набора аргументов (по факту другой genesis hash).
- Подтверждение: `effective_genesis_hash` различается между 3030 и 3031.

2) **Параметры запуска не совпадают с обязательными входами задачи**
- Требовалось: wallet `tmp/genesis.yaml` + pass `1234`.
- Фактический live startup source использует genesis/passphrase другого профиля (`genesis-custom.json` + `12345`) из terminal history.

3) **Шум/склейка команд в terminal metadata**
- В `active_command/last_command` есть склеенные фрагменты (`...3030al ...`, повторы флагов), что повышает риск неявного дрейфа профиля запуска и операционных ошибок.

## Классификация корневой причины

**Primary:** operator sequence gap.  
Intent не был финализирован до TTL (`status=expired`), поэтому handoff-register на target закономерно отклоняется.

**Secondary:** transport/relay connectivity/config mismatch.  
У source и target различный genesis hash и разный startup profile; это ухудшает межнодовую согласованность и диагностику real-transport.

**Runtime bug:** не подтверждён текущими данными.  
Наблюдаемое поведение соответствует контракту API (`expired`, reject non-relayed handoff, policy rejects).

## Операторский чек-лист для стабильного прогона

1. Запустить обе ноды в одном профиле genesis/network:
   - одинаковый `--network-id`
   - один и тот же genesis bundle/hash
   - корректные `--transport-real --transport-peer-seed` в обе стороны.
2. Проверить health перед экспортом:
   - `GET /v1/status` обеих нод (`ready=true`, одинаковый `effective_genesis_hash`),
   - при доступности dev-профиля проверить peer handshake (`/v1/dev/peers`).
3. Выполнить source preflight для финального payload (`/v1/export-readiness`), затем сразу submit export/intent (без пауз).
4. Сразу после submit вызвать source finalize и убедиться, что `status=relayed` (не `expired`).
5. На target выполнить `tx-handoff-register` ровно тем handoff JSON, который вернул finalize.
6. Убедиться, что target recipient initialized (`GET /v1/account/<to>` -> `initialized=true`).
7. Выполнить `tx-import` с target signer/domain (DO wallet на target RPC), ожидать `204`.
8. Проверить пост-условия:
   - source intent lifecycle: terminal status без `expired` на critical пути,
   - target `bridge_imported_set_size` увеличился,
   - target `/v1/flow/recent` содержит import-событие.

## Быстрые health-check команды

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/status"
Invoke-RestMethod -Uri "http://127.0.0.1:3031/v1/status"
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/flow/recent"
Invoke-RestMethod -Uri "http://127.0.0.1:3031/v1/flow/recent"
```

```powershell
cargo run -p pwm-cli -- --rpc http://127.0.0.1:3031 tx-handoff-register --handoff-json tmp/s15-s3-3-handoff.json
```
