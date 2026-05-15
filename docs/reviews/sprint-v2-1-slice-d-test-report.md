# Sprint V2-1 — Slice D: testing gate report (docs-only)

**Дата:** 2026-05-05  
**Агент:** `pwm-testing`  
**Входы:** [sprint-v2-1-slice-d-api-contract-freeze.md](./sprint-v2-1-slice-d-api-contract-freeze.md), [tasks/20260505-v2-s1-slice-d-api-errors.json](../tasks/20260505-v2-s1-slice-d-api-errors.json)

---

## Verdict: **PASS**

**Обоснование:** Slice D как **RFC freeze** закрывает декларированный в Slice C пробел по **wire/API**: зафиксированы четыре `response_class`, таблица **семантика → стабильный `error.code` (`E_*`) → класс ответа**, трассируемость к decision classes C (**C-POL-\***, **C-MAT-1**, **C-ANC-\***, **C-REO-1**) и явно добавлены **burn-related** строки (**`E_BURN_*`**) — это снимает нормативный scope-gap по burn на уровне контракта отказов. Контракты **D-CON-1..3** (симметрия `preflight` / `mempool` / `apply`, drift при смене canonical state, burn parity) формулируют проверяемые ожидания для будущих свойственных и интеграционных тестов. Минимальный **JSON reject shape** (§5) задаёт обязательные поля (`ok`, `phase`, `tx_kind`, `response_class`, `error.code|message|trace_id`) — достаточно для проектирования негативных golden/fixture кейсов после появления реализации в `crates/*`.

Ограничение слайса: исполняемые тесты **не запускались** и **не добавлялись** (**docs-only**, без правок `crates/*`); gate оценивает **тестопригодность и полноту нормы D**, а не CI.

---

## 1. Сопоставление с контрактом D (что становится testable)

| Блок freeze | Оценка | Комментарий |
|-------------|--------|-------------|
| **§2 Response classes** | Testable | Четыре значения; детерминированное восстановление класса из `error.code` — свойство для таблицы §3. |
| **§3 Stable mapping** | Testable | Каждая строка → ожидаемая пара (`code`, `response_class`); burn и claim на одной шкале. |
| **§4 D-CON-1..3** | Testable при наличии кода | Нужен общий snapshot/fixture и три фазы (или минимум пара `preflight`/`apply` + mempool где есть harness). |
| **§5 Minimal JSON reject** | Testable | Схемные/полевые asserts + примеры A–C как эталоны формы (не обязательно дословный `message`). |

---

## 2. Test-gaps (что ещё не автоматизировано и почему)

1. **`crates/*`:** по Slice D в этом тикете **нет** новых unit/integration тестов и **нет** прогона `cargo test` — намеренно **docs-only**.
2. **Golden JSON / serde:** после кодирования ответов `pwmd`/RPC нужны негативные кейсы «строго обязательные поля §5» по каждому значимому `phase` × `tx_kind` × строке §3 (или сокращённая эквивалентная матрица без дублей семантики).
3. **Симметрия D-CON-1:** потребуется общий pre-state и один и тот же вход для сравнения кодов между фазами; для mempool — учёт явных операционных послаблений (fee admission), не меняющих consensus-critical mapping (как в D).
4. **Drift D-CON-2:** сценарии «preflight прошёл / apply отказал из‑за смены anchor» должны проверять, что новый отказ всегда из таблицы §3 и что **C-ANC-D** даёт **`TEMPORARY_UNAVAILABLE`** / **`E_ANCHOR_STATE_UNAVAILABLE`**, а не произвольный generic.
5. **`error.message`:** человекочитаемая строка; автотесты разумно проверять **`code` + `response_class` + поля верхнего уровня**, а не фиксировать точный текст сообщения, если продукт не обещает freeze текста.
6. **`error.trace_id`:** норма задаёт наличие корреляционного идентификатора; **формат** (UUID vs произвольная строка) в RFC не зафиксирован — при желании жёсткой проверки в CI понадобится доп. продуктовое правило или сужение в следующем RFC.
7. **Успешные ответы (`ok: true`):** Slice D нормирует **reject**; позитивные контуры и форма success остаются вне этого freeze — отдельные тест-планы/спецификация при необходимости.

---

## 3. Рекомендации `pwm-testing` на следующий код-слайс

- Строить негативные API-кейсы **напрямую от** `error.code` + `response_class` + `phase` + `tx_kind`, как указано в §7 handoff freeze D.
- Для burn — отдельная минимальная матрица отказов по четырём `E_BURN_*`, зеркально к подходу для claim.

---

## Participation / token estimate (`pwm-testing`)

```yaml
agent: pwm-testing
result: PASS
artifacts:
  - docs/reviews/sprint-v2-1-slice-d-test-report.md
  - tasks/20260505-v2-s1-slice-d-api-errors.json
commands: []
cleanup: n/a (no spawned processes)
preflight_target_debug: n/a (no cargo build/test)
token_usage:
  source: estimate
  input: null
  output: null
  total: 3200
  confidence: low
```

_Оценка по объёму нормы D и проверке тестопригодности без провайдера токенов._
