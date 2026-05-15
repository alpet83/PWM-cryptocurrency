# Sprint V2-1 — Slice A: testing gate (docs-only)

**Дата:** 2026-05-05  
**Агент:** pwm-testing  
**Входы:** [sprint-v2-1-slice-a-tx-schema-freeze.md](./sprint-v2-1-slice-a-tx-schema-freeze.md), [sprint-v2-1-slice-1-test-matrix.md](./sprint-v2-1-slice-1-test-matrix.md), task `20260505-v2-s1-slice-a-tx-schema-purpose-claim`.

---

## Verdict: **PARTIAL**

Freeze-документ Slice A достаточно однозначен для unit/integration-тестов по **`purpose`** (лимит, trim, запреты, коды ошибок) и для базовой связки **`mode`/`fee`** у ClaimTx, а также для стратегии **legacy BurnMarkTx v1**. Полный testing-gate для сценариев **free/paid claim по календарю UTC** и дельты по **`anchor_ref`** закрывается только совместно со Slice B/C; кроме того, общая test-matrix всё ещё содержит формулировку, противоречащую выбранному в A лимиту.

---

## 1) Проверка тестопригодности freeze

### 1.1 Лимит и единица `purpose`

| Критерий | Оценка |
|----------|--------|
| Единственная единица измерения | **PASS.** §2.B: только UTF-8 байты после нормализации; `PURPOSE_MAX_BYTES = 80`; диапазон `1 <= utf8_len(purpose_norm) <= 80`. |
| Нормализация | **PASS.** Trim по Unicode White_Space на краях; без NFC/NFD/NFKC/NFKD — воспроизводимо в коде. |
| Граничные кейсы | **PASS.** Пустая строка после trim вне диапазона (нижняя граница 1) — явное отклонение; ровно 80 байт — верхняя граница; 81 — отклонение. |
| Коды ошибок | **PASS.** `INVALID_PURPOSE_LENGTH`, `INVALID_PURPOSE_CHARS`, `INVALID_PURPOSE_ENCODING` согласованы с правилами §2.D и UTF-8 входом. |

**Замечание:** в [sprint-v2-1-slice-1-test-matrix.md](./sprint-v2-1-slice-1-test-matrix.md) строка **P-PUR-01** всё ещё говорит про «байты UTF-8 **или** графемы — одна схема» и `PURPOSE_MAX_CHARS` — это **дрейф относительно Slice A** (только байты, константа в байтах). Для однозначного traceability матрицу следует выровнять под §2.B freeze (отдельным docs-тикетом, вне crates).

### 1.2 Сценарии free / paid claim (Slice A baseline)

| Критерий | Оценка |
|----------|--------|
| Синтаксис режима | **PASS.** `mode ∈ {"free","paid"}`; при `free` → `fee = 0`; при `paid` → `fee > 0` и «policy-порог» (число порога — вне A). |
| Согласованность путей | **PASS.** §3.B и §5.6: одинаковый вердикт в mempool, block apply, preflight — проверяемо контрактными тестами после реализации. |
| Дневной лимит free | **PARTIAL.** §3.C: одна бесплатная claim за UTC-day по chain time — **оракул дня** однозерен; полные сценарии (вторая free, платный fallback, полночь UTC) в матрице помечены Slice **B/C** (**P-FRE-01…06**) и требуют state/policy из B/C. |
| Остальная семантика claim | **GAP (ожидаемо).** `anchor_ref`, точная дельта `claim_units`, `CLAIM_DELTA_INVALID`, nonce относительно аккаунта — заявлены как минимальная валидация в A, но **семантика якоря и формулы** — Slice B (**P-MAT-***, **P-RST-*** в матрице). |

### 1.3 Legacy compatibility

| Критерий | Оценка |
|----------|--------|
| Два формата BurnMark | **PASS.** §6.1–6.2: legacy v1 без `purpose`; v2 с обязательным `purpose`. |
| Адаптер | **PASS.** Для legacy: `purpose_norm = ""`; tx не отвергается только из-за отсутствия поля — воспроизводимые фикстуры «до/после». |
| Новые клиенты | **PASS.** Preflight/клиенты по умолчанию v2 — проверка политики продукта, не консенсуса. |
| ClaimTx vs старые типы | **PASS.** Новый тип не ломает валидность прочих tx — изолированные тесты типов. |

**Риск тест-дизайна:** поведение «пустой `purpose`» у **legacy v1** (адаптер) vs **недопустимость пустого `purpose` после trim у v2** — нужно явно различать в тестах по `schema_version`/наличию поля, чтобы не смешать ветки.

---

## 2) Test-gaps (что не закрыто только документом A)

1. **Матрица vs A:** обновить P-PUR-01 (и легенду при необходимости) под **только UTF-8 байты** и имя константы **`PURPOSE_MAX_BYTES`**.  
2. **Численный порог комиссии** для `paid` — не зафиксирован в A; тесты **P-FRE-03/04** остаются без числового оракула до Slice C (или явной константы в B).  
3. **Семантика `anchor_ref` и `claim_units`** — Slice B; код **`CLAIM_DELTA_INVALID`** без полной спецификации даёт только shape-level тесты.  
4. **UTC midnight и reorg** для free-slot — намерения в матрице (**P-FRE-06**, **P-REO-***); нормативное закрытие в B/C.  
5. **API-трасса ошибок** (**P-PUR-07** и привязка кодов к JSON) — Slice D.

---

## 3) Рекомендации для следующего coding/testing цикла

- После выравнивания строки матрицы P-PUR-01: трассировать сценарии P-PUR-02…06 напрямую на §2 Slice A.  
- Для ClaimTx: в коде завести table-driven тесты на **`CLAIM_MODE_INVALID`**, **`CLAIM_FEE_MODE_CONFLICT`**, обязательные поля (**`CLAIM_REQUIRED_FIELD_MISSING`**) до подключения полной экономики.  
- Legacy: отдельный набор кейсов «deserialize v1 → normalized internal representation» vs «v2 missing purpose → schema/taxonomy error».

---

## Participation / token estimate (pwm-testing)

```yaml
agent: pwm-testing
result: PARTIAL
artifacts:
  - docs/reviews/sprint-v2-1-slice-a-test-report.md
commands:
  - docs review only (no cargo)
  - duration: n/a
  - pass_fail: n/a
  - hang_watchdog: no
cleanup:
  cleaned: "yes"
  killed: none
  artifacts: none
token_usage:
  source: estimate
  input: null
  output: null
  total: 4200
  confidence: low
```
