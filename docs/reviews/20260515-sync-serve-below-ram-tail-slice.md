# Review: sync serve below RAM tail (hdr/blk/catchup disk fallback)

**Ticket:** `tasks/20260515-slice-sync-serve-below-ram-tail.json`  
**Scope (код):** `crates/pwmd/src/snapshot/incremental.rs`, `transport/peer_session/sync_live.rs`, `wire.rs`, `handshake_state.rs`, `peer_session/mod.rs`, `CHANGELOG.md`  
**Reviewer:** `pwm-review` (independent)

---

## 1. Scope recap

Тикет фиксирует RCA: при `tip_h > TAIL_BLOCK_CAP` в RAM нет ранних блоков, поэтому обработчики sync ранее отвечали NACK на заголовки с малым `from_h`, если «первая» высота в RAM выше запрошенной — пир с пустой памятью не начинал синхронизацию.

Заявленная реализация: чтение последовательных блоков из epoch JSONL + manifest при нехватке RAM в `on_hdr_req` и `on_cup_req`; в `on_blk_req` — опциональный `block_heights` на wire, в `SyncPeerState` очереди `(height, hash)` для `pend_blk` / `wait_blk`, совместимый fallback полного скана эпох по hash для клиентов без высот; регрессионный тест на сценарий «tip выше tail, запрос заголовков с высоты 1».

В JSON тикета `mvp_checklist` пустой; приёмка в brief опирается на CY lab скрипты.

---

## 2. Requirements fit

**Цель закрыта по сути.**

- **Заголовки:** после сборки из `g.chain.blocks` при отсутствии непрерывной цепочки с `from_h` выполняется `load_consecutive_blocks_from_epochs`; при успехе и первой высоте ровно `from_h` батч отдаётся — устраняет описанный NACK для `from_h=1` при большом tip.
- **Блоки:** сначала поиск в RAM tail; промежутки без блока пополняются с диска либо через `load_block_at_height(heights[ix])` с проверкой `hex(hdr_hash) == want`, либо через `load_hash_scan_blocks`, если высот на wire нет. Несовпадение длин `block_heights` и `block_hashes` — контролируемый NACK.
- **Catchup:** если выбор из RAM не даёт окно `[start_h, end_h]` целиком, выполняется consecutive load с диска до совпадения границ, иначе `catchup_gap`.

**Частичный зазор:** автоматический тест есть только для заголовков (`hdr_req_disk_below_tail`). Изолированных регрессий на дисковую ветку `on_blk_req` (с высотами и отдельно legacy hash-scan) и на дополнение окна в `on_cup_req` в дереве не видно — код выглядит согласованным, но эти ветки без прямого тестового покрытия.

---

## 3. Style and module shape

- **`python scripts/check_rust_fn_name_segments.py`** по путям из тикета: **нарушений нет** (`violations` пустые).
- **`sync_live.rs`**, **`incremental.rs`**, **`handshake_state.rs`**: краткий англоязычный модульный `//!` сохранён.
- Логика sync по-прежнему сконцентрирована в `sync_live.rs`; для узкого слайса допустимо без декомпозиции как отдельного требования.
- **Wire / semver:** `SyncBlocksReq` расширен опциональным полем (`serde(default)`, при сериализации пропуск если `None`). **`PWM_PROTOCOL_VERSION`** не менялся; при общей serde-модели это ожидаемо расширение без обязательного bump major при отсутствии жёсткого контракта «строго те же ключи».

---

## 4. Safety

- Данные для ответа берутся только из `app.data_file` и локального RAM — путь к файлам по произвольному вводу пира не строится.
- Для блоков с диска проверяется соответствие hash запросу; рассогласование высота/hash ведёт к NACK (`blocks_hash` / диапазон).
- **`load_cons_blocks_epochs`** контролирует непрерывность высот внутри читаемого отрезка — снижает риск «рваных» ответов при повреждении JSONL (ошибка как `Result`).
- В **`on_hdr_req`** ошибка загрузки с диска обрабатывается через `if let Ok(...)`, без обязательного логирования в этом ревью — итог мягкий NACK; диагностика оператором может ограничиваться симптомом sync.

---

## 5. Tests

- **Есть:** `transport::peer_session::sync_live::tests::hdr_req_disk_below_tail` — локальный прогон `cargo test -p pwmd hdr_req_disk_below_tail` успешен.
- **Рекомендуется добавить:** тесты disk-веток `on_blk_req` и `on_cup_req`, плюс при желании маленький unit на `load_hash_scan_blocks` с двумя hash в одной эпохе (nit, не блокер при принятии риска).

---

## 6. Performance notes

- **`on_blk_req`:** перед разбором клонируется полный deque tail в RAM (`g.chain.blocks.clone()`). При длинном хвосте это лишнее копирование на каждый запрос блоков до лимита cap — возможный CPU/memory hot path под sync-нагрузкой.
- **`load_hash_scan_blocks`:** намеренно дорогой совместимый путь — линейный проход строк эпох с внутренним сопоставлением к спискам hash и клонами `Block`; на больших эпохах и при многих промахах время ответа растёт (смягчено caps на размер запроса).

---

## 7. Verdict

**Approve with nits** — RCA и главный регресс по заголовкам закрыты, проверки hash сохранены, wire обратносуместим. Ниты: добить тесты blk/cup (и опционально hash-scan), при необходимости профилировать клонирование tail.

---

## 8. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260515-sync-serve-below-ram-tail-slice.md
token_usage:
  source: estimate
  input: 18000
  output: 3500
  total: 21500
  confidence: low
```

---

**Gate (документ): PASS.**

Нит: для финального закрытия спринта глоссарий не затрагивался (не финальное ревью спринта).

