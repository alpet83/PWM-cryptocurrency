# V2-4 Slice 1 — независимое ревью (pwm-review)

**Дата:** 2026-05-06  
**Коммит (продукт):** `1ffb84015e19032de9b377fb451b1080c62b62a8`  
**Коммит `680040a` (отчёт тестирования):** в объёме продуктового ревью не рассматривался.

---

## Итоговый вердикт

**FAIL** — блокирующие расхождения с объявленной целью среза и с контрактом pwmd HTTP.

---

## Таблица критериев приёмимости (v2-4-slice0-ux-freeze)

| ID | Критерий | Статус | Комментарий |
|----|----------|--------|-------------|
| AC-1 | CLI: баланс/аккаунт показывает `marks` | **FAIL** | Отдельной команды `acct show` в `pwm-cli` нет; в этом коммите вывод marks в пользовательские команды не добавлен (кроме побочных строк в `tx-unstake`, см. AC-2). |
| AC-2 | CLI `tx-burn-mark`: marks до сабмита и подтверждение после | **FAIL** | `run_tx_burn_mark` **не** вызывает `fetch_marks` и не печатает marks до подписи/отправки. Импорт `fetch_marks` и вызовы попали в **`run_tx_unstake`**, а не в burn. |
| AC-3 | TUI: таблица аккаунтов показывает marks | **NOTE** | В таблице Owner/Receivers по-прежнему только колонки «Address» и «Balance»; marks видны в **нижней панели детализации** (`Marks: N`) и в JSON debug — это частично закрывает «sub-row» из freeze, но не полноценную колонку в таблице. |
| AC-4 | TUI F5: текущий баланс marks сверху формы | **PASS** | `render_burn_modal` выводит строку `Current marks: {}`; при открытии F5 значение берётся из `owner.marks`. |
| AC-5 | Негативный тест `InsufficientMarks` | **PASS** | `tx_burn_err_insufficient_marks` проверяет цепочку `post_signed_tx` при HTTP 400 и теле с `InsufficientMarks` для `TxBody::BurnMark` — сценарий не тривиален. Он не прогоняет полный `run_tx_burn_mark` и не затрагивает `fetch_marks`. |
| AC-6 | Единообразие текста ошибок CLI/TUI | **NOTE** | Вне среза Slice 3; см. раздел «Сообщения об ошибках». |
| AC-7 | Документация tester-guide | **NOTE** | Не в объёме коммита `1ffb840`. |

---

## Scope recap

Заявлено: marks в модели TUI, парсинг из RPC, отображение в деталях и F5, CLI pre-check для burn, хелпер `fetch_marks`, негативный тест, переименования хелперов.

Фактически по коду на `1ffb840`: TUI-часть (модель, poll, F5, detail) в целом согласована; **CLI burn pre-check отсутствует**; **`fetch_marks` использует несуществующий у pwmd путь**; часть логики ошибочно добавлена в **`tx-unstake`** с текстом «burn submitted».

---

## Requirements fit

- **Парсинг `marks` в TUI:** поле `x["marks"]` через `parse_u128` — согласовано с типичным представлением `AcctOut` (строка или число). При отсутствии/битом значении `parse_u128` даёт **0** (как и для баланса) — отдельного sentinel для «неизвестно» нет; для marks это может вводить в заблуждение по сравнению с `???` для баланса/nonce.
- **`fetch_marks` и контракт API:** в `pwmd` зарегистрированы `GET /v1/accounts` (список) и `GET /v1/account/:id` (один аккаунт). Реализация запрашивает **`GET /v1/accounts/{hex}`**, маршрута с таким шаблоном **нет** — ожидается стабильный отказ (404) и невозможность получить marks через этот хелпер на эталонном узле.
- **Ожидаемый путь (для выравнивания):** тот же, что у `fetch_nonce` — `/v1/account/{hex_id}`; парсинг `marks` как decimal string или число — ок.

---

## Style

- Сегменты имён: `python scripts/check_rust_fn_name_segments.py` на перечисленные файлы — **нарушений нет** (`violations: []`).
- Переименования `fmt_wallet_acct_line`, `parse_nonce_acct_json`: остаточных ссылок на старые имена в коде не найдено (поиск по репозиторию).

---

## Safety

- Новых опасных `unwrap` в горячих путях не выделяется; `fetch_marks` при ошибке HTTP/JSON возвращает `Result`.
- Доверие к RPC: ошибочный URL ломает **новый** pre-submit путь для `tx-unstake` (или любой вызов `fetch_marks`).

---

## Tests

- Покрытие отклонения tx для burn через `post_signed_tx` — хорошо для surfacing ошибки узла.
- Нет теста, что `fetch_marks` бьёт в существующий endpoint и парсит реальный JSON.
- Нет интеграционного теста на `run_tx_burn_mark` + marks до отправки.

---

## Сообщения об ошибках

- CLI: `format_tx_submit_error` при JSON-ответе отклонения использует `summarize_tx_reject_json` (структурированная строка `reject: code=…`). При не-JSON теле (как в юнит-тесте `InsufficientMarks`) пользователь видит **сырой фрагмент тела** — это ожидаемо для fallback.
- TUI: в `BurnForm::apply_submit_result` в статус уходит **строка ошибки как есть** (из submit слоя). Полного совпадения с CLI при разных форматах тела нет — соответствует ожиданию AC-6 в Slice 3.

---

## Nits (дополнительно к блокерам)

- После успешного `tx-unstake` в stderr выводится **`pwm: burn submitted; marks before: …`** — семантически неверно для unstake.
- Импорт `fetch_marks` в `cmd_tx.rs` используется только в `run_tx_unstake`; для burn — мёртвый импорт с точки зрения заявленной фичи (после исправления путей стоит перенести вызов в `run_tx_burn_mark`).

---

## Verdict (кратко)

**Request changes:** исправить URL `fetch_marks` на `GET /v1/account/{id}`, перенести pre/post печать marks в **`run_tx_burn_mark`** (и поправить тексты eprintln для unstake, если marks там остаются), доработать AC-1/AC-3 по согласованию с владельцем (колонка таблицы vs только detail).

---

## Participation (для тикета)

```yaml
agent: pwm-review
result: FAIL
artifacts: docs/reviews/v2-4-slice1-review-20260506.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 12000
  confidence: low
```

---

## Git handoff (оркестратору)

См. финальный блок в теле ответа агента (powershell, `# git-handoff`).

---

## Re-review after fix commit 8e0161a

**Продуктовый фикс-коммит:** `8e0161a` (поверх исходного `1ffb840`).

### Сводка проверки

- **`rpc_helpers::fetch_marks`:** запрос на `{rpc_base}/v1/account/{hex}` — тот же шаблон, что у nonce-init; ошибочного `/v1/accounts/…` нет.
- **`run_tx_burn_mark`:** вызывает `fetch_marks`, предупреждает при сбое fetch, печатает `pwm: current marks: …` до подписи/отправки; после `post_signed_tx` — `pwm: burn submitted; marks before: …`. **`run_tx_unstake`** не вызывает `fetch_marks`, без ложных сообщений про burn.
- **TUI-таблица:** заголовок `panel_head_row()` включает колонку **Marks**; строки Owner и Receivers выводят `r.marks` с согласованными `Constraint`.
- **F5:** в `tui_loop` в подсказке формы по-прежнему строка вида **Current marks: …** (`form.marks_available`).
- **Имена символов:** `python scripts/check_rust_fn_name_segments.py` на `rpc_helpers.rs`, `cmd_tx.rs`, `tui_loop.rs` → **`violations: []`**.

### AC spot-check (относительно `v2-4-slice0-ux-freeze`)

| ID | Статус | Замечание |
|----|--------|-----------|
| AC-2 | **PASS** | Marks до сабмита с RPC выполнены; после сабмита выводится подтверждение отправки и повтор того же значения «до» (повторного fetch «после» нет — см. nits). |
| AC-3 | **PASS** | Колонка Marks в таблице владельца и получателей. |
| AC-4 | **PASS** | Read-only текущий баланс marks в F5 сохранён. |
| AC-1 | **NOTE** | Вне дельты `8e0161a`: отдельная команда «acct show» с marks по-прежнему не заявлена как закрытая этим коммитом (как и в первом ревью). |

### Вердикт (re-review)

**PASS-WITH-NITS** — прежние блокеры (URL, привязка pre-check к burn, колонка в таблице) устранены. Нит: при желании строго «marks после tx» — повторный `fetch_marks` или явная подпись ожидаемого остатка; **AC-1** остаётся на усмотрение владельца среза/спринта.

### Participation (re-review)

```yaml
agent: pwm-review
slice: 1-final
result: PASS-WITH-NITS
artifacts: docs/reviews/v2-4-slice1-review-20260506.md
product_commits_reviewed:
  - 1ffb840
  - 8e0161a
token_usage:
  source: estimate
  input: null
  output: null
  total: 8000
  confidence: low
```
