# Snapshot: рассинхрон `checkpoint_height` (summary) vs `canonical_h` (epoch manifest)

## Scope recap

Тикет `20260507-snapshot-verify-progress-and-lag-root-cause`: объяснить, **помимо аварийного завершения**, почему при старте возникает предупреждение «snapshot summary lags epoch manifest» и затем включается полная верификация цепочки (долгий replay без частого прогресса — отдельная задача на стороне pwm-coding).

Источники в коде: `crates/pwmd/src/snapshot/io.rs`, `incremental.rs`, `repair.rs`, `epoch.rs`, `store.rs`.

---

## Root causes (ranked)

### 1. Нормальный режим JsonFile: manifest обновляется каждый блок, summary — только на границе `SNAP_CHK_BLK_IV` (главная причина)

На пути **seal** вызывается `json_file_seal_persist`: сначала каждый раз добавляется блок в JSONL и **перезаписывается manifest** с актуальным `canonical_h`, а **`pwm-data.json` с `checkpoint_height` переписывается только если `h % SNAP_CHK_BLK_IV == 0`** (константа **100**).

```853:859:p:/opt/docker/PWM-cryptocurrency/crates/pwmd/src/snapshot/io.rs
pub(crate) fn json_file_seal_persist(path: &FsPath, inner: &Inner) -> Result<(), String> {
    incremental::append_tip_block(path, inner)?;
    let h = inner.chain.tip_h();
    if h > 0 && h % super::epoch::SNAP_CHK_BLK_IV == 0 {
        save_checkpoint_summary(path, inner)?;
    }
    Ok(())
}
```

Внутри `append_block_for_epoch` после записи epoch-файла manifest всегда получает `canonical_h = h` текущего блока:

```86:101:p:/opt/docker/PWM-cryptocurrency/crates/pwmd/src/snapshot/incremental.rs
    let mut man = if let Some(m) = load_manifest(summary_path)? {
        m
    } else {
        mk_manifest(h, tip_hash.clone(), vec![meta.clone()])
    };
    man.canonical_h = h;
    man.tip_hash = tip_hash;
    // ...
    write_manifest(summary_path, &man)?;
```

Итого между чекпоинтами summary на диске отстаёт от manifest до **99 высот** — это **не сбой**, а следствие политики редкой перезаписи summary на seal-пути (см. комментарий в `epoch.rs` про относительную частоту чекпоинтов).

При старте с `snapshot_verify_chain = false` (дефолт в `config.rs`) загрузчик видит расхождение и **принудительно включает полную верификацию**:

```582:592:p:/opt/docker/PWM-cryptocurrency/crates/pwmd/src/snapshot/io.rs
    if snap.blocks_stored == BlocksStored::Epochs && !effective_opts.verify_chain && mp.exists() {
        if let Ok(Some(man)) = incremental::read_epoch_manifest(path) {
            if man.canonical_h > 0 && man.canonical_h != snap.checkpoint_height {
                warn!(
                    target: SNAP_STARTUP_TARGET,
                    summary_checkpoint = snap.checkpoint_height,
                    manifest_tip = man.canonical_h,
                    "snapshot summary lags epoch manifest; forcing full chain verification"
                );
                effective_opts.verify_chain = true;
            }
        }
    }
```

Отсюда типичная картина: WARN почти на каждом старте, если между запусками не было вызова пути, который **каждый раз** обновляет summary (см. ниже).

### 2. Разные кодовые пути сохранения: «полное» выравнивание summary только на runtime/API save

`SnapshotBackend::save` для JsonFile вызывает `json_file_runtime_persist`, который после `sync_epoch_to_tip` **всегда** вызывает `save_checkpoint_summary` — тогда `checkpoint_height` совпадает с tip:

```813:817:p:/opt/docker/PWM-cryptocurrency/crates/pwmd/src/snapshot/io.rs
pub(crate) fn json_file_runtime_persist(path: &FsPath, inner: &Inner) -> Result<(), String> {
    if manifest_file_path(path).exists() {
        incremental::sync_epoch_to_tip(path, inner)?;
        save_checkpoint_summary(path, inner)
```

Если оператор или автоматика опираются только на **seal** (`save_seal_persist`) и редко дергают **`save`** / корректное завершение с финальным flush, summary чаще отстаёт от manifest. Обратный сценарий (summary новее manifest при отключённом sync) маловероятен при штатном одном процессе, но возможен при ручном редактировании/копировании файлов.

### 3. Нет единой транзакции между тремя артефактами (epoch JSONL, manifest, summary)

Каждый файл пишется атомарно (temp → fsync → rename) для epoch-шарда и manifest:

```374:388:p:/opt/docker/PWM-cryptocurrency/crates/pwmd/src/snapshot/incremental.rs
fn write_manifest(summary_path: &Path, man: &EpochManifest) -> Result<(), String> {
    // ...
        f.sync_all().map_err(|e| format!("manifest fsync: {e}"))?;
    }
    fs::rename(&tmp, &p).map_err(|e| format!("manifest rename: {e}"))?;
```

Summary — отдельная атомарная запись в `save_checkpoint_summary`. Между успешным `append_block_for_epoch` и следующим `save_checkpoint_summary` возможен обрыв процесса/OOM/kill: manifest уже «впереди», summary — на последнем успешном чекпоинте. Это **расширение** пункта 1, уже при аварии; пользователь просил причины кроме «только авария», но комбинация **редкий summary + любой нештатный стоп** усиливает расхождение.

### 4. Частичное обновление дерева при ручном вмешательстве

Копирование только `pwm-data.json`, только `epochs/`, только `pwm-epochs-manifest.json`, смешение каталогов от разных машин или бэкапов, монтирование устаревшего тома — все варианты, когда **логическая пара summary/manifest/JSONL не из одного момента времени**. Код это не может отличить от «отстающего summary», поэтому срабатывает тот же downgrade на полный replay.

### 5. Repair и порядок записи

`repair_json_epochs` переписывает epochs, manifest, summary последовательно. Прерывание посередине может оставить несогласованное дерево до следующего успешного прогона repair или ручной правки. После успешного завершения вызывается `load_snapshot` как пост-проверка — штатный happy-path выравнивает метаданные.

### 6. Версии формата и наследие

Миграции и поля по умолчанию (`checkpoint_height` для legacy inline может быть 0) при смешении старых summary с новым epoch-tree усиливают расхождение с manifest. Контракт канонического snapshot подчёркнут в `decode_snap_value_raw`; несовпадение версий даёт ошибку парсинга раньше, но частично обновлённые деревья остаются операционным риском.

### 7. Конкуренция процессов / два writer на один каталог данных

Два экземпляра pwmd на одном пути snapshot без внешней блокировки могут чередовать записи manifest и summary произвольно — класс гонок вне модели «один writer».

---

## Operational mitigations

1. **Ожидать WARN при дефолтном trust-load**, если доминирует только seal-путь: это следствие `SNAP_CHK_BLK_IV`, не обязательно порча данных.
2. **Перед остановкой** подстраховаться вызовом пути, который синхронизирует summary с tip (в коде — `SnapshotBackend::save` / `json_file_runtime_persist`), либо дождаться высоты кратной 100 после последнего seal (хрупко для оператора).
3. **Не копировать** дерево snapshot по частям; переносить каталог целиком (`pwm-data.json`, `epochs/` включая manifest).
4. После сбоев использовать **`repair_json_epochs`** и проверять пост-load.
5. Для диагностики сравнивать `checkpoint_height` в summary и `canonical_h` в `epochs/pwm-epochs-manifest.json`.

---

## Could code changes reduce lag / WARN noise? (для pwm-coding / RFC)

- **Выровнять семантику**: либо записывать `checkpoint_height` в summary чаще (цена — больше перезаписей большого JSON state), либо ослабить эвристику в `load_snapshot_timed` (например, считать отставание допустимым, если manifest ≥ summary и хвост JSONL согласован — потребуется строгое доказательство безопасности trust-load).
- **Явный маркер «ожидаемого лага»** в manifest или summary (schema bump) мог бы отличать политику от повреждения.
- **Прогресс при полном replay** — UX-исправление долгого «молчания» (отдельный пункт тикета).

---

## Requirements fit (формальный блок ревью)

Цель тикета — объяснение причин — выполнена кодом: доминирующая причина задокументирована в связке `json_file_seal_persist` / `append_block_for_epoch` / `load_snapshot_timed`.

## Style / Safety / Tests

Прод-код не менялся. Логика явная; тесты `incremental.rs` покрывают sync при `save_snapshot` / `json_file_runtime_persist`, но не интеграционно фиксируют частый WARN при режиме «только seal» между границами чекпоинта как ожидаемое поведение старта.

## Verdict (pwm-review)

**RFC-CHANGE** — текущее поведение согласовано с выбранной экономией записей summary, но конфликтует с UX (частый WARN и принудительный full verify при дефолтном trust-load). Исправление прогресса replay и/или политики выравнивания summary vs эвристики загрузки — предмет отдельного слайса.

---

## Update 2026-05-08 (strategy B, JsonFile demo)

В `pwmd` seal-путь переведён на пакетный вызов persist только на границе `SNAP_CHK_BLK_IV`: lifecycle больше не дергает `save_seal_persist` на каждый блок, а вызывает его на кратных 100 высотах. При этом сам `json_file_seal_persist` теперь делает полный `sync_epoch_to_tip + save_checkpoint_summary`, чтобы manifest/summary сразу выравнивались до текущего tip при каждом редком persist-вызове.

`POST /v1/shutdown` и Json fallback из ClickHouse используют режим полного flush того же типа (safe-first для demo durability): весь pending хвост epochs догоняется до tip и summary переписывается. Это снижает постоянный I/O в runtime, но между чекпоинтами при нештатном останове (`kill -9`) всё ещё возможна потеря последнего хвоста — ожидаемо для demo JsonFile режима.

---

## Participation / token estimate

```json
{
  "agent": "pwm-review",
  "result": "PASS",
  "artifacts": "docs/reviews/20260507-snapshot-summary-manifest-lag.md",
  "token_usage": {
    "source": "estimate",
    "input": null,
    "output": null,
    "total": 5500,
    "confidence": "low"
  }
}
```

**Ticket artifact verdict:** `RFC-CHANGE`
