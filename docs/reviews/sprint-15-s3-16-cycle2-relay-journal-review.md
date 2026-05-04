# S15-S3.16 cycle2 — журналы релея и импорта (pwm-review)

**Тикет:** `tasks/20260430-s15-slice3-16-cycle2-xshard-credit-tui-step5.json`  
**Поиск по коду:** `rg 'relay:|export-provenance|tx commit delta|registered:export' crates/pwmd/src`

## 1. Что видно в журналах нод (терминалы `node-1.ps1` / `node-2.ps1`)

### CY (source, `:3030`, peer `:3130` → seed DO `:3131`)

- После перезапуска снапшот **загрузился**, фаза `ready`.
- Периодически: `export/import summary: … exported_count=1 … imported_count=0 … pending_count=1` — состояние моста **не закрыто** (импорт по сводке не зафиксирован).
- В доступном хвосте лога **нет** строк с префиксом **`relay:`** (`relay: begin select_target`, `relay: POST /v1/export-provenance`, `relay: POST /v1/tx (import)`).  
  **Интерпретация:** либо **Import на source после `relayed` не отправлялся** (старый клиент, ручной сценарий, ошибка до HTTP), либо отправка была **вне окна** лога; если бы `relay_import` ходил на peer и получал 4xx/5xx, на CY ожидались бы **`relay: import HTTP error`** / **`peer relay import unavailable`**.

### DO (target, `:3031`, peer `:3131` → seed CY `:3130`)

- При старте: **`snapshot load failed (fallback to genesis state): snapshot chain mismatch: block[16] state_root does not match replayed state`** → фаза **`ready_degraded`** (см. `lifecycle.rs` / `InitState::ready_degraded`).
- **`is_ready()`** для `ReadyDegraded` всё ещё **true** (`state.rs`), т.е. **`POST /v1/tx` и handoff не режутся только фазой** — отказ должен быть по телу ответа или валидации, а не `503 node is not ready`.
- В хвосте лога DO **нет** строк **`tx commit delta: kind=import`** (они пишутся в `api.rs` после успешного seal Import).
- **`POST /v1/export-provenance`** (`v1_export_handoff_register`) **не пишет `info!` на входе** — только `push_flow` с видом `registered:export_provenance` во **внутреннюю** очередь (`recent_flow`), в консоль это **не попадает**.

## 2. Пробел покрытия логами (не только «релей не доставил»)

| Событие | Где | В консоль (tracing) |
|--------|-----|---------------------|
| Исходящий handoff / import с source | `relay.rs` | Да: `relay: …` |
| Входящий handoff на target | `api.rs` `v1_export_handoff_register` | **Нет** отдельной INFO |
| Входящий Import, успешный seal | `api.rs` `v1_tx` | Да: `tx commit delta: kind=import …` |
| Отклонение до seal | `v1_tx` | Частично (HTTP тело клиенту; на target не всегда дубль в warn) |

Отсюда ощущение «на целевой ноде тишина»: **даже при успешной регистрации handoff в логе может не быть явной метки**, пока не случится `tx commit delta` для Import.

## 3. Выводы для следующего копания (pwm-coding / оператор)

1. **Подтвердить факт отправки Import с CY:** на **source** в момент теста искать **`relay:`** и **`peer relay`**. Если пусто — проблема **до** сети (TUI/CLI, nonce, ключ получателя, `PWM_RPC`).
2. **Если `relay:` есть, но `import HTTP error`:** смотреть тело ответа (snippet уже в warn на source) — provenance, `export_id is not known`, recipient gate, duplicate import и т.д.
3. **Состояние DO:** `ready_degraded` из‑за битого снапшота — для чистого E2E либо **удалить `pwm-data.json` / state-root** на DO и поднять с genesis, либо починить снапшот; иначе сложно отделить «релей не дошёл» от «стейт целевой ноды невалиден».
4. **Улучшение наблюдаемости (низкий риск):** на target добавить **`info!`** в начале `v1_export_handoff_register` и при принятии Import (до/после seal) с `export_id` / кратким итогом — чтобы журнал DO отражал межшард даже без запроса `/v1/…/flow`.

## 4. Связь с отчётом pwm-testing

`docs/reviews/sprint-15-s3-16-cycle2-testing.md`: `total_imported_amount=0`, `pending_count=1` на CY согласуется с «импорт не завершён»; отсутствие наблюдаемых событий на DO **совместимо** и с **отсутствием доставки**, и с **дырой в логах** на handoff.

```yaml
participation:
  agent: pwm-review
  verdict: partial
  note: "Анализ по коду pwmd + хвостам терминалов; полный прогон с grep по свежим логам после намеренного cross-shard остаётся за оператором."
```
