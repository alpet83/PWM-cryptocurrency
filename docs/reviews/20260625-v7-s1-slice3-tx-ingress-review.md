# V7-S1 Slice 3 — tx ingress, HTTP hot path (pwm-review)

Дата: 2026-06-25  
Тикет: `20260625-v7-s1-slice3-review`  
Задача-якорь: `tasks/20260625-v7-s1-slice3-integration.json`  
Норматив: `docs/adr/0013-tx-pipeline-seda.md`, `docs/plans/mvp_v7s1.md` § Slice 3 (узкая интеграция по task JSON)

## 1. Scope recap

Слайс убирает **`pool.push` под write lock** из HTTP hot path для обычных TX (`Transfer`/`Init`/`Stake`/…): вместо этого — bounded `TxIngressChannel` (cap 256) и drain в seal loop.

Затронуто (по коду в main):

- `crates/pwmd/src/pipeline/queue.rs` — `TxIngressChannel`
- `crates/pwmd/src/state.rs` — `App.tx_ingress`
- `crates/pwmd/src/bootstrap.rs` — init `TxIngressChannel::new(256)` (3 пути создания App)
- `crates/pwmd/src/api/handlers_tx.rs` — read-lock precheck + `try_send`; roaming ветки без изменений
- `crates/pwmd/src/lifecycle.rs` — drain ingress → `g.pool` перед `take(64)`
- `crates/pwmd/src/tests/http_export.rs` — `v1_tx_accepts_signed_init` проверяет ingress, не pool

**Вне scope (как в task):** `WorkerPool`, `DispatchQueues`, изменения `Chain::seal` сигнатуры, wire, snapshot.

## 2. Requirements fit

| Критерий task JSON | Статус |
|--------------------|--------|
| `App.tx_ingress: Arc<TxIngressChannel>` | **Да** (`state.rs:54`) |
| Bounded cap 256, tokio mpsc + `Mutex<Receiver>` | **Да** (`queue.rs:20-32`) |
| Hot path: read precheck, `try_send`, 507 on full, NO_CONTENT | **Да** (`handlers_tx.rs:233-267`) |
| Roaming/direct-seal ветки не тронуты | **Да** (`handlers_tx.rs:90-231`) |
| Seal loop drain перед `pool.take` | **Да** (`lifecycle.rs:1816-1820`) |
| `cargo test -p pwmd` | **Не прогнан** в сессии; тест `v1_tx_accepts_signed_init` адаптирован под ingress |
| Нет новых крейтов | **Да** |
| `Chain::seal` / wire / snapshot | **Без изменений сигнатуры/формата** |

**vs формальный `mvp_v7s1.md` § Slice 3 post-condition:** интеграция **частичная** — ingress-канал, не полный SEDA `WorkerPool` path; smoke ≥10 tx/s и «все запросы через pipeline» — **не заявлены** в task JSON (explicit OUT: WorkerPool в Slice 3.5/4). Для **данного тикета** — fit OK.

## 3. Style and module shape

- `TxIngressChannel` в `pipeline/queue.rs` рядом с SEDA types — логично.
- Production hot path без `unwrap`/`expect` в `handlers_tx` ingress ветке.
- `check_entity_name_segments.py` — не прогнан.

### Wire JSON / u128

Wire JSON / u128: not applicable (HTTP `/v1/tx` JSON unchanged; no peer wire slice).

### Concurrency / parallelism

**Компоненты:** `tokio::sync::mpsc` ingress; `tokio::sync::Mutex` на receiver; seal loop `inner.write()` + drain; `RwLock<Inner>` в handlers.

**Инвариант task:** единственные писатели в `g.pool` под write lock — seal drain + roaming/direct-seal. **Соблюдён** для pool mutations.

**Находки:**

1. **Остаётся write lock на каждый `/v1/tx`** (строки 80–88): `roaming_pool.expire_by_height` + `lock_conflict_for` **до** match — все TX, включая `Transfer`, берут `inner.write()`. Главный bottleneck `pool.push` снят, но contention на этом guard сохраняется (**medium nit**).
2. **Второй краткий write lock** после `try_send` только для `push_tx_flow` (256–266) — противоречит step 6 task («лог без guard»); под нагрузкой снова serializes flow trace (**low/medium nit**).
3. **Drain:** `let _ = g.pool.push(tx)` — при полном `Mpool` (cap 4096) tx **уже извлечён из ingress и теряется** после клиентский 204 (**medium severity**). Нужен: `push` error handling (break + log/metric; или не `try_recv` если pool full).
4. **Ingress receiver `try_lock`:** при занятом mutex drain пропускается на один seal tick — безопасно (tx остаётся в канале), не потеря.
5. **Precheck на read lock** vs seal-time state — ожидаемая семантика «accepted ≠ guaranteed sealed» (task notes).

## 4. Safety

- Переполнение ingress → 507 `INSUFFICIENT_STORAGE` — корректный backpressure на ingress.
- Roaming path: rollback/snapshot поведение сохранено.
- **Silent drop** при `pool.push` Err в drain — единственный существенный safety gap (см. §3).

## 5. Tests

**Покрыто:**

- `v1_tx_accepts_signed_init` — tx в ingress после POST (`http_export.rs:49-50`).
- Preflight/reject paths (`v1_tx_underfunded_xfer_mempool` и др.) — pool пуст (precheck до ingress).

**Пробелы:**

- Нет теста drain ingress → pool → seal (e2e с seal loop).
- Нет теста 507 при полном ingress (cap 256).
- Нет теста поведения при `pool.push` Err в lifecycle drain.

## 6. Verdict

**approve with nits** — цель слайса (убрать `pool.push` write lock с hot path) **достигнута**; seal integration минимальна и scope-safe; merge допустим с follow-up.

**Приоритет nits:**

1. **Medium:** `lifecycle.rs` drain — не игнорировать `pool.push` Err (риск потери accepted tx).
2. **Medium:** вынести `roaming_pool.expire_by_height` / lock check с write lock для non-roaming TX или под read + отдельный maintenance hook.
3. **Low:** `push_tx_flow` после ingress без `inner.write()` (info-only или read-only trace).
4. **Low:** метрики ingress depth / rejected (ADR «метрики с первого дня»).
5. **Process:** формальный Slice 3 post-condition в `mvp_v7s1.md` (WorkerPool, 10 tx/s smoke) — отдельный тикет.

## 7. Participation / token estimate

```text
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260625-v7-s1-slice3-tx-ingress-review.md
token_usage: { "source": "estimate", "input": null, "output": null, "total": 8500, "confidence": "low" }
```

**Glossary:** GLOSSARY.md: без изменений (нового жаргона не появилось).

**Вердикт одной строкой для оркестратора:** `PASS — tx_ingress+seal drain OK; nits: silent pool.push drop, residual write locks (roaming guard + push_tx_flow).`