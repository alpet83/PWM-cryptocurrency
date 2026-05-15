# Review: V2-8 Slice 5 — observability, chaos validation, operator docs

**Ticket:** `tasks/20260508-v2-sprint8-slice5-observability-chaos-docs.json`  
**Coding commit reviewed:** `9029fb0`  
**RFC baseline:** `docs/rfc/15-same-shard-sync-v1.md`  
**Testing report:** `docs/reviews/20260508-v2-8-slice5-testing.md` — см. секции ниже по согласованию с фактической прогонкой.

## 1. Scope recap

Цель Slice 5 (по тикету и RFC): эксплуатационный слой same-shard sync v1 — метрики и reason-коды, проверки деградации / «хаос», runbook для оператора; согласованность с RFC 15 (в т.ч. storm-guard метрики).

Коммит `9029fb0` затрагивает:

- `TransportSnapshot`: `sync_v1_msg_drop_reason_total`, `mempool_ingress_kind_total`, `mempool_cluster_push_suppressed_total`, `mempool_egress_relay_total`; вспомогательная `add_str_u64_bucket` и рефактор `increment_string_u64_bucket`.
- Приём синх-х кадров: инкремент drop/reason на wire decode errors в `process_inbound_socket` (`decode_failed`, `invalid_frame_len`).
- Mempool ingress/egress: счёт по kind на входном batch, suppression и egress relay при `send_sync_tx_batch`; логи storm-guard.
- Нормализация путей drop: общий helper `add_sync_v1_drop`; для cross-shard в sync-тракте связанный `add_sync_tx_drop` с причиной **`shard_mismatch`** (раньше в этом ветвлении фигурировал `profile_mismatch` для tx-счётчика — логическая правка классификации).
- Интеграционный тест `prod_bad_sync_frame_counted` (битый JSON payload после handshake).
- Внутренние тесты в `peer_session/mod.rs`: `storm_egress_not_blackhole`, усиление проверки `shard_mismatch` по reason-map, reconnect/catch-up сценарии (по диффу — часть уже в файле как хаос-/регресс-проверки).
- Документы: новый `docs/runbook-same-shard-sync-v1.md`; в RFC добавлено MAY-положение про маппинг snapshot-ключей к минимальным метрикам.
- Обновление тикета JSON в том же коммите (delegations pwm-coding, artifacts).

Scope соответствует заявленному Slice 5; лишней функциональной «фичи» вне observability/sync/mempool операционного слоя не видно.

## 2. Requirements fit

**Соответствие RFC:** runbook явно опирается на ключи, перечисленные в RFC (включая ingress/suppression/egress); точечная правка RFC формализует допустимое отображение в transport snapshot без изменения контрактов wire — уместно.

**Operational value:** runbook даёт связный troubleshooting (sync stuck, catch-up, gossip storm/blackhole); метрики и логи закрывают типичные слепы зоны между «соединение есть» и «почему не apply / не egress».

**Пробелы / частичное покрытие:**

- **Регресс в `cargo test -p pwmd peer_session::tests`:** локально подтверждено падение `tx_batch_profile_drop`. Сценарий `route_test(..., full_v1: false)` попадает в ветку `if !full_v1 || !same_shard`, где код инкрементирует **`profile_mismatch`** (`add_sync_tx_drop` в `route_sync_stub`), тогда как тест утверждает ключ **`shard_mismatch`** для `sync_tx_drop_reason_total` — ожидания теста устарели после нормализации reason-кодов (pwm-testing уже зафиксировали как FAIL). Продуктовая логика для legacy здесь выглядит **согласованной с runbook/RFC**; чинить нужно утверждение теста (или разделить кейсы shard vs profile), это одна правка строки подхода pwm-coding.

## 3. Style and module shape

- Именование `fn`/helpers в затронутых путях проверено `python scripts/check_rust_fn_name_segments.py` по заявленным файлам — **violations пустые**.
- Небольшой **дубликат абстракции**: локальный `add_bucket` в `mod.rs` дублирует семантику `metrics::add_str_u64_bucket` — допустимо для микрослайса, но унификация упростила бы сопровождение.

## 4. Safety

- Инкременты счётчиков через saturating arithmetic — без паники на переполнении.
- Корреляция wire ошибок через `contains` подстрок в `sync_wire_reason` — хрупковато к изменению текста ошибок; риск **низкий** для observability (деградация в «нет reason-бакета»), не про безопасность сети.
- Логирование ошибок wire с полным `err` может быть шумным; для dev/diagnostic таргета `pwmd::peer` приемлемо; при включении в боевые уровни стоит контролировать объём полей.

## 5. Tests

pwm-testing сообщили **14 PASS / 1 FAIL** на `peer_session::tests`; независимо проверен `cargo test -p pwmd tx_batch_profile_drop` — **FAIL**: требуется запись **`shard_mismatch`**, карта при этом отражает ветку legacy (**`profile_mismatch`** — см. код `route_sync_stub`).

- `prod_*` подфильтр — зелёный; `prod_bad_sync_frame_counted` закрывает corrupt JSON → `decode_failed`.
- `storm_egress_not_blackhole`, `live_reconnect_sync_no_deadlock`, `sync_shard_drop_noop` — по отчёту pwm-testing проходят.

**Блокер до закрытия слайса:** исправление ожиданий в `tx_batch_profile_drop` (или разделение тестов).

Дополнительно (nit, не блокер этого ревью): `add_sync_tx_drop` всё ещё увеличивает `sync_tx_drop_total` на `count`, а reason-бакет — только на `+1`; это прежнее поведение, но ослабляет интерпретацию «сумма по причинам» для оператора.

## 6. Verdict

**request changes** (по качеству поставки `9029fb0`): наблюдаемость, runbook и правки роутинга reason-кодов выглядят здравыми, но **юнит-набор `peer_session::tests` не зелёный** из‑за устаревшего ключа в `tx_batch_profile_drop`. До тривиальной правки pwm-coding (или эквивалента) считать конвейер слайса закрытым нельзя.

Укороченный код для оркестратора: **FAIL**.

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: FAIL
artifacts:
  - docs/reviews/20260508-v2-8-slice5-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 12000
  confidence: medium
```

---

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260508-v2-8-slice5-review.md'
git add 'tasks/20260508-v2-sprint8-slice5-observability-chaos-docs.json'
git commit -m 'docs(v2-8-s5): pwm-review FAIL — peer_session tx_batch_profile_drop'
```
