# Review: документация раздачи same-shard sync ниже RAM-хвоста + стиль имён (fn segments)

**Ticket:** `tasks/20260516-review-sync-serve-docs-fn-names.json`  
**Связанный слайс:** `tasks/20260515-slice-sync-serve-below-ram-tail.json`  
**Предыдущий отчёт по коду:** [`20260515-sync-serve-below-ram-tail-slice.md`](./20260515-sync-serve-below-ram-tail-slice.md)

---

## 1. Scope recap

Тикет требует проверить целевое **operator/dev-facing** описание поведения, когда высота цепи превышает RAM-хвост (`pwm_core::TAIL_BLOCK_CAP`, **1000**): пропозер/пир отдаёт заголовки, блоки и catch-up, поднимая данные из **JsonFile epochs** (manifest + JSONL), если память не покрывает запрошенный `from_h`; опциональное поле **`block_heights`** в `SyncBlocksReq` (обратная совместимость через serde); сохранение **legacy hash-scan** пути. Ожидалось добавление поясняющего текста в **`docs/rfc/15-same-shard-sync-v1.md`** (по необходимости помеченного как non-normative) и/или addendum в **`docs/reviews/`** с перекрёстными ссылками на тикет слайса и CY lab скрипты; краткая отметка про **трёхузловой CY lab**, `cy-cluster-follower.ps1` и **`--seal-lease-backend process-local`** в согласовании с quorum-нодами или `cy-cluster-common.ps1`. Параллельно — повторный прогон **`scripts/check_rust_fn_name_segments.py`** по заявленным путям `pwmd`.

Продакшен-Rust в этом тикете не менялся (review-only).

---

## 2. Requirements fit (документация)

**Частичное покрытие.**

- **RFC 0015 (`docs/rfc/15-same-shard-sync-v1.md`):** после просмотра полного файла и поиска по репозиторию **отдельного информативного подраздела** про раздачу sync с диска при `tip_h` выше нижней границы RAM-хвоста, про JsonFile epochs, `block_heights` и hash-scan **нет**. Текст RFC по-прежнему описывает нормативный wire-контракт v1 (в т. ч. `BlocksRequest` с `block_hashes` в §6.2) без привязки к реализации **disk-backed ответов** и лимиту `TAIL_BLOCK_CAP`. Для целей тикета это **зазор**: операторам и последующим спринтам не хватает одной явной «карты чтения» RFC ↔ фактическое поведение `pwmd` в JsonFile режиме.
- **`docs/reviews/20260515-sync-serve-below-ram-tail-slice.md`:** содержит технически полезный разбор реализации (`load_consecutive_blocks_from_epochs`, дисковые ветки hdr/blk/cup, `block_heights`, hash-scan, тест `hdr_req_disk_below_tail`). На **CY lab** указано лишь общо («приёмка в brief опирается на CY lab скрипты»); **нет** явных ссылок на `cy-cluster-follower.ps1`, `cy-cluster-common.ps1` и мотива **process-local** lease для follower.
- **Другая существующая документация:** в `docs/guide-node-storage-and-snapshot.md` раздел «Tail load» объясняет **загрузку** при ограниченном RAM-хвосте при старте, но **не** описывает **исходящую** раздачу истории удалённому peer при синхронизации — для симптома «виден tip, mem=0» это недостаточно как единственный операторский вход.
- **CY скрипты:** в репозитории `cy-cluster-follower.ps1` уже передаёт `'--seal-lease-backend', 'process-local'`, строка-хост сообщает то же; в `cy-cluster-common.ps1` есть комментарий про избежание stale file-lease CAS при локальных повторных запусках. То есть **поведение лаборатории задокументировано в коде скриптов**, но связка «3-node follower + quorum alignment» не процитирована в RFC15 / отдельном addendum под этот ревью-тикет.

**Итог по цели 1 документации:** ключевое ожидаемое дополнение в **RFC15** (informative subsection) **не выполнено**; addendum со ссылками на CY тоже можно усилить. Рекомендация последующего микрослайса документации: добавить в RFC15 после §11 или в «Rollout Notes» короткий **non-normative** блок (implementation note для `pwmd` + JsonFile): RAM-хост `TAIL_BLOCK_CAP`, чтение epoch JSONL/manifest для hdr/blk/catch-up ниже нижней RAM-высоты, optional `block_heights` для тел без полного скана, legacy перебор по hash при отсутствии высот; перекрестная ссылка на `guide-node-storage-and-snapshot.md`, тикет `20260515-slice-sync-serve-below-ram-tail.json` и обзор `20260515-sync-serve-below-ram-tail-slice.md`; одна фраза про CY follower и process-local lease.

---

## 3. Style and module shape (`check_rust_fn_name_segments.py`)

Из корня репозитория выполнена команда из тикета:

`python scripts/check_rust_fn_name_segments.py crates/pwmd/src/snapshot/incremental.rs crates/pwmd/src/transport/peer_session/sync_live.rs crates/pwmd/src/transport/peer_session/wire.rs crates/pwmd/src/transport/handshake_state.rs crates/pwmd/src/transport/peer_session/mod.rs`

**Результат:** для всех пяти файлов массив **`violations` пустой** (политика prod ≤ 4 сегментов на идентификатор соблюдена для проверенных путей). **Серьёзных замечаний по именованию по этому инструменту нет.**

---

## 4. Safety / протокол (без изменения кода в этом тикете)

Новых рисков от текущего тикета нет — изменений в `crates/**` не вносилось. Напоминание из предыдущего слайс-ревью (для трассируемости): границы доверия остаются «локальный `data_file` + проверки hash», hash-scan остаётся дорогим совместимым путём (см. [`20260515-sync-serve-below-ram-tail-slice.md`](./20260515-sync-serve-below-ram-tail-slice.md)).

---

## 5. Tests

Отдельный прогон тестов в рамках этого документного тикета не выполнялся. Покрытие слайса по коду уже зафиксировано в `20260515-sync-serve-below-ram-tail-slice.md` (`hdr_req_disk_below_tail`; ниты про blk/cup/hash-scan без отдельных тестов остаются прежними рекомендациями для `pwm-coding`/`pwm-testing`).

---

## 6. Verdict

**Approve with nits (документ), итоговый результат трейсинга тикета — PARTIAL.**

Код-путь и стиль имён для заявленных файлов соответствуют автоматической проверке. **Не закрыта** явная доля цели по **RFC15 / сводному operator-facing addendum** (включая перекрёстки на CY follower и lease). После добавления указанного non-normative блока в RFC15 (или эквивалентного узкого раздела в `docs/pwmd.md` + ссылка из RFC15) тикет можно перевести к **полному PASS** по документной части без повтора fn-segment проверки, если имена Rust не менялись.

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PARTIAL
artifacts: docs/reviews/20260516-sync-serve-docs-and-style-review.md
token_usage:
  source: estimate
  input: 14000
  output: 4500
  total: 18500
  confidence: low
```

---

**Glossary:** GLOSSARY.md: без изменений (нового жаргона не появилось).

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260516-sync-serve-docs-and-style-review.md'
git add 'tasks/20260516-review-sync-serve-docs-fn-names.json'
git commit -m 'docs(review): sync serve below RAM tail docs + fn segments traceability'
```

**Verdict (one-line):** `PARTIAL` — стиль имён OK; документация по RFC15/CY cross-links неполная.
