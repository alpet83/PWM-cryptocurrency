# Sprint 15: Cross-shard user scenarios review

## Итог расследования

Ошибка
`409 missing_preflight`
в твоем кейсе — это **ожидаемое поведение backend** после `S15-S1`.

Причина: `pwmd` теперь fail-closed и требует обязательный `POST /v1/export-readiness` перед отправкой `EXPORT`.

Проблема сейчас не в `pwmd`, а в интеграции: текущие потоки `TUI/CLI` для cross-shard отправки могут идти сразу в `/v1/roaming-intents` без preflight.

Классификация:
- backend: **expected**
- user flow (TUI/CLI): **integration/docs/UX gap**

## Что означает каждая типовая ошибка

- `missing_preflight`  
  Экспорт отправлен без preflight.  
  Что делать: выполнить `/v1/export-readiness` для этого же payload и сразу повторить submit.

- `stale_preflight`  
  Preflight протух по TTL.  
  Что делать: сделать preflight заново и сразу отправить.

- `binding_mismatch`  
  Payload изменился после preflight (`to/amount/domain`).  
  Что делать: зафиксировать payload, заново preflight, потом submit.

- `nonce_mismatch` / `height_mismatch`  
  Состояние source изменилось после preflight.  
  Что делать: пересобрать экспорт с актуальными nonce/контекстом, снова preflight.

- `recipient account not found` / `recipient account not initialized`  
  На target получатель не инициализирован.  
  Что делать: сначала `tx-init` на target, затем import flow.

- `invalid import: export_id is not known`  
  На target нет зарегистрированного handoff/provenance.  
  Что делать: `tx-handoff-register` (или `POST /v1/export-provenance`), затем `tx-import`.

- `invalid import: export provenance mismatch`  
  Параметры import не совпадают с provenance.  
  Что делать: использовать поля строго из handoff.

- `duplicate import: export_id already consumed`  
  Этот export уже импортирован.  
  Что делать: не ретраить этот `export_id`.

- `503 ... genesis/hash mismatch`  
  Сработал genesis guard, user tx заблокированы.  
  Что делать: выровнять genesis bundle/hash между нодами, перезапустить, проверить `/v1/status`.

## Минимальный рабочий сценарий (сейчас)

1. Source: сделать `POST /v1/export-readiness` для финального EXPORT payload.
2. Source: сразу отправить EXPORT/roaming intent тем же payload.
3. Target: зарегистрировать handoff (`tx-handoff-register` или `/v1/export-provenance`).
4. Target: выполнить `tx-import`.
5. Проверить статус/историю.

## Почему у тебя вышло именно так

- Тест 1 (без инициализации получателя) заблокировался корректно — это отдельный guard получателя.
- Тест 2 (с инициализацией) дошел дальше и упал на новом обязательном guard `missing_preflight`.

## Рекомендация

Приоритетно в следующих доработках: довести TUI/CLI cross-shard flow до автопрефлайта (или очень явного шага preflight в UX), чтобы пользователь не ловил `409 missing_preflight` в обычном режиме.
