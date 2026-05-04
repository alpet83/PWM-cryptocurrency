# Sprint 15 — архитектурное ревью: доверие чекпоинтам vs полный перечень эпох

> **Historical / pre-change.** Этот отчёт описывает состояние до внедрения JsonFile trust-default startup. Актуальный doc audit: [`sprint-15-doc-audit-trust-default-arch-shift.md`](./sprint-15-doc-audit-trust-default-arch-shift.md); trust-boundary RFC: [`sprint-15-snapshot-trust-boundary-review.md`](./sprint-15-snapshot-trust-boundary-review.md).

**Тикет:** `tasks/20260503-s15-arch-trust-checkpoint-rescan-review.json`

## 1. Scope recap

**Contract:** заявленное направление — по умолчанию доверять сохранённому состоянию и чекпоинту (аналог «зрелых» узлов), полная верификация цепочки только по явному запросу или в режиме восстановления; согласованность с высокой частотой блоков и с тем, что эпохи и SNAP_CHK изначально подавались как оптимизация/масштабирование.

**Источники кода:** `snapshot/io.rs`, `snapshot/incremental.rs`, `snapshot/store.rs`, `snapshot/epoch.rs`, `lifecycle.rs`, `api/common.rs`, `api/handlers_tx.rs`.

## 2. Requirements fit (к целевому архитектурному направлению)

Текущее поведение **не соответствует** описанному целевому направлению «trust persisted state по умолчанию»:

- При старте JsonFile-бэкенд в `load_snapshot_timed` при `blocks_stored == Epochs` вызывает последовательную загрузку всех блоков из epoch JSONL (`incremental::load_blocks_from_epochs`), затем `validate_snapshot` выполняет **полный replay** по всем блокам в памяти.

- Эпохи и интервал `SNAP_CHK_BLK_IV` (100 блоков) сейчас лучше описываются как **шардирование хранения на диске** и периодический перезапись summary (`pwm-data.json` без массива блоков через `save_checkpoint_summary`), а не как способ избежать полной перечитки при следующем старте.

- Желаемое «доверять чекпоинту и manifest tip_hash» в коде **не реализовано как режим по умолчанию**: `checkpoint_height` фиксируется на запись и попадает в телеметрию старта, но **не отключает** загрузку эпох и полный replay при JsonFile load.

**Вывод по fit:** направление тикета — **RFC-уровень**; текущая реализация остаётся на модели «полная самопроверка при каждом старте», плюс горячие пути сохранения, которые дополнительно перечитывают эпохи при операциях API.

## 3. Style and module shape

Задача ревью — архитектура. В `incremental.rs` явно зафиксировано отсутствие ленивого кеша блоков — это сигнал честности относительно заявленной «оптимизации».

## 4. Safety (границы доверия, PoA vs Bitcoin-style)

- **Сегодня:** целостность для JsonFile строится на полном replay с проверкой связности, подписей и `state_root`.

- **Целевое направление:** если по умолчанию поднимать состояние с диска без replay, доверие смещается к целостности ФС/оператора и к непротиворечивости summary и manifest.

- **Риски при частых блоках:** полный replay при старте и полное чтение эпох при горячих сохранениях масштабируются по времени и памяти с длиной истории.

## 5. Tests

Регрессионные тесты вокруг epoch save/reload/sync есть; явных тестов «быстрый старт без replay» или разделения путей save vs tip-save для API — не выявлено в обзоре.

## 6. Ответы на вопросы постановки

**Почему сегодня читаются все эпохи**

1. **Старт:** `load_snapshot_timed` при `Epochs` заполняет `snap.blocks` через `load_blocks_from_epochs`, затем `validate_snapshot` итерирует все блоки.

2. **После cross-shard и других операций:** устойчивый путь — не только рестарт. `SnapshotBackend::save` для JsonFile вызывает `save_snapshot` → `encode_inner_snap_json`. Если в памяти цепочка **не покрывает полную историю** (хвост после `absorb_blocks_tail`), ветка кодирования подтягивает недостающее с диска: `sync_epoch_disk_to_tip` при необходимости и затем **`load_blocks_from_epochs`** — полный проход по epoch JSONL для сборки монолитного снимка.

   Цепочка: `snapshot_save_under_inner_lock` → `backend.save` → используется из HTTP-слоя (`handlers_tx`, roaming/relay в `api/common.rs`). Это объясняет наблюдение «после межшардовой операции нода снова прошлась по всем epoch-файлам» **без перезапуска**, если смотреть телеметрию/диск.

**Что реально гарантируют чекпоинты / SNAP_CHK**

- **SNAP_CHK_BLK_IV:** периодичность перезаписи summary поверх append в epoch JSONL (`json_file_seal_persist`).
- **`checkpoint_height`:** маркер высоты на момент записи summary; **не заменяет** загрузку эпох при текущем алгоритме load.
- **Manifest:** используется для последовательной загрузки; не как единственный источник истины без replay в текущей логике validate.

## 7. Рекомендации (приоритет)

1. **P0 — развести пути персистентности:** для операций, вызывающих `SnapshotBackend::save` (монолитный JSON), рассмотреть переход на инкрементальный append + checkpoint/tip summary без сборки полной цепочки из epoch на каждый API-save; монолитный `save_snapshot` — миграции, отладка, явный запрос.

2. **P1 — контракт быстрого старта:** формализовать «cold start trusted», инварианты summary/manifest, где включается полный replay.

3. **P2 — операционные режимы:** флаги audit/recovery; связать с телеметрией стадий.

4. **P3 — документация trust model** для операторов (PoA vs Bitcoin-style).

## 8. Verdict

**Verdict: request changes (architecture)** — заявленное «эпохи и чекпоинты как оптимизация старта» **не выполняется** текущей семантикой load и частью API-save; наблюдение после cross-shard **согласуется** с монолитным save после commit → чтение всех эпох при неполном хвосте в RAM.

---

## Participation / token estimate (review agent)

- `agent`: pwm-review  
- `result`: PASS (report delivered)  
- `token_usage`: estimate, total ~4200, confidence low  
