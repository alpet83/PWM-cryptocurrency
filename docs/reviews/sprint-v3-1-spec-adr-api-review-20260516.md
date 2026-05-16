# Review: MVP V3 Sprint 1 — spec/ADR/API foundation (docs-only)

**Дата:** 2026-05-16  
**Роль:** `pwm-review` (независимое ревью после `pwm-coding`, `pwm-testing`)  
**Тикет:** `tasks/20260516-v3-sprint1-spec-adr-api.json`  
**План:** `docs/plans/mvp_v3.md` Sprint V3-1  

---

## 1. Scope recap

Проверены артефакты Sprint V3-1 по тикету и плану:

- `docs/api-v1.md` — skeleton freeze границы public `/v1/*`, классы endpoint’ов, минимальный smoke (curl/PowerShell).
- `docs/adr/README.md` — индекс ADR 0001–0004 и явная граница V3 vs отложенный runtime V4/V5/V7.
- `docs/adr/0002-ipv4-claiming-design.md`, `0003-offchain-scaling-model.md`, `0004-cleanup-chain-bootstrap-snapshot-and-anchoring.md` — статус, контекст, решение, deferred boundaries, ссылки.
- Согласованность с `docs/plans/mvp_v3.md`, выборочно с `docs/CONCEPT_ROADMAP.md` (секция MVP V3, cleanup-chain / snapshot terminology).

Прод-код (`crates/**`) в объёме этого слайса не рецензировался (docs-only slice).

---

## 2. Requirements fit (acceptance Sprint V3-1)

| Критерий (план/тикет) | Оценка |
|----------------------|--------|
| `docs/api-v1.md` существует; явно разделены public stable `/v1/*`, operator и dev-only | **Да.** Разделы 2–3 и таблица 3.1 vs 3.2–3.3. |
| В `docs/adr/` есть индекс и три V3 ADR с понятным статусом | **Да.** README + Draft (V3 foundation) в каждом из 0002–0004. |
| ADR не обещают реализацию V4/V5/V7 в V3; зафиксированы границы и направление | **Да.** Секции «Deferred implementation boundaries» и формулировки «не часть V3» во всех трёх ADR; README повторяет границу. |
| `tasks/*.json` содержит delegations и ссылки на artifacts | **Да** до ревью; после ревью обновлены поля `pwm-review` и `artifacts.review_md`. |
| Разделение **Epoch Snapshot** vs **Bootstrap Snapshot** | **Да.** ADR 0004 п.1 жёстко разводит термины; `CONCEPT_ROADMAP.md` содержит согласованное описание (проверено по ключевым строкам). |

**Частичное / процессное:** в frontmatter `docs/plans/mvp_v3.md` пункт `v3-sprint-1-spec-adr-api` остаётся `pending` — это ожидаемый **owner/orchestrator closeout**, а не пробел в тексте ADR/API. На закрытие gate спринта по статусу тикета влияет решение владельца (обновить frontmatter и/или пометить тикет `done`).

---

## 3. Style and module shape

Документы краткие, структура ADR соответствует рекомендации в `docs/adr/README.md`. Язык основной — русский; для внешних ссылок на пути используется консистентный стиль.

**Механика разметки (закрыто в review leg):** под «## 3) Endpoint-классы» добавлена одна вводная строка в `docs/api-v1.md`, чтобы не было пустого блока перед «## 3.1».

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

---

## 4. Safety

Для docs-only слайса: примеры smoke направлены на `127.0.0.1` и не подразумевают передачу секретов в репозитории. Риски эксплуатации кода не затрагиваются.

---

## 5. Tests

Логика Rust не менялась. Предыдущая оценка `pwm-testing`: **PASS_WITH_NITS** (ссылочная целостность, согласованность scope); механические правки в плане уже внесены.

Остаются известные оговорки владельца/следующих спринтов (не блокеры V3-1 spec):

- точная форма ответов `/v1/*` и smoke PowerShell (`$accounts[0].id`) — финальная проверка против живого devnet уместна в V3-3/V3-4;
- normative wire-контракт API — заявлен как целевой freeze baseline с обязательной сверкой с `docs/pwmd.md` и `pwmd` до production/final closeout (`docs/api-v1.md` §3.1) — это снижает риск переутверждения exact JSON на этом этапе.

---

## 6. Verdict

**PASS_WITH_NITS.**

**Blocking findings:** none.

**Findings (по серьёзности):**

1. **Процесс / owner decision:** закрытие Sprint V3-1 в метаданных плана (`mvp_v3.md` frontmatter todo `v3-sprint-1-spec-adr-api` → `done`) и перевод тикета в `done` — по усмотрению владельца после принятия этого отчёта; на текст ADR/API не влияет.

**Проверки «foundation-only» и «нет обещания V4/V5/V7 runtime в V3»:** выполнены; формулировки «primary path для V5» в ADR 0003 относятся к **направлению**, не к реализации в V3, что согласуется с deferred-секциями.

**Граница API freeze:** документ явно ограничивает стабильность только перечисленным public `/v1/*` и требует сверки с кодом/docs оператора перед финализацией — overclaiming полного wire-контракта без доказательств нейтрализовано.

---

## 7. Participation / token estimate

```json
{
  "agent": "pwm-review",
  "result": "PASS_WITH_NITS",
  "artifacts": [
    "docs/reviews/sprint-v3-1-spec-adr-api-review-20260516.md",
    "docs/api-v1.md"
  ],
  "token_usage": {
    "source": "estimate",
    "input": 28000,
    "output": 4200,
    "total": 32200,
    "confidence": "medium"
  }
}
```

---

## 8. Sprint-final glossary traceability

Не применимо: это ревью Sprint V3-1 (не финальное закрытие всего MVP V3). **`docs/GLOSSARY.md` не изменялся.**
