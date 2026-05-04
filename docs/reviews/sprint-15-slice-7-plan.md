# Sprint 15 — Slice 7: incremental storage architecture plan (design lock)

Цель: зафиксировать архитектурные решения до старта `pwm-coding`, чтобы избежать повторного drift к монолитным snapshot-записям.

## 1) Зафиксированные решения

1. **ClickHouse write model:** `1 row = 1 block` (строгий append-only per block).
2. **Checkpoint cadence:** каждые `100` блоков.
3. **Storage topology:** `DB per network` + `table per cluster/domain`.
4. **JsonFile model:** эпохи по `1000` блоков (`block_e{num}.json`) + `pwm-data.json` как summary/state.
5. **Backward compatibility:** legacy `pwm-data.json` с `blocks[]` продолжает грузиться через fallback path.
6. **Memory bound:** in-memory cache блоков не более `1000`, но canonical height хранится отдельно.
7. **Explorer/ops fields:** в явные поля выносятся как минимум `tx_count`; `shard_balance` допускается как checkpoint-level aggregate при фиксированной семантике.

## 2) Инварианты (обязательные)

1. `tip_height` не вычисляется из `tail_blocks.len()`.
2. Любая запись в CH и epoch-файлы выполняется только после локальной проверки целостности хвоста.
3. Для checkpoint используется детерминированный digest payload (подписи не на произвольную строку/JSON).
4. Bootstrap работает по схеме `checkpoint + tail replay`, при mismatch — fail-fast.
5. Epoch-публикация только через atomic shadow-copy (`tmp -> rename`) + manifest commit.

## 3) Контур БД (v1, без переусложнения)

**Ключ сортировки и ReplacingMergeTree.** Таблицы `blocks__*` / `checkpoints__*` используют `ReplacingMergeTree(inserted_at)`: при слиянии частей ClickHouse оставляет одну строку на каждый ключ `ORDER BY`, выбирая версию по наибольшему `inserted_at`. Чтобы при совместном использовании одной физической таблицы несколькими логическими цепочками (разный `row_key`, одинаковые высоты) не происходило склейки чужих строк, **`ORDER BY` обязан включать `row_key` первым компонентом**: для блоков `(row_key, height)`, для чекпоинтов `(row_key, genesis_digest, checkpoint_height)`. Разделение по БД на сеть и суффиксу `__0xHH` остаётся основной топологией; расширенный ключ — страховка для операторских конфигураций и миграций.

## 3.1 `blocks__0xHH`
- `height`
- `block_hash`
- `prev_hash`
- `ts`
- `prod_idx`
- `tx_count`
- `state_root`
- `payload_json` (wire текущего блока)
- `inserted_at`

Engine: `ReplacingMergeTree(inserted_at)`, `ORDER BY (row_key, height)`.

## 3.2 `checkpoints__0xHH`
- `checkpoint_height`
- `genesis_digest`
- `state_root`
- `state_json`
- `roaming_json`
- `cross_shard_json`
- `shard_balance` (опционально v1, если фиксируем формулу)
- `inserted_at`

Engine: `ReplacingMergeTree(inserted_at)`, `ORDER BY (row_key, genesis_digest, checkpoint_height)`.

## 3.3 `validators_accept` (MVP-safe)
- `checkpoint_height`
- `validator_id`
- `checkpoint_digest`
- `sig`
- `accepted_at`

Режим: append-only; удаление/перезапись не используются в runtime-path.

## 4) Политика валидации перед записью

1. **Per-block write:** перед insert проверяем хвост до checkpoint window (до 99 блоков) по каноническим fingerprints:
   - `height`, `block_hash`, `prev_hash`, `tx_root`, `state_root`, `tx_count`.
2. Если mismatch:
   - блок в БД не пишется,
   - checkpoint не подписывается,
   - событие уходит в divergence diagnostics.
3. **Checkpoint write (каждые 100):**
   - сначала полная валидация окна 100 блоков,
   - затем запись checkpoint,
   - затем публикация подписи в `validators_accept`.

## 5) JsonFile epoch protocol (crash-safe)

1. Запись блока в `block_e{num}.json.tmp`.
2. `fsync(file)`.
3. `rename(tmp -> block_e{num}.json)`.
4. Обновление manifest (`tmp -> rename`).
5. `fsync(dir)` где возможно.

Recovery при старте:
- удалить/игнорировать незакоммиченные `.tmp`,
- проверить непрерывность диапазонов эпох,
- при разрыве fail-fast с явной диагностикой.

## 6) Волновая стратегия (до начала широкой имплементации)

### Wave 0 — design lock + contracts
- утвердить формат таблиц/полей,
- утвердить формулу `checkpoint_digest`,
- утвердить replay contract и bootstrap steps.

### Wave 1 — Json epochs + fallback
- summary-only `pwm-data.json`,
- `block_e{num}.json`,
- atomic publish + recovery,
- legacy fallback load.

### Wave 2 — memory model
- bounded tail cache (1000),
- отдельные `canonical_height`/`tip_hash`,
- адаптация runtime flow к bounded cache.

### Wave 3 — CH incremental path
- per-block insert,
- checkpoint insert every 100,
- `validators_accept`,
- диагностика рассинхрона.

### Wave 4 — bench + hardening + closeout
- сравнительные бенчи старого/нового путей,
- multi-node consistency сценарии,
- final `pwm-review` gate и decision note.

## 6.1) Wave 4 — `shard_balance` и `validators_accept` (эксплорер / подписи)

### `shard_balance` (только на границе checkpoint)

- **Когда считается:** в момент записи строки в `checkpoints__*` — то есть при seal на высотах кратных cadence (`100`), при tip-summary snapshot для ClickHouse, и при импорте через `import_snapshot_file` (каждая синтетическая контрольная точка импорта).
- **Формула (v1):** JSON-объект с ключами `"0xHH"` (нижний регистр, два hex-разряда старшего байта домена аккаунта). Значение — десятичная строка \( \sum (\texttt{balance\_pwm} + \texttt{staked}) \) по всем записям `accounts`, у которых `domain_of_account_id(account_id)` имеет данный старший байт. `fee_pool` не входит (не привязан к шарду). Пустое состояние без счетов ⇒ `{}`.
- **Упорядочивание ключей:** лексикографически по строке ключа (используется `BTreeMap` при сериализации).

### `validators_accept` и `checkpoint_digest`

- Строки в `validators_accept__*` в Wave 4 **пока не пишутся** из runtime (нет проводки `validator_id` + Ed25519 подписи в консенсусе).
- **Детерминированный идентификатор снимка состояния для будущих подписей:** `checkpoint_digest = hex(blake3(bincode(state)))`, то есть `pwm_core::digest(state)` в hex — тот же дайджест, что и для genesis-linked row identity; при расширении контракта подписи на roaming/cross_shard отдельным ADR.

## 7) Явно запрещено в Slice 7

- Перезаписывать в БД "всю цепочку целиком" на каждый блок.
- Считать canonical height из длины bounded cache.
- Молчаливо пропускать повреждённые epochs/checkpoints.
- Подписывать checkpoint по недетерминированному JSON-представлению.
- Включать `node_id` в canonical identity ключ состояния.
