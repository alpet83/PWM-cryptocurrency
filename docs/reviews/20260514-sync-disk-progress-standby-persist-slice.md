# Review: sync disk progress + standby persist (`20260514-slice-sync-disk-progress-standby-persist`)

## Verdict summary

**PASS** — требования тикета и RCA закрыты; `cargo test -p pwmd` зелёный; политика имён Rust для затронутых путей без нарушений.

---

## 1. Scope recap

Слайс по тикету `tasks/20260514-slice-sync-disk-progress-standby-persist.json` и RCA `tasks/20260514-slice-sync-disk-progress-standby-persist-debug.md`:

- прогресс синхронизации: разделение память / диск / цель, без ложного «100%» при неизвестном peer tip и нулевой базе;
- `apply_blk_batch`: детект пересечения границ autosnapshot (mod 100) **внутри** батча, не только по финальному tip;
- для `SealRole::Standby`: дополнительные checkpoint-персисты на высоте 1 и каждые 10 блоков (`STANDBY_SYNC_FLUSH_BLK_IV` рядом с `AUTOSNAPSHOT_BLOCK_INTERVAL`);
- атомарный `last_snapshot_height` после успешного save/load старта;
- откат цепочки при ошибке persist через общий `periodic_snap_finish` и сохранённый `CommitBak`;
- регрессионные тесты на пересечение 100 внутри батча и standby до высоты 100.

Затронутые основные файлы: `crates/pwmd/src/state.rs`, `bootstrap.rs`, `lifecycle.rs`, `transport/peer_session/sync_live.rs`.

---

## 2. Requirements fit

**Соответствует.**

- **Ложное 100% при genesis / `peer_tip == 0`:** `sync_prog_snap` возвращает `None`, если вычисленный `goal == 0` (в т.ч. `local_h == persisted_h == 0` при нулевом peer tip). Прогресс не логируется как завершённый в этом состоянии; тест `sync_prog_snap_goal_rules` закрепляет `sync_prog_snap(0, 0, 0) == None` и отсутствие ложного завершения в `sync_prog_tick` для `(0,0,0)`.

- **Пересечение mod-100 внутри sync-батча:** диапазон `tip_before + 1 ..= tip_h` сканируется на `autosnap_hit`; один вызов `save_seal_persist` на конец батча при любом пересечении — соответствует рекомендации RCA «не только финальный tip». Регресс `batch_cross_ckpt_writes_snap` (96 + 9 блоков → tip 105) подтверждает появление manifest при пересечении 100 без tip ровно на 100.

- **Standby 1 и каждые 10 блоков:** условие `h == 1 || h % STANDBY_SYNC_FLUSH_BLK_IV == 0` только при `SealRole::Standby`; proposer/active не получают этот канал — согласовано с заметкой тикета про seal path proposer.

- **`last_snapshot_height`:** поле в `App`, инициализация в bootstrap; `Ordering::Release` после успешной записи в `apply_snapshot_init_state`; после загрузки снимка в runtime — store по фактическому tip; при пустом снимке — 0. При ошибке persist атомик не обновляется на успех.

- **Rollback + `periodic_snap_finish`:** при непустом `save_result` резервный снимок состояния извлекается из `bak_opt`; при `Err` вызывается `rollback_commit`, затем деградация init-state; при `Ok` — обновление высоты и ready — паритет с intent RCA.

**Замечание (нит):** при `peer_tip_h == 0`, но уже ненулевой `local_h` или `persisted_h`, цель прогресса берётся как `local_h.max(persisted_h)` без явной пометки «peer unknown». Это разумная эвристика для отображения, но отличается от формулировки RCA «suppress until non-zero peer tip»; при желании операторской ясности можно позже уточнить текст лога или отдельное поле состояния — не блокер для приёмки слайса.

---

## 3. Style and module shape

- Модульные баннеры и английские комментарии в затронутых местах выглядят согласованно с существующим стилем (`lifecycle.rs`, `sync_live.rs`).
- Запуск `python scripts/check_rust_fn_name_segments.py` для `state.rs`, `bootstrap.rs`, `lifecycle.rs`, `sync_live.rs`: **violations пустые** (prod ≤ 4 сегментов, тесты ≤ 5).
- Крупный расползание `main.rs` / фасада не затрагивается слайсом.
- Wire/handshake версия не менялась; поведение транспорта на уровне совместимости не затронуто с точки зрения semver протокола.

---

## 4. Safety

- Персист только при наличии backend; блокировка inner снимается до IO через типичный паттерн seal/sync.
- Откат памяти при сбое записи снимка снижает риск рассинхрона «память записана, диск нет» без обновления `last_snapshot_height`.
- Дополнительные вызовы `take_bak` на каждый успешный батч дают O(размер состояния) клон при применении блоков — осознанная цена надёжности; узких паник или новых `unwrap` в горячем пути по слайсу не выявлено.

---

## 5. Tests

- Юниты на математику прогресса и throttle (`sync_prog_snap_goal_rules`, `sync_prog_tick_*`).
- Интеграционные async-тесты: пересечение autosnapshot внутри батча (`batch_cross_ckpt_writes_snap`), standby-персист до 100 (`standby_batch_cross_10_writes`).
- Выполнено локально: **`cargo test -p pwmd`** — 335 + прочие таргеты crate, **все прошли**.

Пробелы на будущее (не блокер): отдельный тест на симуляцию **ошибки** `save_seal_persist` в sync-пути с проверкой rollback и неизменного `last_snapshot_height` был бы сильным дополнением; в рамках текущего диффа покрытие выглядит достаточным для заявленной регрессии.

---

## 6. Verdict

**approve with nits** — функционально готово к слиянию; нит сверху про семантику цели при `peer_tip == 0` и ненулевой локальной/дисковой высоте (документация/лог), без требования обязательного код-изменения в этом ревью.

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260514-sync-disk-progress-standby-persist-slice.md
token_usage:
  source: estimate
  input: 28000
  output: 4200
  total: 32200
  confidence: medium
```

Финальное ревью спринта: **нет** — обновление `docs/GLOSSARY.md` не требуется в этом отчёте.

---

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260514-sync-disk-progress-standby-persist-slice.md'
git commit -m 'docs(review): sync disk progress standby persist slice'
```
