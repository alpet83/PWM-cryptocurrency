# Sprint V2-1 — Slice E: independent review gate (implementation handoff, docs-only)

**Дата:** 2026-05-05  
**Ревьюер:** `pwm-review`  
**Входы:** [sprint-v2-1-slice-e-implementation-handoff.md](./sprint-v2-1-slice-e-implementation-handoff.md), [sprint-v2-1-slice-e-test-report.md](./sprint-v2-1-slice-e-test-report.md), `tasks/20260505-v2-s1-slice-e-implementation-handoff.json`  
**Ограничение:** docs-only; `crates/*` не ревьюились и не менялись.

---

## 1) Scope recap

Slice E заявляет **implementation handoff** после RFC freeze слайсов A–D: единый **lock-пакет** нормативных решений (§1), **file-level implementation map** для первой код-волны (`pwm-core` / `pwmd` / клиенты), **фазированный rollout** E-1 → E-2 → E-3 с гейтами, блок **рисков миграции** `marks_quota → marks`, **test plan handoff** для будущего `pwm-testing` (§5), **критерии закрытия V2-1** (§6) и заметка кодовой ноге (§7). Тикет: `mvp_checklist` — «§1 Спецификация и решения»; объём — только документы, без кода.

Проверка трассируемости: базовые ссылки в шапке handoff (`sprint-v2-1-rfc-inputs-20260505.md`, freeze A/B/C/D) **присутствуют** в `docs/reviews/`.

---

## 2) Requirements fit

**Соответствие цели слайса (свести A–D в исполнимый пакет для первой кодовой волны):** высокое. §1 компактно фиксирует решения, согласуемые с линией V2-1 (purpose, ClaimTx baseline, maturity/time/free-day, anchor predicates, stable errors, reorg, legacy v1 path). §2 даёт ориентиры по модулям без раздувания scope. §3–4 связывают порядок работ и риски миграции с принципом «один источник истины». §5–6 задают ожидаемые проверки и барьер закрытия всего V2-1 (включая код и тесты — это корректно как **целевой** критерий, а не как требование завершить код в рамках одного docs-тикета).

**Пробелы / частичное покрытие:**

- Имена полей состояния в §2.1 (`last_claim_anchor_ref`, …) — **ориентировочные**; расхождение с фактическими идентификаторами в коде возможно и ожидаемо до E-1, но командам стоит явно сверять с уже замороженными Slice B/C при первом PR.
- **pwm-testing:** [sprint-v2-1-slice-e-test-report.md](./sprint-v2-1-slice-e-test-report.md) даёт **PASS** на тестопригодность handoff и явно фиксирует отсутствие исполняемых `cargo` тестов в рамках docs-only тикета; оценки **E-1/E-2 READY**, **E-3 PARTIAL** и gap-лист §3 отчёта **согласуются** с формулировками handoff §3–§5.

---

## 3) Style and module shape

Продакшн-Rust в объёме слайса отсутствует — применимость правил **`AGENT_PROMPT_coding.md`** (длина идентификаторов, баннеры модулей) **не затрагивается**. Стиль документа выдержан: нумерованные lock-пункты, чёткие подзаголовки, табличная логика в духе предыдущих freeze.

---

## 4) Safety

На уровне спеки handoff полезно подчёркивает детерминизм **UTC day** от chain time, симметрию anchor-предикатов, replay/reorg, запрет «кредита» дробного остатка maturity, риски double-accounting и UTC-boundary — это снижает класс операционных и консенсусных футов. Новых криптопримитивов или доверенных границ документ не вводит.

---

## 5) Tests

Исполняемые тесты `crates/*` слайсом не добавлялись — согласуется с docs-only. §5 handoff задаёт **минимальный пакет** будущих проверок; отчёт `pwm-testing` подтверждает покрытие целей **на уровне спецификации** и перечисляет пробелы до исполняемого PASS V2-1 (§6 handoff), что приемлемо как **testing-gate для docs-only**.

---

## 6) Findings by severity

### Low

- **L1.** В тест-отчёте зафиксированы ожидаемые **PARTIAL** на E-3 (TUI/manual, legacy-детали) и мягкая норма по `trace_id`/`message` — не блокер handoff, но оркестратору стоит не потерять эти пункты при планировании E-3 и пост-кодовой автоматизации.

### Medium

- **Нет.**

### High / Blocker

- **Нет.**

---

## 7) Verdict

**Approve with nits** — handoff выполняет заявленную роль (lock + карта + фазы + риски + test handoff + done criteria); существенных противоречий с трассируемыми входами A–D, отчётом `pwm-testing` и планом `mvp_v2.md` по смыслу V2-1 не выявлено. Ниты: удержать в плане E-3/TUI и trace/message нормы, отмеченные в test-report.

---

## 8) Recommendation

1. Перед стартом E-1: короткая сверка имён полей §2.1 с `sprint-v2-1-slice-b-state-freeze.md` / policy freeze C.  
2. После первого код-слайса: завести полноценный отчёт `pwm-testing` по матрице §5 и повторный review-gate на код (отдельный тикет/слайс).  
3. При планировании E-3 заложить явное решение по TUI (hook vs manual) согласно gap test-report §3.

---

## 9) Participation / token estimate (machine-copyable)

```json
{
  "agent": "pwm-review",
  "result": "PASS",
  "artifacts": "docs/reviews/sprint-v2-1-slice-e-review.md",
  "token_usage": {
    "source": "estimate",
    "input": null,
    "output": null,
    "total": 4200,
    "confidence": "low"
  },
  "verdict_short": "approve_with_nits"
}
```

Примечание: `result: PASS` означает приемлемость **merge для docs-handoff** при учёте нитов; полный product-gate V2-1 по §6 handoff остаётся после реализации и тестов.
