# Sprint V2-1 — Slice C: independent review gate (policy matrix freeze)

**Дата:** 2026-05-05  
**Ревьюер:** `pwm-review`  
**Входы:** [sprint-v2-1-slice-c-policy-matrix-freeze.md](./sprint-v2-1-slice-c-policy-matrix-freeze.md), [sprint-v2-1-slice-c-test-report.md](./sprint-v2-1-slice-c-test-report.md), `tasks/20260505-v2-s1-slice-c-policy-matrix.json`  
**Ограничение:** docs-only; `crates/*` не рассматривались как предмет изменений.

---

## 1) Scope recap

Slice C заявляет RFC-freeze **policy-уровня** для ClaimTx в фазах `mempool` / `preflight` / `apply`: единый порядок проверок (C-POL-1..2), правило округления N-MAT-5 через C-MAT-1 (floor в пользу сети), формализация anchor incompatibility (C-ANC-A..D) как закрытие M1 из Slice B review, уточнение P-REO-04 через C-REO-1 (canonical replay only), таблица «решение → класс ошибки» без финализации wire API. В `mvp_checklist` тикета указан блок «§1 Спецификация и решения». Заявленная связь с Slice A/B (схема и state) сохраняется: матрица опирается на уже зафиксированные семантики snapshot vs canonical apply.

---

## 2) Requirements fit

**Соответствие цели слайса:** высокое по перечню обязательств из тикета и decision log §7 freeze: перечисленные пункты 1–6 выполнены в тексте freeze.

**Пробелы / частичное покрытие:**

- Тикет формулирует scope как «claim **and burn**»; основной нормативный текст матрицы и таблица ошибок в freeze сфокусированы на **ClaimTx**. Явной симметричной policy-матрицы для **BurnMarkTx** в документе нет. Это согласуется с заголовком §2 freeze («Фазовая policy matrix (ClaimTx)»), но **расхождено с формулировкой тикета** — для полного закрытия burn-части нужен отдельный абзац/таблица или явное «out of scope для C» в freeze.
- Артефакт **pwm-testing** [sprint-v2-1-slice-c-test-report.md](./sprint-v2-1-slice-c-test-report.md) даёт **PARTIAL**: согласуется с ревью по ClaimTx/C-POL/C-MAT/C-ANC/C-REO и по пробелу **BurnMarkTx**; указывает на редакционное обновление slice-1 test-matrix для P-MAT-06 / P-REO-04 и на отсутствие исполняемых тестов (docs-only).

---

## 3) Style and module shape

Продакшн-Rust в scope отсутствует. Документ сохраняет стиль предикатной фиксации (идентификаторы C-*), последовательность «норма → матрица → семантика» согласуется с Slice B. Рекомендация: при добавлении burn-политики использовать тот же шаблон таблиц, чтобы D мог унифицировать коды ошибок.

---

## 4) Safety

**Позитивно:** явное разграничение консенсус-критичных MUST и операционного SHOULD для fee в mempool; приоритет `apply` при гонках; C-REO-1 устраняет «призрачные» ограничения после reorg; floor-округление убирает непереносимый sub-quantum кредит как класс недетерминизма.

**Риски спецификации:** предикат C-ANC-C зависит от того, **когда** claim «заявлен как использующий непрерывность» — это должно однозначно вытекать из Slice A/B (иначе два клиента могут по-разному интерпретировать условие). Не блокер RFC, но точка внимания для implementers.

---

## 5) Tests

Исполняемые тесты в `crates/*` слайсом не добавлялись — ожидаемо для docs-only. Отчёт pwm-testing фиксирует **testable** нормы по блокам freeze и **PARTIAL** вердикт (см. входы): полный PASS привязан к явному закрытию BurnMarkTx-политики и к Slice D для wire `E_*`; рекомендована редакция [sprint-v2-1-slice-1-test-matrix.md](./sprint-v2-1-slice-1-test-matrix.md) по строкам P-MAT-06 / P-REO-04 после принятия C.

---

## 6) Findings by severity

### Low

- **L1.** Несоответствие формулировки тикета («claim and burn») и фокуса freeze (практически только ClaimTx): добавить явный scope для BurnMarkTx или уточнить тикет.
- **L2.** В C-POL-2 для free-day в mempool формулировка «SHOULD … MUST on final admit» полезна, но без определения «final admit» в глоссарии слайса операторы ноды могут по-разному трактовать границу SHOULD/MUST — микро-уточнение в Slice D или короткий footnote.

### Medium

- **M1.** C-ANC-C опирается на «continuity» интервала; жёсткая трассируемость к B-STATE (сброс непрерывности при изменении B) в виде одной перекрёстной ссылки или номера нормы снизит риск разночтений между mempool и apply.

### High

- **Нет** для объёма docs-only и заявленного RFC-freeze.

---

## 7) Verdict

**Approve with nits** — документ пригоден как **policy matrix baseline** для Sprint V2-1; ключевые хвосты Slice B (M1, N-MAT-5, P-REO-04) закрыты на уровне норм; отсутствие тест-артефакта Slice C и неявный scope по burn — ниты, не блокеры для перехода к D при явном решении по burn.

---

## 8) Release recommendation (спринтовый gate)

**Разрешить переход к Slice D** при условии: (1) зафиксировать wire/API для классов `E_*` как запланировано в handoff §8 freeze; (2) явно развести или дополнить политику **BurnMarkTx**, если она остаётся в том же тикете; (3) синхронизировать slice-1 test-matrix с закрытием P-MAT-06 / P-REO-04 в C (редакция docs, вне `crates/*`). Промышленный **binary/network release** из docs-only слайса не заявлялся.

---

## 9) Participation / token estimate (`pwm-review`)

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/sprint-v2-1-slice-c-review.md
  - tasks/20260505-v2-s1-slice-c-policy-matrix.json
token_usage:
  source: estimate
  input: null
  output: null
  total: 5200
  confidence: low
```

_Оценка по объёму прочитанных freeze/ticket и объёму отчёта; провайдер токенов недоступен._

---

## 10) Git handoff для оркестратора

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/sprint-v2-1-slice-c-review.md'
git add 'tasks/20260505-v2-s1-slice-c-policy-matrix.json'
git commit -m 'docs(v2-1): Slice C policy matrix review gate and ticket traceability'
```

---

_End of review report._
