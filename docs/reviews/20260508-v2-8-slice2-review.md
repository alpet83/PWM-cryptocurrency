# Ревью: V2-8 Slice 2 — native mempool gossip baseline (`SyncTx*`)

**Дата:** 2026-05-08  
**Агент:** pwm-review  
**Коммит реализации:** `74a14f9` (`feat(pwmd): add native mempool gossip baseline for sync v1`)  
**Тикет:** `tasks/20260508-v2-sprint8-slice2-mempool-gossip.json`  
**Входы:** `docs/reviews/20260508-v2-8-slice2-testing.md`, `docs/rfc/15-same-shard-sync-v1.md` (§4.1 mempool gossip, §11 deferring §6.1 mempool subset to Slice 2).

---

## 1. Scope recap

Заявленная цель тикета: baseline gossip pending tx между native peers с dedup, rate-limit и legacy-safe поведением на wire `SyncTx*`, без выхода за рамки Slice 2.

По диффу `74a14f9` затронуты: `peer_session` (inbound + seed steady + маршрутизация sync-кадров), `handshake_state` (`MempoolGspState`), метрики transport, `Mpool::snapshot` в `pwm-core`, плюс harness/wire строки. Отдельного внедрения `TipAnnounce`, catch-up §6.3 или полной live header/block матрицы Slice 3/4 в этом коммите нет; `SyncTxAnnounce` / `SyncTxReq` намеренно считаются unsupported (счётчик `unsupported`), что согласуется с узким baseline «только batch».

---

## 2. Requirements fit

**Соответствие RFC и плану слайса.**

- Ingest `SyncTxBatch` при `full_v1` + same-shard: mempool пополняется после цепочки проверок (`validate_tx_shape`, `precheck_apply_with_ctx`, `pool.push`) — покрыто тестом `tx_batch_valid_in`.
- Дедуп по идентификатору транзакции (hex от `tx_hash()`), скользящее окно по времени, prune при обработке — покрыто `tx_batch_dup_drop`.
- Входной cap: батч целиком отбрасывается при превышении `min(cap_hint, SYNC_TX_IN_CAP)` — логика есть; отдельного unit-теста на ветку `rate_limit` в коммите нет (как уже отмечено pwm-testing).
- Исходящий relay: снимок пула (до `SYNC_TX_SCAN_CAP`), выбор до `min(profile.max_txs_per_msg, SYNC_TX_OUT_CAP)`, локальный per-peer dedup отправки за окно — согласуется с «bounded relay» на уровне кода; отдельного harness-теста на исходящий путь нет.
- Gating: `send_sync_tx_batch` не шлёт кросс-шард и без `supports_sync_v1` / нулевого `max_txs_per_msg`; `route_sync_stub` отсекает `shard_id` mismatch и отсутствие `full_v1`/same-shard — покрыто `tx_batch_profile_drop`.
- Legacy: для non-tx sync-кадра при `full_v1 == false` считаются v1 sync метрики, но **не** инкрементируются `sync_tx_*` — покрыто `legacy_sync_hdr_safe` (отсутствие ложного учёта mempool-метрик).

**Пробелы (не блокирующие baseline, но зафиксированы):** нет e2e-доказательства исходящего батча в тестах; announce/request отложены на последующие итерации RFC.

---

## 3. Style and module shape

- Именование: независимый прогон `python scripts/check_rust_fn_name_segments.py` по путям артефакта тикета — **violations: []** (политика prod ≤4 / test ≤5 сегментов соблюдена на проверенных файлах).
- Крупный блок логики сосредоточен в `peer_session/mod.rs`; для слайса это ожидаемо, но дальнейшая декомпозиция (отдельный модуль mempool-gossip) может упростить сопровождение — рекомендация на будущее, не как дефект Slice 2.
- Комментарии на английском в новых `//!`/`///` — выборочно ок; модуль уже имел англоязычный баннер.

---

## 4. Safety

**Положительное.**

- Нет паник в просмотренном пути: ошибки цепи/mempool уходят в счётчик `invalid`.
- Порядок удержания блокировок: pwm-testing корректно отмечает отсутствие взаимной блокировки `inner` vs `handshake` в `ingest_sync_tx_batch` / `send_sync_tx_batch`; самостоятельно подтверждено по чтению кода.
- Входной размер батча ограничен; исходящий — двойным cap (профиль + константа).
- JSON wire + длина кадра ограничены существующим фреймингом (как и раньше для sync-сообщений).

**Замечания (низкий/средний приоритет).**

- Дедуп-карта обновляется **до** `validate_tx_shape` / precheck: уникальный идентификатор для заведомо невалидной транзакции остаётся в `tx_seen_ms` до prune. Это снижает повторную обработку мусора с тем же хэшем и укладывается в окно 30s; при намеренном спаме уникальными невалидными телами нагрузка на `HashMap` ограничена горизонтом prune, но **нет отдельного per-peer rate limit на частоту батчей** — остаётся общий класс DoS-риска транспорта, не специфичный только этому слайсу.
- При `shard_id` mismatch для tx-кадра в `sync_tx_drop_reason_total` учитывается причина `profile_mismatch` (через ту же ветку, что и профиль). Семантика для оператора может путать с реальным «не тот профиль v1»; это **чистая наблюдаемость**, не логика приёма.

---

## 5. Tests

 pwm-testing: `cargo check -p pwmd`, `peer_session::tests` (четыре целевых теста), `pwm-core mempool::`, name-segment gate — **PASS**. Полный `pwmd --lib`: зафиксирован **флак** `slice20_dual_flow_ok` (403 trust); повтор проходит — по смыслу ошибки не выглядит регрессией `SyncTx*`, но остаётся техдолгом стабилизации вне Slice 2.

Пробелы ревью: нет теста на жёсткий inbound `rate_limit`; нет автоматического теста исходящего `send_sync_tx_batch`.

---

## 6. Verdict

**PASS_WITH_NITS** — реализация baseline mempool gossip и gating соответствует заявленному scope Slice 2, без признаков захвата Slice 3/4; блокирующих дефектов безопасности в разборе не выявлено. Ниты: метрика `profile_mismatch` при shard mismatch, отсутствие unit-тестов на cap/outbound, флак e2e вне узкого scope.

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
verdict_human: PASS_WITH_NITS
artifacts:
  - docs/reviews/20260508-v2-8-slice2-review.md
evidence:
  - git:74a14f9
  - scripts/check_rust_fn_name_segments.py on ticket paths: violations []
  - read-only code review peer_session/mod.rs, inbound, steady_session, handshake_state, mempool snapshot
token_usage:
  source: estimate
  input: null
  output: null
  total: 14000
  confidence: low
```

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260508-v2-8-slice2-review.md'
git add 'tasks/20260508-v2-sprint8-slice2-mempool-gossip.json'
git commit -m 'docs(v2-8): Slice 2 mempool gossip review and ticket close'
```

---

**Verdict (one line):** PASS_WITH_NITS
