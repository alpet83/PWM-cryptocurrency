# Sprint V2-2 — Slice 1 review: единый `marks`, вывод `State.marks_quota`

**Date:** 2026-05-06  
**Agent:** `pwm-review`  
**Commit reviewed:** `6c52b71` — `feat(core): drop marks_quota mirror and use single marks state`

---

## 1. Scope recap

Слайс закрывает пункты **`docs/plans/mvp_v2.md`** §Sprint **V2-2** (ядро + согласование состояния): **`BURN_MARK` и начисление марок на `Account.marks`**, удаление зеркала **`State.marks_quota`** из **`pwm-core`**, перенос совместимости в слой **snapshot** (`pwmd`): чтение legacy-поля только при строгом **`quota == account.marks`**, канонические записи без пустого/лишнего зеркала; обновления **`docs/pwm-core.md`**, запись в **`issues-report.md`**, правки **`pwm-cli`** (текст help), снятие **`normalize_marks_quota`** из bootstrap/`Inner`.

Тикет: `tasks/20260506-v2-sprint2-double-balance.json` (слайс 1 в `slices["1"]`).

---

## 2. Requirements fit

- **Ядро:** поле `marks_quota` удалено из `pwm_core::state::State`; сняты `normalize_marks_quota`, `marks_quota_of` и все записи зеркала на путях создания аккаунта, burn, claim и после accrue. Burn/claim тесты переведены на проверку **`after.marks`**, без опоры на параллельную карту — соответствует цели «один консенсусный счётчик марок».
- **Снапшоты:** при десериализации строки `marks_quota` валидируются через общую **`validate_quota_rows`**: дубликаты id, orphan id, **mismatch** относительно уже распарсенных `accounts` — явные ошибки; в собранный **`ChainState`** поле `marks_quota` больше не кладётся (карта не строится). При сериализации — **`Vec::new()`** + **`skip_serializing_if = "Vec::is_empty"`**, то есть канонический JSON без ключа/пустого массива для новых дампов (для wire-формы — пустой вектор не сериализуется).
- **Bootstrap / `Inner`:** вызовы нормализации зеркала убраны; состояние после загрузки опирается только на аккаунты — согласовано с ядром.
- **CLI:** подсказка burn указывает **marks**, а не `marks_quota` — согласовано с продуктовой семантикой v2.
- **Документация:** `docs/pwm-core.md` фиксирует отсутствие отдельного `marks_quota` в состоянии; `issues-report.md` описывает риск legacy-файлов и строгий loader — уместно.

**Частичный/внешний gap (не блокер слайса):** в **`mvp_v2.md`** блок «Текущее состояние кода (ориентиры)» всё ещё описывает `marks_quota` как наследие в `state.rs` и привязку accrue к квоте — текст плана **устарел** относительно `6c52b71`; это тема отдельного docs-прохода, не ошибка реализации в коммите.

**Согласование с Slice 0 (API freeze):** в диффе **нет** изменений `crates/pwmd/src/api/types.rs` / `common.rs`. Публичный **`AcctOut.marks`** по-прежнему одно поле; отдельного `marks_quota` в REST нет — совпадает с **`docs/reviews/sprint-v2-2-slice0-account-api-freeze.md`**.

**Заметка о дрейфе документа Slice 0:** в том freeze в §«Note on `marks_quota`» указано, что зеркало есть во внутреннем `State`; после Slice 1 это **не так** — зеркало осталось только как **legacy-поле в snapshot JSON**. Имеет смысл поправить формулировку в следующем docs-тикете, чтобы не вводить читателя в заблуждение.

---

## 3. Style and module shape

- **Имена:** в изменённых участках **`state.rs`** / **`snapshot/types.rs`** / тестах — укладываются в лимиты; тестовые переименования (`precheck_tip_next_ctx`, `stake_autoclaim_zero_matured`, и т.д.) снижают длину имён — в духе **`AGENT_PROMPT_testing.md`**.
- **Автопроверка:** запущен `python scripts/check_rust_fn_name_segments.py` для путей слайса. **Нарушений в диффе не введено.** Отчёт скрипта показывает **пять** существующих production-имён с **5** сегментами в **`crates/pwmd/src/bootstrap.rs`** (`app_from_dev_net_shard`, `app_from_genesis_in_shard`, …) — строки **не затронуты** телом коммита `6c52b71` (только соседние блоки с `Inner` / `normalize_marks_quota`). Классификация: **наследованный долг**, не повод к **REQUEST_CHANGES** для Slice 1; при желании оркестратора — отдельный cleanup-слайс или waiver.
- **Модульность:** вынесение **`validate_quota_rows`** убирает дублирование между двумя путями десериализации — удачно.
- **Комментарии:** пояснения к `serde` для legacy-поля на английском — ок.

---

## 4. Safety

- **Миграция снапшотов:** главный риск — тихое расхождение зеркала и `marks`; он закрыт **строгой** проверкой equality и явными сообщениями об ошибках (orphan / mismatch).
- **Паники / unwrap:** в просмотренном диффе новых небезопасных паттернов в горячих путях не добавлено.
- **Межшард / RPC:** слайс не меняет контракт handlers; поведение марок консистентно с единым источником в состоянии.

---

## 5. Tests

- **`pwm-core`:** тесты burn/claim обновлены под единый `marks`; переименования тестов без изменения смысла проверок.
- **`pwmd`:** в **`snapshot_roaming`** добавлен **`snap_reject_quota_mismatch`**; существующий сценарий orphan id скорректирован (явное `marks_quota: []` перед мутацией массива) и ожидаемая подстрока ошибки приведена к сообщениям loader — покрывает негативные ветки `validate_quota_rows`.
- **Конвейер:** в тикете зафиксирован **`pwm-testing` PASS** после `6c52b71` (полный прогон `pwm-core`, `pwmd`, `pwm-cli`, bench compile-only) — приемлемое внешнее подтверждение для данного ревью.

**Возможный дополнительный сценарий (низкий приоритет):** десериализация legacy-снапшота с **непустым** корректным `marks_quota`, совпадающим с `accounts` (smoke «старый файл всё ещё грузится») — если ещё не покрыт другими тестами, можно добавить позже; для слайса 1 достаточны отказ на mismatch/orphan + зелёный полный прогон.

---

## 6. Verdict

**Approve with nits**

**Nits (не блокируют merge):**

1. Обновить **`docs/plans/mvp_v2.md`** §ориентиры кода под отсутствие `marks_quota` в `State` (отдельный docs PR).
2. Уточнить **`sprint-v2-2-slice0-account-api-freeze.md`** §про `marks_quota` (теперь только legacy в snapshot, не в `pwm_core::State`).
3. Опционально: backlog cleanup для 5-сегментных `fn` в `bootstrap.rs` или формальный waiver.

---

## 7. Participation / token estimate (orchestrator)

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/sprint-v2-2-slice1-review-20260506.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 12000
  confidence: medium
done_at: 2026-05-06T00:00:00+03:00
```

*(Verdict для цитирования оркестратором: **Approve with nits** / machine `result: PASS` с зафиксированными nits.)*

---

**Verdict:** **Approve with nits**
