# Slice 2 — тестирование: divergence dump + time-align (2026-05-09)

**Коммиты:** `ad0bee1` (coding), `e3e0e24` (обновление тикета).  
**Тикет:** `tasks/20260509-protocol-versioning-debug-controls.json`

## Вердикт: **PASS**

## 1) Триггер дампа при расхождении и ограничение по числу файлов

- **Код (стрек пер-пир):** при `Ok(Some(div))` инкрементируется `div_streak`; дамп выполняется только если `app.debug_dump.on_divergence && div_streak >= app.debug_dump.trigger_streak.max(2)`. При успешном совпадении tip (`Ok(None)`) стрек сбрасывается в `0`.
- **Кап записей:** `dump_blk_json` проверяет `dump_count >= max_files` → `DumpWrite::CapReached`; после успешной записи — `fetch_add(1)`.

Ключевой фрагмент триггера в транспорте:

```645:674:crates/pwmd/src/transport/peer_session/mod.rs
                    if app.debug_dump.on_divergence
                        && div_streak >= app.debug_dump.trigger_streak.max(2)
                    {
                        let blk_opt = {
                            let g = app.inner.read().await;
                            g.chain
                                .blocks
                                .iter()
                                .rev()
                                .find(|blk| {
                                    blk.hdr.height == div.local_h
                                        && hex::encode(hdr_hash(&blk.hdr)) == div.local_hash
                                })
                                .cloned()
                        };
                        if let Some(blk) = blk_opt {
                            match dump_blk_json(app, &blk, "divergence_probe", node_id) {
                                Ok(DumpWrite::Wrote(path)) => info!(
                                    target: "pwmd::peer",
                                    "divergence debug dump written node_id={} height={} path={}",
                                    node_id,
                                    blk.hdr.height,
                                    path.display()
                                ),
                                Ok(DumpWrite::CapReached) => warn!(
                                    target: "pwmd::peer",
                                    "divergence debug dump skipped node_id={} reason=cap_reached cap={}",
                                    node_id,
                                    app.debug_dump.max_files.max(1)
                                ),
```

- **Автотесты:** прямого интеграционного теста на «два подряд SyncTipDivergence» нет; поведение стрека и капа подтверждено ревью кода + юнит `div_dump_writes_block_file` (одна успешная запись).

## 2) Именование пути: `…/blocks/b{height}.json` и настраиваемый каталог

- **Имя файла:** `dump_path` → `{base}/b{height}.json`.
- **База каталога:** явный `debug_dump.dir` из CLI/env; иначе `parent(data_file)/blocks`; иначе fallback `state/blocks`.

```48:62:crates/pwmd/src/debug_dump.rs
pub(crate) fn dump_path(base_dir: &Path, height: u64) -> PathBuf {
    base_dir.join(format!("b{height}.json"))
}

fn dump_dir(app: &App) -> PathBuf {
    if let Some(dir) = app.debug_dump.dir.clone() {
        return dir;
    }
    if let Some(path) = app.data_file.as_ref() {
        if let Some(parent) = path.parent() {
            return parent.join("blocks");
        }
    }
    PathBuf::from("state").join("blocks")
}
```

- **Тесты:** `dump_path_uses_b_height`; `div_dump_writes_block_file` пишет во временный корень с явным `dir`.

## 3) Режим выравнивания к середине секунды и приоритет над deterministic seal-time

- **Флаг эффективности:** `align_mid_on(debug_align_mid, debug_det_seal_time)` → align активен только если включён align и **выключен** deterministic seal-time.
- **Seal loop:** `maybe_align_mid` вызывается перед `chain.seal` только если `app.debug_align_mid`.
- **Старт:** при одновременном включении align и deterministic — предупреждение «align ignored … deterministic wins»; effective `app.debug_align_mid` снимается через `align_mid_on`.

```30:32:crates/pwmd/src/debug_dump.rs
pub(crate) fn align_mid_on(debug_align_mid: bool, debug_det_seal_time: bool) -> bool {
    debug_align_mid && !debug_det_seal_time
}
```

```563:598:crates/pwmd/src/lifecycle.rs
    app.debug_align_mid = align_mid_on(config.debug_align_mid, config.debug_det_seal_time);
    // ...
    if config.debug_align_mid && config.debug_det_seal_time {
        warn!(
            "debug-align-seal-mid-second ignored because debug-deterministic-seal-time is active (deterministic mode wins)"
        );
    } else if app.debug_align_mid {
        warn!(
            "debug-align-seal-mid-second active (test/dev-only): seal loop is aligned near mid-second with bounded wait"
        );
    }
```

- **Тесты:** `align_det_wins_over_mid`; `mid_wait_stays_bounded` (ожидание ограничено политикой `MID_WAIT_CAP_MS` в реализации).

## 4) По умолчанию OFF и регрессии «обычного» пути

- **Dump / defaults:** `DebugDumpCfg::default()` и CLI — `on_divergence = false`, cap 16, streak 2; тест `dump_on_div_default_off`.
- **Align:** в `PwmdConfig::default()` поле `debug_align_mid: false` (без отдельного теста с именем align-default; совпадает с bootstrap defaults).
- **Дымовой регресс handshake (slice1):** `cargo test -p pwmd handshake --lib` — 16 passed.

## Команды

```text
powershell -NoProfile -ExecutionPolicy Bypass -File tools/dev/preflight_target_debug.ps1
# pwm-testing preflight: target/debug … (threshold 4096MiB) — ok

cargo check -p pwmd
# Finished dev profile (ok)

cargo test -p pwmd debug_dump::tests -- --nocapture
# 4 passed (align_det_wins_over_mid, dump_path_uses_b_height, mid_wait_stays_bounded, div_dump_writes_block_file)

cargo test -p pwmd dump_on_div_default_off -- --nocapture
# 1 passed

cargo test -p pwmd handshake --lib -- --nocapture
# 16 passed
```

## Участие / handoff

- **agent:** pwm-testing  
- **result:** PASS  
- **artifacts:** этот файл  
- **preflight_target_debug:** ~226 MiB под порогом; removed: no; script: `preflight_target_debug.ps1`  
- **cleanup:** фоновых `pwmd`/`pwm-tui` не запускалось; временные файлы теста `div_dump_writes_block_file` удаляются в самом тесте  
- **token_usage:** estimate `{ "source": "estimate", "input": null, "output": null, "total": 14000, "confidence": "low" }`

## Остаточные риски (кратко)

- Нет автотеста на двухшаговый стрек перед дампом и на ветку `DumpWrite::CapReached` после исчерпания капа.
