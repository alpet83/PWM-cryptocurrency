# Тестирование: V2-8 Slice 2 — native mempool gossip baseline (`SyncTx*`)

**Дата:** 2026-05-08  
**Агент:** pwm-testing  
**Якорный коммит реализации:** `74a14f9` (`feat(pwmd): add native mempool gossip baseline for sync v1`)  
**Тикет:** `tasks/20260508-v2-sprint8-slice2-mempool-gossip.json`

---

## Executive summary

Сборка `pwmd` и целевые unit-тесты в `transport::peer_session::tests` для inbound `SyncTxBatch`, дедупа, дропа по профилю и безопасного legacy-заголовка — **PASS**. Проверка сегментации имён функций на затронутых transport-файлах — **без нарушений**. Полный прогон `cargo test -p pwmd --lib` один раз завершился падением `slice20_dual_flow_ok` (HTTP 403 / «export handoff source peer is not trusted»); **повторный одиночный прогон того же теста — ok** → зафиксирован **флак/race** вне узкого scope Slice 2, не как регрессия от `SyncTx*` по коду ошибки.

**Вердикт по запросу слайса:** **PASS** (обязательные проверки + целевые тесты + name-segment gate).

---

## Preflight `target/debug`

- Скрипт: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/dev/preflight_target_debug.ps1`
- Результат: успех (`target/debug` в пределах порога 4096 MiB).
- **removed:** no

---

## Команды и результаты

| Команда | Результат |
|---------|-----------|
| `cargo check -p pwmd` | **PASS** |
| `cargo test -p pwmd peer_session::tests` | **PASS** (4 теста: `tx_batch_valid_in`, `tx_batch_dup_drop`, `tx_batch_profile_drop`, `legacy_sync_hdr_safe`) |
| `cargo test -p pwm-core mempool::` | **PASS** (2 теста; коммит трогает `Mpool::snapshot` для исходящего gossip) |
| `python scripts/check_rust_fn_name_segments.py` (см. ниже список путей) | **PASS**, `violations: []` |
| `cargo test -p pwmd --lib` (полная матрица lib) | **FLAKY**: 1-й прогон — **FAIL** `slice20_dual_flow_ok`; повтор `cargo test -p pwmd --lib slice20_dual_flow_ok` — **PASS** |

### Файлы для name-segment check

`crates/pwmd/src/transport/peer_session/mod.rs`, `inbound.rs`, `seed/session/initial_exchange.rs`, `seed/session/steady_session.rs`, `peer_session/wire.rs`, `transport/tests/harness.rs`, `transport/handshake_state.rs`, `transport/metrics.rs`.

`cargo fmt` по Rust не выполнялся: в данном handoff меняются только `docs/` и `tasks/`.

---

## Соответствие scope Slice 2

| Тема | Наблюдение |
|------|------------|
| Inbound `SyncTxBatch` → mempool | `tx_batch_valid_in` подтверждает рост `sync_tx_accept_total` и `pool.snapshot(8).len() == 1`. |
| Dedup | `tx_batch_dup_drop` — `duplicate` в `sync_tx_drop_reason_total`. |
| Rate limit / cap | Логика `SYNC_TX_IN_CAP` в `ingest_sync_tx_batch`; отдельного unit-теста на превышение cap в коммите нет (acceptance — по коду/метрикам). |
| Precheck / shape | Невалидные формы уходят в счётчик `invalid` (покрытие косвенное через happy-path precheck в dev_net). |
| Legacy / profile gate | `tx_batch_profile_drop`, `legacy_sync_hdr_safe` — дропы по `profile_mismatch` и отсутствие учёта `sync_tx_*` для legacy non-tx кадра. |
| `SyncTxAnnounce` / `SyncTxReq` | В `route_sync_stub` помечены как **unsupported** (метрика `unsupported`); ожидаемо для baseline «только batch». |

---

## Deadlock / порядок блокировок (coding note)

Статический разбор новых путей:

- `ingest_sync_tx_batch`: для каждой транзакции сначала короткий `app.handshake.write()` (дедуп `mempool_gsp`), **отпускает**, затем `app.inner.write()` для `pool` / precheck. В конце при `accepted > 0` снова `handshake.write()` — **взаимной удержки `inner` и `handshake` нет**.
- `send_sync_tx_batch`: `inner.read()` (снимок пула) → отпуск → `handshake.write()` для отбора/учёта отправки → сеть → снова `handshake.write()` для `tx_sent_peer_ms`.

Явной инверсии «`inner` удержан и ждёт `handshake`» vs «`handshake` удержан и ждёт `inner`» в этих функциях не видно. **Остаточный риск:** любые будущие вызовы, которые удерживают оба замка в другом порядке, или гонки на уровне HTTP/e2e (см. флак ниже).

---

## Риски и follow-up

1. **Флак `slice20_dual_flow_ok`:** доверие peer / export-provenance; повтор проходит — завести стабилизацию (readiness, фикстура trust) отдельно от Slice 2.
2. **Нет unit-теста на исходящий `send_sync_tx_batch` в wire-harness** (есть обновления строк в `harness.rs`); приемлемо для baseline, если pwm-review ок с покрытием только inbound stub + tick path вручную/позже.
3. **`SyncTxAnnounce`/`SyncTxReq`:** намеренно unsupported в этом слайсе — не баг по отчёту кодинга.

---

## Participation / token estimate

```yaml
agent: pwm-testing
result: PASS
artifacts:
  - docs/reviews/20260508-v2-8-slice2-testing.md
commands:
  - cmd: cargo check -p pwmd
    result: PASS
  - cmd: cargo test -p pwmd peer_session::tests
    result: PASS
  - cmd: cargo test -p pwm-core mempool::
    result: PASS
  - cmd: python scripts/check_rust_fn_name_segments.py (transport paths)
    result: PASS
  - cmd: cargo test -p pwmd --lib
    result: FLAKY (slice20_dual_flow_ok fail then pass on rerun)
cleanup:
  cleaned: yes
  note: постоянные демоны не запускались
preflight_target_debug: powershell ps1 PASS; threshold ok; removed no
snapshot_benches: n/a (не требовалось тикетом Slice 2)
token_usage:
  source: estimate
  input: null
  output: null
  total: 12000
  confidence: low
```
