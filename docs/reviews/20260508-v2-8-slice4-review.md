# Independent review — V2-8 Slice 4 (epoch catch-up fallback)

**Repo:** PWM-cryptocurrency  
**Coding commit:** `4df23d53a431e02ade502201aaeebc7926aefd06`  
**RFC baseline:** `docs/rfc/15-same-shard-sync-v1.md` (§6.3 epoch catch-up, §8 Anti-DoS, §9 legacy, §10 observability; Slice 4 acceptance в §11)  
**Testing input:** `docs/reviews/20260508-v2-8-slice4-testing.md` — присутствует, результат **PASS** (юниты + check + naming)  
**Reviewer:** pwm-review  

---

## 1. Scope recap

Slice 4 по RFC §11: активация catch-up при пороге отставания, возврат в live после сходимости, ограничения окон/chunk против Anti-DoS (§8), wire-поднабор §6.3.

Дифф `4df23d5`: типы wire (`SyncCatchupReq` с доп. полем `anchor_hash`, `SyncCatchupChunk` / `SyncCatchupChunkWire` с полями связности `first_prev_hash` и `last_hash`, `SyncCatchupDone` с `last_hash`), расширение `SyncPeerState`, логика в `sync_live.rs` (инициатор `maybe_start_cup` / `send_cup_req`; сервер `on_cup_req` с чанкованием; клиент `on_cup_chunk`, `on_cup_done`; hand-off в live через `ask_hdr`), счётчики в `TransportSnapshot`, маршрутизация в `peer_session/mod.rs` с `can_cup`, тесты в `mod.rs` и `wire_decode`.

---

## 2. Requirements fit

**Соответствует заявленной цели slice**

- §6.3: диапазонный запрос, чанки headers+blocks, завершение; расширения wire согласуются с §5.4 для опциональных полей.
- §8: `SYNC_CUP_WIN_CAP == 4096`; чанки ограничены (`SYNC_CUP_CHUNK_CAP` 32 при лимите до 64 блоков на сообщение); на сервере проверки диапазона и эпохи с `SyncNack`; на приёмнике порядок чанков и цепочка заголовков/тел перед `apply_blk_batch` и откатом при ошибке.
- §9: без `full_v1` и `can_cup` catch-up кадры не обрабатываются — drop и метрики; тест `cup_profile_mismatch_noop`.
- §11 Slice 4: триггер по lag (`SYNC_CUP_LAG_MIN`), повтор после `live_stall >= 2`, после успешного `on_cup_done` запрос заголовков для оставшегося хвоста к `tip`.

**Зазоры документации контракта**

- Ответчик **не интерпретирует `anchor_hash`** в запросе; для минимального v1 приемлемо как defer, последствия загрязнения ветки остаются на apply/валидаторе блока.

pwm-testing подтвердил сборку и целевые тесты; multi-node TCP e2e остаётся вне-scope (явно отмечено в testing-отчёте).

---

## 3. Style and module shape

- Структура и `//!` в изменённых модулях согласованы с существующим transport-слоем.
- `python scripts/check_rust_fn_name_segments.py` по путям артефактов slice4: **violations пустые**.

---

## 4. Safety

**Сильные стороны:** откат состояния цепи при ошибке batch-apply; жёсткие проверки чанка; backoff через `cup_next_ms` / `cup_try`; игнорирование live header/block при `cup_active`, чтобы не смешивать очереди.

**Основной остаточный риск (нит):** после того как `send_cup_req` выставил `cup_active`, ответ **`SyncNack`** обрабатывается общим `on_nack`, который **не вызывает `cup_clear`**. Тогда `maybe_start_cup` при дальнейших tip-событиях выходит сразу по «уже активен» и **не инициирует ни новый catch-up, ни live `ask_hdr`**, пока состояние не сброшено иным путём (например переподключение). Аналогичная уязвимость состояния при **`write_wire_msg` → Err** после установки флагов catch-up. Рекомендация для pwm-coding: явный abort catch-up на Nack в контексте активной сессии и откат при ошибке отправки запроса; добавить юнит, фиксирующий снятие `cup_active`.

Полная матрица peer penalty / disconnect по §8.4 — ожидаемо Slice 5.

---

## 5. Tests

- Покрыто: happy path `cup_missing_range_ok`, отказ на битом чанке `cup_bad_chunk_safe`, gate `cup_profile_mismatch_noop`, wire decode для catchup chunk (см. testing-отчёт).
- Пробел, согласованный с нитом выше: нет сценария **SyncNack после выставленного catch-up** и ошибки отправки запроса.

---

## 6. Slice boundary (без перелёта в Slice 5)

Нет полного hardening: централизованные штрафы/дисконнект по всем кодам отказа, полная карта метрик §10 с лейблами — вынесены за пределы этого slice. Пересечения с Slice 5 минимальны и ожидаемы.

---

## 7. Verdict

**approve_with_nits** → бинарно **PASS_WITH_NITS** (критичный для продакшн микрофикс жизненного цикла `cup_active` при Nack/write-fail остаётся рекомендованным follow-up).

---

## 8. Participation / token estimate (machine-copyable)

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts:
  review_md: docs/reviews/20260508-v2-8-slice4-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 9500
  confidence: medium
notes:
  coding_commit: 4df23d53a431e02ade502201aaeebc7926aefd06
  testing_md: docs/reviews/20260508-v2-8-slice4-testing.md
```

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260508-v2-8-slice4-review.md'
git add 'tasks/20260508-v2-sprint8-slice4-epoch-catchup.json'
git commit -m 'docs(slice4): v2-8 epoch catch-up review and ticket close'
```
