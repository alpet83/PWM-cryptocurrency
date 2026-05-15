# V2-4 Slice 3 — финальное ревью спринта (pwm-review)

**Дата:** 2026-05-06  
**Дельта Slice 3:** `4c7a34a` — `docs/tester-guide-cli-tui-scenarios.md` (§11), комментарий у `run_tx_burn_mark` в `crates/pwm-cli/src/cmd_tx.rs`.  
**Контекст продукта (Slices 1 набор):** `1ffb840`, `8e0161a`, `81f43ec`, `22a613f` — без повторного полного диффа; проверена согласованность с текущим деревом.

---

## Вердикт по спринту V2-4 (overall)

**PASS-WITH-NITS** — операторский путь marks в CLI/TUI и покрытие тестами в целом закрыты по смыслу предыдущих срезов; Slice 3 добавляет полезный runbook в tester-guide. Остаются открытыми AC-1 и точность ожиданий по тексту отклонения burn (док-комментарий и §11 Step 5 расходятся с фактическим телом ответа pwmd).

---

## Таблица AC (`docs/reviews/v2-4-slice0-ux-freeze-20260506.md`)

| ID | Критерий | Статус | Комментарий |
|----|----------|--------|-------------|
| AC-1 | CLI `acct show` (или любая команда баланса) печатает `marks` | **FAIL** | В `pwm-cli` по-прежнему нет подкоманды `acct`; `wallet account list` / `fmt_wallet_acct_line` выводят только id/derivation, без RPC-баланса и без `marks`. §11 гайда обходит это через `wallet show` + `GET /v1/account/<hex>` — это не закрывает формулировку AC-1 про CLI-баланс. |
| AC-2 | CLI `tx-burn-mark`: marks до сабмита и подтверждение после | **PARTIAL** | До сабмита: `pwm: current marks: N` — есть. После успеха: печатается только `pwm: burn submitted; marks before: N` (то же «до», без повторного fetch после tx). Перенесённый из Slice 1 nit остаётся актуальным. |
| AC-3 | TUI: колонка `marks` в таблице | **PASS** | Модель `AcctRow`, poll и колонка Marks согласованы с финальным состоянием Slice 1 (ревью после `8e0161a`). |
| AC-4 | TUI F5: read-only текущие marks | **PASS** | `f5_build_burn_form` передаёт `owner.marks`; в `tui_loop` строка вида `Current marks: {}` отображается в модалке. |
| AC-5 | Негативный тест `InsufficientMarks` | **PARTIAL** | Тест `tx_burn_err_insufficient_marks` имитирует **сырое** тело `InsufficientMarks`, что не совпадает с реальным JSON от pwmd (`tx_reject_json`). Цепочка `post_signed_tx` всё же проверяется; для полного соответствия узлу стоило бы подставлять RFC-0014 фикстуру с `code=E_BURN_OVER_BALANCE` и текстом из `Display(TxError)`. |
| AC-6 | Согласованность текста ошибок CLI и TUI | **PARTIAL** | Для JSON-reject оба пути используют `pwm_core::summarize_tx_reject_json`: одинаковый компактный hint (`code`, `class`, `phase`, `msg`, …). Префиксы различаются: CLI — `tx submit: HTTP …`, TUI burn — `burn failed: …`. Полная строка пользователю не идентична; семантика узла — да. |
| AC-7 | Tester-guide: сценарий stake → accrue → burn | **PARTIAL** | §11 добавлена; флаги (`--mark-amount`, `--wallet`, `--rpc`) согласованы с остальным гайдом. Риск: Step 5 просит подстроку `InsufficientMarks`, тогда как эталонный pwmd отдаёт RFC JSON с кодом **`E_BURN_OVER_BALANCE`** и сообщением через **`thiserror` Display** для `InsufficientMarks` (**`insufficient marks`**, без PascalCase имени варианта). |

---

## Scope recap

- Slice 3 формально закрывает G6 (док-сценарий) и подготовку к проверке AC-6 (комментарий рядом с burn).
- Затронутые артефакты ревью: документация оператора; прод-код только комментарий (читаемость/точность).

---

## Requirements fit

1. **Tester-guide §11:** шаги stake / REST marks / happy burn / TUI smoke логичны и совпадают с текущими сообщениями CLI burn (`pwm: current marks`, `pwm: burn submitted; marks before`). Негативный шаг завышен по литералу `InsufficientMarks` относительно фактического wire-формата pwmd.
2. **Комментарий в `cmd_tx.rs`:** утверждение, что `TxError::InsufficientMarks` «сериализуется как `"InsufficientMarks"` в JSON», **не подтверждается** кодом ядра: `TxError` в `pwm-core` оформлен через `thiserror::Error`, без отдельного `Serialize`, который бы выставлял имя варианта в теле ответа. В **pwmd** `tx_reject_json` кладёт в JSON стабильный **`code`** из `tx_err_wire` (`E_BURN_OVER_BALANCE` для burn) и **`message`**, собранный из `Display`, т.е. человекочитаемую строку, а не имя enum.

---

## Style

- `python scripts/check_rust_fn_name_segments.py crates/pwm-cli/src/cmd_tx.rs` → **`violations: []`** (политика имён соблюдена для затронутого `.rs` в Slice 3).

---

## Safety

- Изменения Slice 3 не меняют исполняемую логику; рисков выполнения нет.

---

## Tests

- Регрессий по тестам Slice 3 не добавлено; актуальность негативного теста к живому контракту узла — см. AC-5.

---

## Вердикт по доставке Slice 3

**Approve with nits** для док-коммита: runbook полезен; исправить неточность комментария и ожидание Step 5 (или явно описать «эквивалент»: `E_BURN_OVER_BALANCE` / `insufficient marks`).

---

## Nits и backlog

| Приоритет | Тема |
|-----------|------|
| Nit | Комментарий у `run_tx_burn_mark`: заменить формулировку про «serialization InsufficientMarks» на описание реального RFC JSON (`E_BURN_OVER_BALANCE`, текст из Display). |
| Nit | §11 Step 5: выровнять expected substring с pwmd (или ослабить формулировку до «стабильный код отклонения burn по marks»). |
| Backlog (не блокер) | Повторный fetch marks после успешного burn в CLI (post-submit «after»). |
| Backlog (блокер спринта по AC-1, не по Slice 3) | Команда или расширение существующего вывода wallet/account с RPC-полем `marks`. |

---

## Participation / token estimate

```
agent: pwm-review
result: PASS-WITH-NITS
artifacts: docs/reviews/v2-4-slice3-final-review-20260506.md
token_usage: { "source": "estimate", "input": null, "output": null, "total": 16500, "confidence": "low" }
```

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/v2-4-slice3-final-review-20260506.md'
git add 'tasks/20260506-v2-sprint4-burn-clients.json'
git commit -m 'docs(v2-4-s3): pwm-review final sprint report'
```

**Verdict line (quote):** PASS-WITH-NITS — AC-1 FAIL; AC-2/5/6/7 PARTIAL; AC-3/4 PASS; Slice 3 docs + комментарий с неточностью про InsufficientMarks vs pwmd JSON.
