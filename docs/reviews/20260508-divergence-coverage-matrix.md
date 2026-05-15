# Матрица покрытия: обнаружение расхождения локальная цепочка ↔ пир (MVP)

**Дата:** 2026-05-08  
**Тикет:** `tasks/20260508-consensus-divergence-guard.json`  
**Агент:** `pwm-review` (только анализ доказательств в коде/тестах/доках; без нового поведения)

## Терминология (важно)

В коде **нет** отдельных флагов `SelfDivergence` / `ForeignDivergence`. Практически релевантные смыслы:

| Обозначение в запросе | Что в репозитории |
|----------------------|-------------------|
| «self» (локальный вид) | Локальный `tip_h` / `tip_hash` и производные (penultimate finalized anchor в `send_sync_tip` / `on_tip`) |
| «foreign» (удалённый вид) | Поля из `SyncTipAnnounce` и цепочка реакций от **конкретного** `node_id` пира |
| `PeerClass::Foreign` | **Другой** `cluster.domain_hi` (кросс-шард): это **не** та же семантика, что guard same-shard tip divergence |

Основной safeguard расхождения канона на том же shard: **`TipDivergence`** → `PeerCloseReason::SyncTipDivergence` (см. `sync_live.rs` + `route_sync_stub`).

---

## Сводная матрица по фазам

### Фаза 1 — initial connection / handshake (genesis proximity, совместимость)

| Ожидаемая проверка | Где реализовано | PASS / GAP | Риск |
|-------------------|-----------------|------------|------|
| **network_id** совпадает с локальным ожиданием | `validate_node_hello` (`handshake.rs`); pre-TCP seed: `/v1/status` в `attempt_seed_connect` (`dial.rs`) | **PASS** | Низкий |
| **genesis_hash** совпадает (если у узла задан ожидаемый genesis) | `validate_node_hello`; inbound журналирование `genesis_guard`; outbound: сравнение с `effective_genesis_hash` seed status (`dial.rs`) | **PASS** | Низкий |
| **Подпись** hello, целостность полей | `verify_signature`, `validate_mandatory_fields` (`handshake.rs`); exercised в `handshake.rs` tests (`hello_reject_bad_sig`, …) | **PASS** | Низкий |
| **replay nonce** окно | `ReplayNonceCache` в `validate_node_hello`; тест `hello_reject_replay_nonce` | **PASS** | Низкий |
| **timestamp skew** | `validate_node_hello`; тест `hello_reject_time_skew` | **PASS** | Низкий |
| **Same-shard federation / bridge digest** при доверенном режиме | `process_incoming_peer_hello`: для `same_shard` и наличии `expected_bridge_commitment` — обязательное совпадение `bridge_commitment` (`incoming_hello.rs`) | **PASS** (условный: только если включён путь bridge trust) | Средний, если оператор ошибётся с флагами доверия |
| **Shard / домен**: классификация native vs foreign peer | `classify_peer(local_domain_hi, hello.cluster.domain_hi)` после успешной валидации (`incoming_hello.rs`) | **PASS** как маршрутизация класса | Низкий |
| **capabilities / sync_profile** на этапе hello | Структура и правила **FullV1**: `supports_sync_v1` (`handshake.rs`); отправка профиля в `build_local_node_hello` (`dial.rs`) | **PASS** как «есть ли full_v1» | Низкий |
| **Строгое совпадение cluster_id** с локальным при handshake | Явной проверки `cluster_id == local cluster_id` в `validate_node_hello` / `incoming_hello` **не найдено** (`rg` по `expected_cluster` пусто) | **GAP** | Средний: при одинаковом `network_id`+`genesis` но ином кластере соединение всё же может считаться валидным на уровне hello |
| **Совместимость protocol_version / tx_features / services** с локальной политикой | Поля должны быть непустыми строками без мусора (`validate_mandatory_fields`); **нет** матрицы «локально поддерживаем X → пир обязан Y» | **GAP** | Низкий–средний для MVP |

**Доказательства:** `handshake.rs` (`validate_node_hello`, юниты `hello_*`), `incoming_hello.rs`, `dial.rs` (`attempt_seed_connect`, `PeerHelloAck`), политика в `docs/reviews/20260508-divergence-guard-policy.md` §2–§3.

---

### Фаза 2 — reconnect / backoff после детекта

| Ожидаемое поведение | Где реализовано | PASS / GAP | Риск |
|--------------------|-----------------|------------|------|
| Close reason трассируется в reconnect reason | `reconnect_from_close`: `SyncTipDivergence` → `PeerReconnectReason::SyncTipDivergence` (`lifecycle.rs`) | **PASS** | Низкий |
| Пер-seed/per identity cooldown после divergence disconnect | После детекта: `set_seed_due(..., now_ms + max(reconnect_runaway_cooldown_ms, SYNC_TIP_DIVERGENCE_COOLDOWN_MS))`; inbound fallback `seed_key_by_node` (`peer_session/mod.rs`, константа `SYNC_TIP_DIVERGENCE_COOLDOWN_MS`) | **PASS** | Низкий (ост. nits из финального ревью: дубликаты `last_node_id`, см. тикет) |
| Общая защита от reconnect storm | `reconnect_runaway_streak`, guard window (`transport_tick.rs`, метрики `reconnect_runaway_*`) | **PASS** как классический runaway слой | Низкий |
| Отдельная политика backoff для класса Foreign vs Native | `select_backoff_for_class`, transport tick ordering (`policy.rs`, `transport_tick.rs`), тесты `transport_peer.rs` | **PASS** как dial/backoff, **ортогонально** tip-divergence | — |

**Доказательства:** тесты `tip_divergence_disconnect_marks_backoff`, `tip_divergence_inbound_seed_cooldown` в `peer_session/mod.rs`; делегирование в `tasks/20260508-consensus-divergence-guard.json` (pwm-testing notes).

---

### Фаза 3 — runtime steady sync (tip / finalized / catch-up)

| Ожидаемая проверка | Где реализовано | PASS / GAP | Риск |
|-------------------|-----------------|------------|------|
| Отсутствие обработки sync с **чужим** `shard_id` в заголовках кадров | `route_sync_stub`: `hdr.shard_id != local_domain_hi` → drop + счётчики (`peer_session/mod.rs`); тест `sync_shard_drop_noop` | **PASS** | Низкий |
| Только **FullV1 + same_shard session** принимает live sync обработчик | Тот же `route_sync_stub`: `!full_v1 \|\| !same_shard` → `profile_mismatch` (`peer_session/mod.rs`) | **PASS** | Низкий |
| Сравнение tip при **ровной высоте** + разрыв сессии при несовместимом каноне | `sync_live::on_tip`: ветка `lag == 0`; при наличии якоря finalized — сравнение хэша на высоте `finalized_h`; иначе сравнение `head_hash` с локальным tip; bypass если «оба признают незакрытый верхушечный блок» (`finalized_hash` present и оба finalized < head); при mismatch → `Some(TipDivergence)` | **PASS** (узкая политика из hotfix + microfix) | Средний: зависимость от корректности полей finalized от пира и от наличия локального блока в tail |
| Отставание по высоте: кто «догоняет» | При `lag > 0` у **локальной** стороны, обрабатывающей входящий announce: сперва `maybe_start_cup` при больших лагах / stall / уже активном CUP (`sync_live.rs`); иначе `ask_hdr(local_h + 1, …)`. Т.е. **локальный узел** инициирует header/catch-up запросы к пиру с более высоким `head_h`. | **PASS** как «отстающий запрашивает» | Низкий |
| Fork / continuity при заголовках | `on_hdr_batch`: разрывы `prev_hash`/высоты → `sync_fork_conflict_total`++, `live_stall`++, **без** обязательного disconnect в этом коде (`sync_live.rs`) | **PARTIAL**: детект и метрики **PASS**, симметричного «глушить сессию» **нет** | Средний: лечится ретраями/другими пирами |
| Применение блоков: форк-безопасное отбраковывание | `apply_blk` / `apply_blk_batch`: откат при любой ошибке (`sync_live.rs`); `on_blk_batch` при несоответствии hash/height → conflict counters + stall | **PASS** как «не применять мусор» | Низкий |
| Отсутствие silent split при **ровной высоте** и разном hash | `SyncTipDivergence` disconnect path (`route_sync_stub` после `on_tip`) | **PASS** (намеренно громко для MVP ops) | Операционный trade-off описан в policy doc |

**Доказательства:** `sync_live.rs` (`on_tip`, `send_sync_tip`, `on_hdr_batch`, `on_blk_batch`, `apply_blk_batch`); `peer_session/mod.rs` (`route_sync_stub`); тесты `tip_divergence_*`, `hdr_batch_break_drop`; метрики `sync_fork_conflict_total`, `sync_tip_divergence_disconnect_total`; runbook контекста: `docs/reviews/20260508-divergence-guard-policy.md`, `tasks/20260508-consensus-divergence-guard.json`.

---

## Краткий ответ: кто догоняет и где «fork prevention»

**Кто догоняет:** узел с **меньшим** локальным `tip_h`, получивший `SyncTipAnnounce` с большим `head_h`, выполняет `maybe_start_cup` или выдаёт `SyncHeadersReq` с `from_height = local_h + 1` (`on_tip`). Пир с большей высотой в этой модели выступает источником заголовков/чанков (ответы через `on_hdr_req` / CUP-сообщения на своей стороне).

**Где предотвращение / снижение форка:**

1. **Handshake:** сеть + genesis (+ подпись, skew, nonce) задают общий «Genesis proximity» контур; опционально bridge digest для replica same-shard trust.
2. **Ранняя операционная палка при равной высоте:** `TipDivergence` → закрытие сессии (не heal через этого пира).
3. **Sync path:** фильтры `shard_id`, `full_v1`; разрывы цепочки заголовков/блоков → счётчики конфликта + stall, блоки не принимаются если не сходится крипт/состояние (`apply_*`).
4. **Явное отсутствие в коде:** нет универсального BFT-finality доказательства «ветки не существует» на handshake; нет симметричного disconnect для всех случаев `sync_fork_conflict_total` (отличается от tip-divergence guard).

---

## Итоговый вердикт по покрытию MVP (без нового поведения)

**PARTIAL.** Same-shard + FullV1 контур покрывает **genesis/network**, wire **shard/header profile**, **tip/finalized** safeguard на равной высоте, **catch-up** и **apply rollback**. Не покрыты или покрыты слабо: **совпадение cluster_id**, **жёсткая capability negotiation**, и **немедлинный операторский detach** при header-level fork без прохождения через tip-equal guard.

---

## Participation / token estimate (оркестратор)

- `agent`: pwm-review  
- `result`: PASS (анализ доставлен артефактом)  
- `artifacts`: `docs/reviews/20260508-divergence-coverage-matrix.md`, обновление `tasks/20260508-consensus-divergence-guard.json`  
- `token_usage`: estimate `{ "source": "estimate", "total": ~12000, "confidence": "low" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260508-divergence-coverage-matrix.md'
git add 'tasks/20260508-consensus-divergence-guard.json'
git commit -m 'docs: divergence coverage matrix and ticket artifact'
```
