# Sprint V2-1 — Slice C: testing gate report (docs-only)

**Дата:** 2026-05-05  
**Агент:** `pwm-testing`  
**Входы:** [sprint-v2-1-slice-c-policy-matrix-freeze.md](./sprint-v2-1-slice-c-policy-matrix-freeze.md), [sprint-v2-1-slice-1-test-matrix.md](./sprint-v2-1-slice-1-test-matrix.md)

---

## Verdict: **PARTIAL**

**Обоснование:** для **ClaimTx** freeze C даёт однозначный фазовый порядок проверок (`mempool` / `preflight` / `apply`), закрывает **N-MAT-5** правилом **C-MAT-1** (`floor` в пользу сети), формализует **C-ANC-A..D** и **C-REO-1** (replay canonical ветки), плюс таблицу **decision → класс `E_*`**. Этого достаточно для проектирования негативных/позитивных кейсов по строкам матрицы **P-MAT-\***, **P-RST-\***, **P-FRE-\***, **P-REO-\***, где первичный слайс помечен как C. При этом (1) в заголовке scope и тикете фигурирует **BurnMarkTx**, а в теле freeze детализирована по сути только **ClaimTx** — для burn нет той же фазовой матрицы и предикатов; (2) исполняемые тесты в `crates/*` по этому коммит-слайсу не добавлялись (**docs-only**); (3) стабильное **wire/API** для классов `E_*` остаётся в **Slice D**, поэтому end-to-end негативные проверки с фиксированным полем/trace ещё нельзя зашить без D. Полный **PASS** testing-gate возможен после явного дополнения нормы по BurnMarkTx (или сужения scope тикета) и завершения D для кодов отклонений.

---

## 1. Сопоставление с test matrix (intent coverage)

| Блок матрицы | Статус после C | Комментарий |
|--------------|----------------|-------------|
| **P-PUR-\*** | Без изменений C | Серийные/schema ветви остаются на A/D; **P-PUR-07** всё ещё D. |
| **P-MAT-01–05** | Testable при C+B | Совместимо с **C-POL-\***, зрелость и округление для **P-MAT-05/06**: **P-MAT-06** больше не «две семантики» — эталон **C-MAT-1**; строку матрицы slice-1 стоит обновить текстом («закрыто в C»), без правки кода в рамках C. |
| **P-RST-\*** | Testable при B+C | **C-ANC-C** задаёт контур «непрерывность интервала» относительно переходов `B(h)!=B(h-1)`; согласовать именование с **N-MAT-3** в B при написании тестов. |
| **P-FRE-\*** | Testable при B+C | **C-POL-2** (free-day: SHOULD/MUST по фазам) + **§6** `E_FREE_CLAIM_DAILY_LIMIT` для повторной free в том же `utc_day`; симметрию фаз — **P-FRE-05**. |
| **P-REO-01–04** | **P-REO-04 закрыт** в C | **C-REO-1** снимает placeholder по `last_free_claim_utc_day` и последнему успешному claim при canonical replay; **P-REO-01–03** усиливаются явным правилом «orphan без эффектов». |

---

## 2. Тестопригодность норм Slice C по блокам

| Блок freeze | Оценка | Комментарий |
|-------------|--------|--------------|
| **C-POL-1 / C-POL-2** | Testable | Единый порядок проверок и матрица MUST/SHOULD по фазам — основа табличных свойственных тестов «та же транзакция / тот же snapshot → тот же класс решения». |
| **C-MAT-1** | Testable | Детерминированное **`floor`**; негатив **E_CLAIM_OVER_MATURED** при `claim_units > matured_units_available_int`. |
| **C-ANC-1 / C-ANC-2** | Testable | Четыре предиката → ожидаемые классы **E_ANCHOR_\*** в §6; фазовая симметрия verdict при одинаковом snapshot. |
| **C-REO-1** | Testable | Replay-only state для free-marker и последнего claim; сценарии **P-REO-02** (нет следов orphan), **P-REO-03** (повторное включение после reorg без двойной материализации). |
| **§6 Decision → E_\*** | Testable семантика | Достаточно для **названий классов** в тест-планах; привязка к HTTP/proto полям — после D. |

---

## 3. Test-gaps (что всё ещё не автоматизировано и почему)

1. **`crates/*`:** ни unit-, ни integration-тестов по Slice C не запускались и не добавлялись — слайс **docs-only**.
2. **BurnMarkTx:** отсутствует зеркальная к ClaimTx фазовая таблица и предикаты; риск расхождения реализации с заголовком scope тикета — **PWM-testing** трактует как нормативный пробел до доп. RFC или уточнения тикета.
3. **Slice D:** без wire/API нельзя зафиксировать в CI стабильные ожидания для **P-PUR-07** и всех отрицательных веток, где нужен точный ответ клиента (имя кода, поле, trace).
4. **Мемпул SHOULD (fee threshold):** локальная политика допускает мягкий отбор в mempool — интеграционные тесты должны явно фиксировать «consensus-critical MUST» vs операционный SHOULD, чтобы не было ложных падений в одноранговых конфигурациях.
5. **Синхронизация документов:** в [sprint-v2-1-slice-1-test-matrix.md](./sprint-v2-1-slice-1-test-matrix.md) строки **P-MAT-06** и **P-REO-04** всё ещё помечены как placeholder от B; после принятия C рекомендуется редакционная правка матрицы (вне объёма настоящего отчёта по `crates/*`).

---

## Participation / token estimate (`pwm-testing`)

```yaml
agent: pwm-testing
result: PARTIAL
artifacts:
  - docs/reviews/sprint-v2-1-slice-c-test-report.md
  - tasks/20260505-v2-s1-slice-c-policy-matrix.json
token_usage:
  source: estimate
  input: null
  output: null
  total: 3600
  confidence: low
```

_Оценка по объёму норм C и сопоставлению с матрицей; без провайдера токенов._
