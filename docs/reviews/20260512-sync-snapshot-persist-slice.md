# Review: sync periodic autosnapshot after `apply_blk_batch` (slice 20260512)

**Ticket:** `tasks/20260512-slice-nonsealing-sync-snapshot-persist.json`  
**Verdict:** **Approve with nits**

---

## 1. Scope recap

Слайс добавляет периодический autosnapshot после успешного peer-sync **`apply_blk_batch`**, когда `autosnap_hit(tip_h)`, через общие **`periodic_snap_save`** / **`periodic_snap_finish`** (`lifecycle.rs`) с источником **`sync_apply`** в логе checkpoint. Целевые файлы: `lifecycle.rs`, `transport/peer_session/sync_live.rs`. Тест: **`batch_cross_ckpt_writes_snap`** в `sync_live.rs` (JsonFile, 100 блоков за один батч).

---

## 2. Requirements fit

- **Семантика checkpoint:** совпадает с seal-path по коду: тот же `SealPersistMode::Periodic`, тот же `save_seal_persist`, та же цепочка **`apply_snapshot_init_state`** при успехе/ошибке через **`periodic_snap_finish`**.
- **Условие срабатывания:** `autosnapshot_backend.is_some()` и `autosnap_hit(tip_h)` после полного применения батча — соответствует тикету.
- **Откат при ошибке persist:** при неуспехе записи вызывается **`rollback_commit`** с резервной копией, если checkpoint реально запускался (`save_result.is_some()`), аналогично тому, как seal цепляет `bak_opt` только на границе.

**Зазор (осознанный vs seal):** на sync-path **`take_bak`** делается **в начале всего батча** (до цикла по блокам), а не «ровно на один блок до границы 100», как в цикле seal (`persist_back` завязан на `now_h+1`). При большом батче, пересекающем границу mod-100, откат по ошибке persist откатывает **весь применённый батч** (верхняя граница по коду — до ~32 блоков: `SYNC_BLK_REQ_CAP` / `SYNC_CUP_CHUNK_CAP`), а не один последний блок. Это согласуется с атомарностью «применили пачку → попытались persist», но **не идентично** seal по гранулярности; для прод-рисков при сбое диска/CH это лучше явно зафиксировать в дизайн-заметках, чтобы не ожидали откат «как один seal».

- **Документация / CHANGELOG:** в тикете acceptance допускает опциональную строку; в диффе слайса отдельной записи в `CHANGELOG.md` не видно — **низкий приоритет**, не блокер.

---

## 3. Style and module shape

- Общий helper вынесен в `lifecycle` (`periodic_snap_*`), дублирование логики persist/rollback минимально.
- **`python scripts/check_rust_fn_name_segments.py`** по заявленным путям: **violations пустые** (policy prod ≤4, test ≤5).
- Wire / `PWM_PROTOCOL_VERSION`: не затрагивается.

---

## 4. Safety

- **Паники / hot-path unwrap:** новых подозрительных мест в показанной логике нет; путь async остаётся прежним по блокировкам (`inner.write` → `drop` → `periodic_snap_finish`).
- **Доверие / DoS:** лимиты размера батча прежние; дополнительной поверхности атаки нет.
- **ClickHouse:** используется тот же **`save_seal_persist`** и существующий **`ch_save_seal_fallback`**; отдельных новых рисков нет, кроме уже известных (сеть, отсутствие `json_fallback` → жёсткая ошибка без JsonFile escape).
- **Двойная запись (seal + sync):** для standby / `debug-disable-seal-loop` локальный seal отсутствует; для active sealer теоретически возможны лишние записи на той же высоте — идемпотентность «перезаписать тот же tip» приемлема; лог различает `source=seal` vs `sync_apply`.

---

## 5. Tests

- **`batch_cross_ckpt_writes_snap`:** покрывает счастливый путь JsonFile: `apply_blk_batch` с ровно `AUTOSNAPSHOT_BLOCK_INTERVAL` блоками, проверка epoch manifest и `canonical_h`.
- **Пробелы (ожидаемо по notes тикета):** нет сценария **ошибки persist → rollback**; нет теста с **ClickHouse** feature; нет полного **E2E** peer TCP — для слайса приемлемо как минимальный контракт, но остаётся регрессионный долг.

---

## 6. Verdict

**Approve with nits.**

**Nits (приоритет):**

1. Сообщение в **`apply_snapshot_init_state`** при ошибке (`snapshot save after seal failed`) вводит в заблуждение для **`sync_apply`** — стоит обобщить формулировку или передавать источник события (для `pwm-coding`).
2. Зафиксировать в коротком комментарии или design note **гранулярность отката** sync-batch vs seal (до размера батча).
3. По желанию владельца: одна строка в **CHANGELOG** после merge.

---

## 7. Participation / token estimate

```json
{
  "agent": "pwm-review",
  "result": "PASS",
  "artifacts": ["docs/reviews/20260512-sync-snapshot-persist-slice.md"],
  "token_usage": {
    "source": "estimate",
    "input": null,
    "output": null,
    "total": 4500,
    "confidence": "low"
  }
}
```

*(Оценочный расход токенов; точных счётчиков провайдера нет.)*

---

## Sprint-final glossary

Не финальное ревью спринта — **GLOSSARY.md не изменялся.**
