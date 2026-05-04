# Sprint 15 — S3.13: Federation table — Code review

## 1. Scope recap

По тикету `tasks/20260430-s15-slice3-13-federation-table-contract-implementation.json` и контракту **`docs/reviews/sprint-15-s3-11-federation-and-reconnect-review.md` § B** в объём входят:

- словарь шардов на ноде (height, last_seen, TTL 60s, merge, sweep);
- обновления **только из доверенных** сигналов (hello / heartbeat / status там, где уже считается trusted);
- **`GET /v1/federation/shards`** с полями ответа из § B;
- bump версии `pwmd` при публичном HTTP API;
- юнит-тесты merge / TTL / view_health и артефакт кодинга.

Проверены в первую очередь: `crates/pwmd/src/federation.rs`, маршрут и снимок в `api.rs`, проводки в `transport.rs` (seed HTTP, seed wire, inbound), поля в `handshake.rs`, поле `Inner.federation` в `state.rs`, запуск sweep в `lifecycle.rs`, инициализация в `bootstrap.rs`.

## 2. Requirements fit (§ B)

**Строка / merge.** Реализация в `FederationTable::merge_row` соответствует заявленным правилам: вставка при отсутствии ключа; при большей высоте — полная замена; при равной — обновление last_seen/source только если входящий last_seen новее; при меньшей высоте — высота не понижается, last_seen берётся как max, source/source_node_id обновляются.

**TTL и eviction.** TTL зафиксирован 60s; просрочка эквивалентна `now >= last_seen + ttl_ms`; sweep удаляет строки по этому условию. Фоновый цикл с интервалом ~1s есть (`spawn_federation_sweep_loop`). Дополнительно sweep вызывается после построения HTTP-снимка — согласовано с handoff.

**Источники строк.** Поля `source` мапятся на строки `"hello" | "heartbeat" | "status"` как в контракте. Локальная строка подмешивается через `merge_local_status` на пути HTTP-снимка.

**HTTP JSON.** Ответ сериализуется из `FederationShardsOut` / `FederationShardRowOut`: `generated_at_unix_ms`, `ttl_sec`, `view_health`, `expected_shard_count`, счётчики, `rows[]` с полями включая `expires_at_unix_ms` и `fresh`. Семантика `view_health` при `expected_shard_count: null` (partial при нуле активных «свежих», иначе complete при отсутствии stale; stale при наличии просроченных строк) совпадает с описанием в `sprint-15-s3-13-coding.md`.

**Пробел по контракту (ожидаемый).** `expected_shard_count` везде передаётся как `None` → в JSON `null`; это явно задокументировано как отсутствие политики/конфига — не противоречит минимальному контракту § B, но **семантика «полноты сети» остаётся ограниченной**, пока не появится источник ожидаемого числа шардов.

## 3. Style

Имена в production-коде в целом короткие и по стилю крейта; модуль `federation.rs` вынесен отдельно от раздувания `api.rs`/`transport.rs`. Замечаний уровня «>4 слова в snake_case» по критичным символам нет.

## 4. Safety

**Граница доверия.** Проверено по call sites:

- Исходящий HTTP hello к seed: `process_incoming_peer_hello(..., true)` и затем `merge_remote_hello` — ок.
- Исходящий wire handshake к seed: то же — ок.
- Входящий TCP: `process_incoming_peer_hello(..., false)`; **`merge_remote_hello` не вызывается**; heartbeat обрабатывается через `merge_remote_hb(..., false)` — **запись в federation не выполняется** — ок.
- Доверенный wire-цикл к seed: `merge_remote_hb(..., true)` — ок.
- HTTP `/v1/peer/hello`: `process_incoming_peer_hello(..., false)` и **нет** вызова federation merge — ок.

**Замечание (nit).** `merge_remote_hello` сам по себе не принимает флаг trusted; безопасность целиком на дисциплине вызывающих сторон. Для текущего набора вызовов это согласовано, но при новых путях — зона риска регрессии.

Паники / явные DoS в добавленной логике не выделяются; HTTP handler использует уже существующий `ensure_ready` и время через `current_time_ms`.

## 5. Tests

Соответствует handoff: четыре юнит-теста в `federation.rs` покрывают merge, sweep, view_health, fallback ключа шарда. В `sprint-15-s3-13-testing.md` зафиксированы PASS по `cargo test -p pwmd federation`, ручной smoke `GET /v1/federation/shards` после rebuild, двухнодовый сценарий.

Пробелы вне обязательного scope тикета: нет постоянного автоматического теста именно на HTTP-роут в дереве `cargo test` (есть только ручной smoke).

## 6. Закрытие слайса при `cargo test -p pwmd --lib` FAIL

По **функциональному scope S3.13** (таблица federation, trusted-only обновления, TTL/sweep, контракт ответа API): да, слайс можно считать закрытым при красном полном `--lib`, **если** организационно зафиксировано, что gate этого тикета — целевые federation-тесты + ревью + согласованный smoke маршрута, а не обязательно зелёный весь `--lib`. Падения из handoff классифицированы как readiness/lifecycle/E2E и **не следуют из диффа federation** при имеющейся классификации — отдельный долг по восстановлению `--lib` для CI не смешивать с приёмкой S3.13.

Если же **официальный DoD репозитория** для merge — строго зелёный `cargo test -p pwmd --lib`, то это **блокер релиза ветки/PR**, но не обязательно «откат S3.13»; нужны правки тестов/readiness/lifecycle в отдельных тикетах.

## 7. Verdict

**approve with nits** для **scope S3.13**.

**Nits (низкий приоритет):** (1) при добавлении новых путей hello рассмотреть явный trusted-guard на уровне API merge hello; (2) опционально — закрепить HTTP контракт интеграционным тестом; (3) follow-up: источник `expected_shard_count` и более строгая семантика `complete`/`partial`.

**Follow-up тикеты (по желанию):** конфиг/политика ожидаемого числа шардов; интеграционный тест `GET /v1/federation/shards`; отдельный эпик на зелёный `cargo test -p pwmd --lib` (export-readiness в тестах, lifecycle, slice20 E2E).

---

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/sprint-15-s3-13-review.md
scope: S15-S3.13 federation table
verdict_one_line: "approve with nits — scope S3.13"
notes:
  - Full pwmd --lib red does not block S3.13 acceptance if DoD is scoped; repo-wide CI may still require separate fixes.
token_usage:
  source: estimate
  input: null
  output: null
  total: 8500
  confidence: low
```
