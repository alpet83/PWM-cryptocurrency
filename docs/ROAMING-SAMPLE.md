# ROAMING SAMPLE (MVP): one-window + fallback runbook

Короткий практический runbook для Sprint 15 one-window relay.  
Цель: безопасно и воспроизводимо провести междоменный перевод через native/source RPC, где target peer достигается `pwmd` по trusted configured seed. Ручной `EXPORT`/`IMPORT` путь сохранён как fallback/debug.

## 1) Что нужно до старта

- Подняты две ноды с разными `domain_hi` (например, `0x10` и `0x20`).
- Для happy path ноды запущены с `--transport-real` и reciprocal `--transport-peer-seed`, чтобы source мог доверенно доставить handoff на target.
- Известны RPC endpoint'ы:
  - source: `http://127.0.0.1:3030`
  - target: `http://127.0.0.1:3031`
- Подготовлен sender key/wallet и адрес получателя в target-домене.
- Получатель уже выполнил `tx-init` на target-нode; missing/`initialized=false` recipient больше не stub-credit'ится и будет отклонён.
- Понимание:
  - основной пользовательский путь: `tx-send` на native/source RPC -> roaming intent lifecycle -> peer relay через trusted seed;
  - fallback/debug путь: manual `finalize -> tx-handoff-register -> tx-import`, где target уже доверяет source peer через configured seed context.

## 1.1 Быстрый one-window путь (CLI/TUI)

- CLI: `pwm tx-send --wallet ... --to <cross-domain-address> --amount ... --fee ...`
  - внутри выполняется `POST /v1/roaming-intents`;
  - пользователь остаётся на source RPC; target RPC не нужен для happy path;
  - source `pwmd` выбирает target peer по configured seed и `cluster_domain_hi`;
  - CLI печатает intent create + lifecycle polling.
- TUI: `F6 send` на cross-domain адрес
  - выполняет roaming-intent submit;
  - показывает lifecycle статусы `queued/exported/relayed/imported/expired/failed`.

### 1.2 Завершение потока (Sprint 15 — по факту отладки)

- Опрос **`GET /v1/roaming-intents/:id` до статуса `imported` сам по себе не создаёт зачисление на target.** После доставки handoff (`relayed`) клиент должен отправить **`POST /v1/tx` с подписанным телом `Import`**; в типичном сценарии это делает **тот же процесс**, что и roaming-intent (CLI retries / **pwm-tui** автоматически после `relayed`).
- **Источник Import:** запрос уходит на **source RPC** (`PWM_RPC`); `pwmd` на source вызывает **`relay_import`** и пересылает tx HTTP на **target** (`POST /v1/tx` на peer relay base, обычно тот же хост, что и listen API target с учётом `--transport-peer-listen`).
- **pwm-tui:** для nonce/balance проверок получателя и **шага 5** (сверка кредита) может понадобиться **target HTTP** явно: **`PWM_TUI_TARGET_RPC`** (например `http://127.0.0.1:3031`). Если не задан — используется эвристика порта относительно `PWM_RPC` (например `3030` ↔ `3031`). Получатель (`to`) должен быть **в wallet**, чтобы подписать Import.
- **Суммы:** в кредит на домашнем шарде получателя входит **`amount`** экспорта; **`fee`** списывается на source и в зачисление **не входит** (см. строку шага 5 в TUI).

## 2) Manual fallback: `finalize -> register -> import`

### Шаг A. Проверка живости нод

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/status"
Invoke-RestMethod -Uri "http://127.0.0.1:3031/v1/status"
```

Ожидаемо: обе ноды отвечают, `phase=ready`.

### Шаг B. Выполнить `EXPORT` на source

```powershell
cargo run -p pwm-cli --bin pwm -- --rpc http://127.0.0.1:3030 tx-export --help
```

Далее выполните `tx-export` с вашими рабочими параметрами (`to`, `amount`, target domain и signing data).

Ожидаемо:
- `POST /v1/tx` на source возвращает `204 NO_CONTENT`;
- в source появляется export provenance (deterministic `export_id` + контекст).

### Шаг C. Передать import-material оператору target

После source finalize сохраните handoff JSON:

```powershell
Invoke-RestMethod -Method Post -Uri "http://127.0.0.1:3030/v1/roaming-intents/<intent_id>/finalize" |
  ConvertTo-Json -Depth 8 > export-handoff.json
```

Передаётся единый signed handoff:
- `handoff.export_id`;
- provenance-контекст (`to`, `amount`, `target_domain`);
- source-node signature, которую target проверяет перед регистрацией.

Зарегистрируйте handoff на target:

```powershell
cargo run -p pwm-cli --bin pwm -- --rpc http://127.0.0.1:3031 tx-handoff-register --handoff-json export-handoff.json
```

Важно: target-side `tx-handoff-register` не является open/no-seed регистрацией. Target должен уже иметь trusted source peer context из configured outbound seed connectivity; forged/self-attested handoff или один inbound/dev hello отклоняются.

### Шаг D. Выполнить `IMPORT` на target

```powershell
cargo run -p pwm-cli --bin pwm -- --rpc http://127.0.0.1:3031 tx-import --help
```

Далее выполните `tx-import` с теми же provenance-значениями.

Ожидаемо:
- target возвращает `204 NO_CONTENT`;
- импорт считается применённым один раз (replay guard активен).
- если пропустить `tx-handoff-register`, target вернёт `400 invalid import: export_id is not known`.
- если recipient не сделал `tx-init` на target, target вернёт `400` с ошибкой recipient initialization и не изменит `imported_set`.

## 3) Negative suite (обязательный минимум)

### 3.1 Duplicate import -> `409 CONFLICT`

Повторите `tx-import` с тем же `export_id` и тем же payload.

Ожидаемо:
- HTTP `409`;
- состояние не меняется (идемпотентный duplicate reject).

### 3.2 Unknown/invalid provenance -> `400 BAD_REQUEST`

Проверьте оба кейса:
- несуществующий `export_id`;
- mismatch одного из полей (`to`/`amount`/`target_domain`) относительно source provenance.

Ожидаемо:
- HTTP `400`;
- import отклонён.

### 3.3 Прямая cross-domain отправка без roaming-path -> reject

Попытайтесь провести междоменный перевод "напрямую" как обычный путь (без `EXPORT -> IMPORT`).

Ожидаемо:
- операция отклоняется runtime/API;
- оператор возвращается к runbook-сценарию `EXPORT -> IMPORT`.

## 4) Операторский чек-лист (короткая форма)

- [ ] Проверил `status` обеих нод (`phase=ready`).
- [ ] Сделал `EXPORT` на source и получил `204`.
- [ ] Finalize на source вернул signed handoff JSON.
- [ ] Передал handoff target-оператору без ручного редактирования полей.
- [ ] Проверил, что target доверяет source peer через configured seed context.
- [ ] Выполнил `tx-handoff-register` на target.
- [ ] Проверил, что recipient initialized на target (`tx-init` уже выполнен).
- [ ] Сделал `IMPORT` на target и получил `204`.
- [ ] Повторил `IMPORT` для проверки `409`.
- [ ] Проверил invalid/unknown кейс для `400`.
- [ ] Зафиксировал результат прогона в отчёте/чеклисте команды.

## 5) Troubleshooting

### Симптом: `400 BAD_REQUEST` на `IMPORT`
- Частая причина: handoff не зарегистрирован на target, target не доверяет source peer через seed context, перепутан `target_domain` / endpoint target-ноды, либо recipient ещё не initialized на target.
- Действие: проверить reciprocal seed/peer status, выполнить `tx-handoff-register` на target, сверить source provenance с фактическим RPC target и выполнить `tx-init` для recipient на target.

### Симптом: `409 CONFLICT` на первом, как кажется, `IMPORT`
- Частая причина: этот `export_id` уже импортировали ранее (или был автоповтор скрипта).
- Действие: считать это duplicate reject; проверить журнал попыток и взять новый `EXPORT`.

### Симптом: "не работает cross-domain send напрямую"
- Причина: прямой `TRANSFER` между domain_hi не является roaming-flow.
- Действие: использовать one-window `tx-send` на source RPC; manual `EXPORT/IMPORT` оставлять для fallback/debug.

### Симптом: операторы расходятся в значениях import-material
- Причина: ручной handoff искажил поля.
- Действие: повторно снять provenance на source, передавать поля как единый пакет без ручного редактирования.
