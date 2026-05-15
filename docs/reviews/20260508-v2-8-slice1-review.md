# Ревью: V2-8 Slice 1 — wire schema и feature gates (same-shard sync v1)

**Дата:** 2026-05-08  
**Агент:** pwm-review  
**Коммит реализации:** `eb5fc5a` (`feat(pwmd): add same-shard sync v1 wire schema and feature gates`)  
**Тикет:** `tasks/20260508-v2-sprint8-slice1-wire-schema.json`  
**Входы:** `docs/reviews/20260508-v2-8-slice1-testing.md`, `docs/rfc/15-same-shard-sync-v1.md`

---

## 1. Scope recap

Заявленный объём спринта (тикет): каркас wire same-shard sync v1 — типы сообщений на JSON-framed канале пира, расширение hello (`sync_profile`), вычисление режима синхронизации (`FullV1` vs `LegacyObserve`), безопасный stub-маршрутизатор без продовой логики apply.

Фактически в `eb5fc5a` добавлены: константы и структуры handshake (`NodeHelloSyncProfile`, `SyncMode`, лимиты в духе RFC §8), объявление `sync_profile` в локальном hello при dial, семейство sync-вариантов в `PeerWireMsg` (`SyncProfileAnnounce`, заголовочные и блоковые req/batch, `SyncNack`) с общим `SyncWireHdr`, счётчики снапшота `sync_v1_msg_seen_total` / `sync_v1_msg_drop_total`, единый `route_sync_stub` с проверкой shard и режима peer, симметричная обработка на inbound seed path и после initial exchange в seed steady loop, тесты decode и handshake/regression-хелперы.

---

## 2. Requirements fit

**Сильные стороны**

- Совместимость с legacy hello без поля `sync_profile` сохранена (`serde(default)`, тесты `mode_legacy_without_profile`, `decode_legacy_hello_ok`), что согласуется с RFC §9 и ожиданиями смешанной сети на уровне handshake.
- Критерий RFC §11 Slice 1 про **отказ при несовпадении shard**: реализован в `route_sync_stub` через сравнение `hdr.shard_id` с локальным `cluster_domain_hi`, с инкрементом `sync_v1_msg_drop_total` и предупреждающим логом — поведение **наблюдаемо** (метрики снапшота + журнал).
- Gating против `LegacyObserve`: при отсутствии полной v1 возможности удалённые sync-кадры не проходят в «accepted» лог-путь и учитываются в счётчике drop, без паники — согласуется с «не использовать как источник v1 данных» до появления реальной синхронизации.

**Разрывы / частичное покрытие**

- RFC §11 **Slice 1** формулирует принятие как: *«All messages in Section 6 … with required fields»*. В текущем `PeerWireMsg` представлен **поднабор** цепочки sync (headers/blocks/nack плюс wire-level profile announce): **не добавлены** wire-формы mempool-сообщений §6.1 (`TxAnnounce`/`TxRequest`/`TxBatch`), `TipAnnounce`, а также catch-up трио §6.3. Для литерального выполнения критерия §11 это **остаётся зазор**: либо перенести недостающие типы в последующие слайсы и **явно поправить** acceptance в плане/RFC трассируемость, либо достроить их в рамках «wire foundations».

- Одно сообщение **`SyncProfileAnnounce`** поверх уже переговорного `sync_profile` в hello (RFC §5.2 описывает расширение handshake, а §6 даёт taxonomy без отдельного profile announce по каналу). Это возможная **тонкая точка переопределения** протокола: не обязательно ошибка для продукта, но требует явного решения («дублируем профиль для late renegotiation» vs «держим только handshake»).

- Тестирование покрывает минимально `SyncHeadersReq` и legacy hello; **нет прямых тестов** на ветку `shard_id` mismatch и на иные sync-варианты декодирования — риск регрессий ниже среднего (логика тонкая и локальная), но наблюдаемость по метрикам не проверена тестами.

**Вывод по соответствию заявке тикета:** каркас, гейты и legacy — выполнены. **Вывод по дословному RFC §11 Slice 1:** неполное покрытие §6 taxonomy — оформить как отложенную работу или уточнить scope в документе приёмки.

---

## 3. Style and module shape

- `python scripts/check_rust_fn_name_segments.py` для путей из артефакта тикета: **нарушений сегментов имён функций не выявлено** (продакшн ≤ 4, тесты ≤ 5).
- Изменения в `wire.rs`, `peer_session/mod.rs` и смежных модулях выглядят модульно: stub вынесен в одну async-функцию, общий матч паттерном на sync-семейство — понятное продолжение существующего стиля.
- Документирование через `//!` у `wire.rs` присутствует; новые символы в `mod.rs` — приватные вспомогательные — без лишнего раздувания публичного API.

---

## 4. Safety

- Изменений криптопримитивов нет; trust boundary прежний (JSON serde на уровень пира уже существовал).
- `read_wire_msg` сохраняет потолок 1 MiB на кадр — **ужесточение относительно** нормативного максимума 4 MiB из RFC §8 (как верхней границы политики); с точки зрения DoS это безопаснее, но при финальной консолидации лимитов стоит выровнять с конфигурируемой политикой RFC.
- `route_sync_stub` не содержит `unwrap` на горячем пути сообщений; сетевые ошибки по-прежнему пробрасываются существующими путями чтения.
- Риск **дребезга метрик при legacy**: любой ingress sync от peer без профиля увеличивает оба счётчика через seen+drop; это честная телеметрия «шум от не-v1 источников», но операторам понадобится интерпретация.

---

## 5. Tests

Согласовано с `docs/reviews/20260508-v2-8-slice1-testing.md`: `cargo check -p pwmd`, фильтры `decode_`, `handshake`, `transport::` прошли. Полный прогон `cargo test -p pwmd` не выполнялся (зафиксировано pwm-testing как осознанное ограничение).

Пробелы ревью-уровня: нет модульных проверок `shard_mismatch` и большинства sync-вариантов кодирование/декодирование; приёмлемо для каркаса, но улучшает регрессионную матрицу Slice 2+.

---

## 6. Verdict

**PASS-WITH-NITS** (**approve with nits**): реализация корректна как инкрементальный каркас без apply-логики; регрессий по имеющимся автопроверкам не выявлено; критический критерий RFC §11 по `shard_id` mismatch закрыт. Ниты: (1) несовпадение RFC §11 «все сообщения §6» — зафиксировать в плане или достроить; (2) зафиксировать назначение `SyncProfileAnnounce` относительно handshake; (3) добавить узкие тесты shard gate и decode для оставшихся форм при расширении слайса.

По условию тикета **закрытие в `done` только при чистом PASS** — при вердикте PASS-WITH-NITS поле **`status`** в `tasks/20260508-v2-sprint8-slice1-wire-schema.json` оставлено **`in_progress`** до решения владельца или устранения нитов/RFC трассируемости.

---

## 7. Participation / token estimate (machine-copy)

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts:
  - docs/reviews/20260508-v2-8-slice1-review.md
commits_reviewed:
  - eb5fc5a
token_usage:
  source: estimate
  input: null
  output: null
  total: 9500
  confidence: medium
```

---

_End of independent review._
