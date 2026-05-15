# Divergence guard hotfix — final review gate

**Date:** 2026-05-08  
**Ticket:** `tasks/20260508-consensus-divergence-guard.json`  
**Reviewer:** pwm-review  
**Upstream artifacts:** policy `docs/reviews/20260508-divergence-guard-policy.md`, coding notes in ticket (`4075ba4`), testing `docs/reviews/20260508-divergence-guard-testing.md`

---

## 1. Scope recap

Закрыть операционный safeguard: при наблюдаемых same-sharp расхождениях tip hash на той же высоте — разрыв P2P-сессии, учёт причины закрытия и backoff переподключения порядка 60 s, без ломания путей catch-up при разной высоте и без включения guard в режимах без полного sync v1 / cross-shard. Трассируемость: тикет, политический отчёт, отчёт тестирования; продуктовые изменения в `pwmd` transport (просмотрены только для ревью, без правок).

---

## 2. Requirements fit

**Условие дивергенции (только та же высота + несовпадение hash):** В `sync_live::on_tip` при `lag == 0` (высота пира совпадает с локальным tip) и строковом неравенстве `head_hash` и `hex::encode(local tip)` возвращается `TipDivergence`; иначе при `lag > 0` идёт существующая логика CUP/header pull без ветки disconnect. В `route_sync_stub` реакция на `Some(div)` выполняется только после прохождения гейтов `hdr.shard_id == local_domain_hi` и `full_v1 && same_shard`. Это соответствует политике «same height + hash mismatch only» и не срабатывает на отставание по высоте.

**Close reason:** `PeerCloseReason::SyncTipDivergence` с маппингом в `PeerReconnectReason::SyncTipDivergence` и строкой `sync_tip_divergence` в `peer_close_by_reason` / reconnect counters через `lifecycle::record_peer_close` и `record_reconnect`. Согласовано с политикой наблюдаемости (отличается от предложенных в политике синонимов `tip_hash_mismatch`, но единообразно в enum и метках).

**Cooldown ~60 s:** Используется `cooldown_ms = cfg.reconnect_runaway_cooldown_ms.max(SYNC_TIP_DIVERGENCE_COOLDOWN_MS)` с константой 60 000 ms; для исходящих seed-сессий при наличии `seed_key` вызывается `set_seed_due`, что тестом подтверждается (`>= 59_000` ms от текущего времени). **Зазор относительно формулировки «per peer» в политике:** при **входящем** соединении `route_sync_stub` вызывается с `seed_key: None`, поэтому отметка `next_due_ms` на seed **не** выставляется — сессия всё равно закрывается и метрики/причина фиксируются, но симметричного 60 s backoff на стороне acceptor для того же remote `node_id` в этом hotfix нет. Для типичного сценария «dial к seed» это приемлемо; для строгого паритета inbound/outbound это отложенное улучшение.

**Регрессии по высоте / legacy:** При разной высоте `on_tip` не возвращает divergence; тест `tip_divergence_height_skip` фиксирует отсутствие инкремента `sync_tip_divergence_disconnect_total` и `Continue`. Пока `!full_v1 || !same_shard`, ранний выход с `profile_mismatch` без обработки tip disconnect — guard не активируется.

**Метрики:** Инкремент `sync_tip_divergence_disconnect_total` на пути disconnect; закрытие учитывается в `peer_close_by_reason` с тем же лейблом, что и `PeerCloseReason::as_str()`. Реконнект-метрики получают обновление через существующий lifecycle. Для полноты операторского стека: отдельный интеграционный прогон Prometheus/RPC в этом gate не выполнялся (как зафиксировано в отчёте тестирования).

---

## 3. Style and module shape

Идентификаторы в затронутых файлах проверены скриптом `scripts/check_rust_fn_name_segments.py` по путям transport/peer_session — нарушений лимита сегментов нет. Структура изменений локальна (константа уровня модуля, ветка в router, расширение enum, поле snapshot).

---

## 4. Safety

Разрыв сессии по сигналу расхождения снижает риск «тихого» смешивания контекстов; цена — отсутствие heal через тот же канал при краткой бифуркации, что согласовано с политикой временного safeguard. Паник в добавленном пути не видно; учёт времени через `saturating_add`. Сравнение хэшей как сырых строк: при едином кодировщике (`hex::encode`) честные PWM-пиры согласованы; отклонение от политики §2 (case-insensitive hex) остаётся теоретическим краем для сторонних клиентов с иным регистром — низкий приоритет, но имя в политике явное.

---

## 5. Tests and docs traceability

- Юниты: equal-height mismatch → `Disconnect` + счётчик + backoff seed (как минимум ~59 s); разная высота → нет disconnect по этому правилу.  
- Шире: `pwm-testing` зафиксировал `peer_session::tests` и fmt/bench compile.  
- Пробелы: нет отдельного теста на inbound без `seed_key` (ожидаемо отсутствие `set_seed_due`); нет явного теста на `full_v198` false / only legacy observe в паре с tip announce (частично покрывается общей веткой profile_mismatch).  
- Документы: политика, тест-отчёт, тикет с цепочкой delegations; данный файл — финальный gate.

---

## 6. Verdict

**PASS** — условие дивергенции, гейты full_v1/same-shard/shard_id, close reason и основная метрика disconnect согласованы с политикой и подтверждены тестами; путь «разная высота» не регрессирует. **Не блокирующие замечания:** (1) backoff через `set_seed_due` только при исходящем seed (`Some(seed_key)`), не для чисто inbound; (2) сравнение хэшей без канонизации регистра — при желании выровнять с текстом политики. **Must-fix для merge:** нет.

---

## 7. Participation / token estimate

- `agent`: pwm-review  
- `result`: PASS  
- `artifacts`: `docs/reviews/20260508-divergence-guard-final-review.md`  
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 14000, "confidence": "low" }`

---

## 8. Executive summary (orchestrator)

Hotfix готов к merge с точки зрения заявленного MVP-safeguard: узкий триггер same-height + hash mismatch под full_v1/same-shard, метрики и причина закрытия согласованы, регрессия по высоте покрыта тестом. Иметь в виду асимметрию cooldown inbound vs outbound и опциональную нормализацию hex при следующем касании.

---

## 9. Microfix final gate (commits `39b258a`, `9328415`, `504b71a`)

**Scope recap.** Повторный gate после микрофикса: симметрия inbound cooldown через `seed_key_by_node` / `last_node_id`, расширение `SyncTipAnnounce` опциональным `finalized_hash`, предпочтение устойчивого якоря при `lag == 0` в `sync_live::on_tip`, обновлённая отправка якоря в `send_sync_tip` (penultimate height). Документы и тикет: `9328415`, `504b71a`; трасса тестирования — `docs/reviews/20260508-divergence-guard-testing.md` § Microfix validation.

### Goal 1 — Settled-anchor preference (correctness / safety)

Реализация согласована с заявленной целью: при равной высоте и наличии `finalized_hash` сначала сравнивается хэш блока на высоте `finalized_h` с локальным (`chain_hash_at`); совпадение даёт `Ok(None)` и **не** отрубает сессию при расхождении только «верхушки»; расхождение якоря возвращает `TipDivergence` с полями по высоте якоря. Если якорь с пира недоступен для локального разрешения, сохраняется прежний триггер по mismatch `head_hash` vs локальный tip (как в тесте fallback). Полезная оговорка безопасности: ветка `finalized_hash.is_some() && finalized_h < head_h && local_finalized_h < head_h` перед disconnect по tip **подавляет** разрыв, когда якорь не удалось сравнить (например, локально нет блока на `finalized_h`) — это снижает ложные отключения, но оставляет теоретический край: при повреждённой/урезанной цепочке и mismatch tip можно остаться в `Continue`. Для устойчивых узлов с полной локальной историей тест `tip_divergence_prefers_settled_anchor` покрывает основной сценарий.

### Goal 2 — Inbound cooldown symmetry

`cooldown_seed` = прямой `seed_key` **или** разрешение через `seed_peers` по `last_node_id == node_id`; при inbound с `seed_key: None` тест `tip_divergence_inbound_seed_cooldown` фиксирует `set_seed_due` ≥59 s на нужном seed bucket. Остаточная асимметрия: при отсутствии подходящей записи `seed_peers` (ни одного seed с таким `last_node_id`) cooldown на маркере seed по-прежнему не выставляется — ожидаемо для «чистого» inbound без предшествующего dial к известному seed.

### Goal 3 — Regression (legacy / height lag)

Ветка `lag != 0` в `on_tip` не изменена по структуре; новые аргументы не задействуются вне `lag == 0`. Гейты в `route_sync_stub` и профиль `full_v1` / same-shard прежние. `tip_divergence_height_skip` и расширенный прогон `peer_session::tests` (pwm-testing) снижают риск ложного срабатывания на отстающей высоте. Новое поле wire сериализуется опционально (`serde` default / skip if none) — обратная совместимость на приёме для старых фреймов в рамках заявленного набора `wire_decode` тестов.

### Goal 4 — Metrics / observability

Счётчик `sync_tip_divergence_disconnect_total` по-прежнему инкрементируется только на фактическом пути `TipDivergence` → disconnect в `route_sync_stub`; при «якорь совпал, tip разошёлся» disconnect не выполняется и метрика не растёт — это согласовано с новой семантикой. Причины закрытия / lifecycle не менялись в этом diff; размерность предупреждений по `TipDivergence` (поля высоты/хэша) для якорного кейса отражает высоту якоря, что для операторского разбора даже предпочтительнее.

### Verdict (microfix)

**Итог:** **PASS.** **Must-fix:** нет. **Merge readiness:** готово к merge вместе с цепочкой `39b258a` при условии, что интеграционный стек уже принял pwm-testing PASS по microfix (см. `504b71a` / testing § Microfix validation).

**Nits (не блокируют merge):** (1) край «якорь от пира есть, локально блок на высоте не найден» — осознанный conservative bypass; при появлении pruned режимов стоит переосмыслить; (2) нормализация hex для сравнения по-прежнему вынесена на будущее; (3) при нескольких seed с одним `last_node_id` выбор первого из `find_map` недетерминирован по порядку map (редкий операторский edge).

### Participation / token estimate (microfix gate)

- `agent`: pwm-review  
- `result`: PASS  
- `artifacts`: `docs/reviews/20260508-divergence-guard-final-review.md` §9  
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 9500, "confidence": "low" }`
