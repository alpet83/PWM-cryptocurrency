# Review: V2-1 RFC publication pack (claims/burn)

**Date:** 2026-05-05  
**Reviewer role:** `pwm-review`  
**Scope:** docs-only publication gate — `docs/rfc/11`–`14`, `docs/rfc/README-v2-claims-pack.md`; traceability to Slice A–E freeze; no `crates/*`.

---

## 1. Scope recap

Тикет `20260505-v2-rfc-publication-pack` и MVP checklist **§1 Спецификация и решения**: формализовать спецификацию claim/burn до кодовых слайсов E-1/E-2/E-3. Пакет нормализует freeze из `docs/reviews/sprint-v2-1-slice-*-freeze.md` и handoff в структурированные RFC 0011–0014 с навигацией в README; план `docs/plans/mvp_v2.md` уже содержит ссылку на [README-v2-claims-pack.md](../rfc/README-v2-claims-pack.md).

---

## 2. Requirements fit

**Соответствует заявленной цели:** цепочка зависимостей tx (0011) → state (0012) → policy (0013) → API wire (0014) воспроизводима; out-of-scope секции явно отсылают на соседние RFC; legacy `BurnMarkTx v1` и adapter path задокументированы.

**Пробелы / частичное покрытие:**

| ID | Severity | Finding |
|----|----------|---------|
| F-1 | **Low** | В RFC 0011 для поля используется `claim_units`, а в списке кодов есть `CLAIM_DELTA_INVALID` без пояснения, что «delta» здесь относится к той же величине. Риск путаницы при реализации и в тест-планах. |
| F-2 | **Low** | Два параллельных пространства имён ошибок: стабильные tx/preflight коды в RFC 0011 (без префикса `E_`) и нормализованные policy/API классы `E_*` в RFC 0013–0014. Семантика согласована частично по смыслу (например, лимит free-claim), но **нет одной нормативной таблицы соответствия** «tx-код ↔ policy class ↔ API `error.code`». |
| F-3 | **Info** | Документ `docs/reviews/v2-rfc-publication-test-report-20260505.md` присутствует (pwm-testing PASS, docs-only gate); выводы по двум слоям ошибок и маппингу совпадают с F-2 — полезно держать review и test report согласованными при следующем уточнении RFC. |

Существенных противоречий между RFC 0011–0014 по anchor, maturity base, phase order и reorg baseline не выявлено при чтении.

---

## 3. Style and module shape

Документация: структура «Abstract / Motivation / Specification / Compatibility / Out-of-Scope / References» единообразна; ссылки на freeze-артефакты проверены — целевые файлы в `docs/reviews/` существуют (включая slice E handoff). Смешение русского текста с английскими идентификаторами полей/кодов соответствует стилю соседних проектных документов. К продакшен-Rust и тестам слайс не относится.

---

## 4. Safety

Для docs-only: спецификация явно фиксирует детерминизм validation между mempool/preflight/apply, границы trust для `purpose`, ограничения длины/символов, reorg/replay и класс временной недоступности для anchor view — без выдуманных числовых лимитов сверх указанного в RFC. Остаточный риск — внедрение без явного mapping слоёв ошибок (см. F-2).

---

## 5. Tests

Код не затронут. Отчёт pwm-testing (`docs/reviews/v2-rfc-publication-test-report-20260505.md`) зафиксирован PASS и дополняет этот обзор замечаниями по тестируемости (маппинг слоёв ошибок для будущих golden vectors).

---

## 6. Verdict

**Approve with nits** — пакет пригоден к публикации как формальная основа для E-1/E-2/E-3; ниты носят уточняющий характер (именование `CLAIM_DELTA_*`, таблица mapping ошибок, опциональный тест-отчёт).

**Recommendation:** в следующей итерации docs (не обязательно до merge gate) добавить короткую нормативную таблицу соответствия или явный подпункт «два слоя кодов» с правилами проекции; переименовать или задокументировать `CLAIM_DELTA_INVALID` относительно `claim_units`.

---

## 7. Participation / token estimate

```json
{
  "agent": "pwm-review",
  "result": "PASS",
  "artifacts": [
    "docs/reviews/v2-rfc-publication-review-20260505.md"
  ],
  "token_usage": {
    "source": "estimate",
    "input": 14000,
    "output": 3200,
    "total": 17200,
    "confidence": "medium"
  }
}
```

Вердикт для оркестратора (одна строка): **Approve with nits — PASS gate для docs-only RFC pack.**
