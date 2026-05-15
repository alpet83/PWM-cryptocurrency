# Sprint V2-1 — Slice A: independent review (tx schema RFC, docs-only)

**Дата:** 2026-05-05  
**Ревьюер:** `pwm-review` (независимый gate)  
**Входы:** `docs/reviews/sprint-v2-1-slice-a-tx-schema-freeze.md`; `docs/reviews/sprint-v2-1-slice-a-test-report.md` (pwm-testing, **PARTIAL**); `tasks/20260505-v2-s1-slice-a-tx-schema-purpose-claim.json`.

---

## 1) Scope recap

Тикет заявляет RFC-level freeze для tx-схемы без правок `crates/*`: BurnMarkTx v2 с обязательным `purpose` (лимит и нормализация), черновик ClaimTx (free/paid), outline error taxonomy, заметки по детерминированной сериализации/валидации и стратегия backward-compat для legacy BurnMarkTx v1. Документ `sprint-v2-1-slice-a-tx-schema-freeze.md` явно ограничивает scope и откладывает state-аккаунтинг и policy matrix на Slice B/C, API — на Slice D, что согласовано с заявкой в JSON.

Связь с MVP-checklist в тикете: «§1 Спецификация и решения» — покрывается как нормативный текст; исполняемая проверка в репозитории в этом слайсе не требовалась.

---

## 2) Requirements fit

**Соответствие заявленной цели:** да. Документ закрывает перечисленные пункты тикета: поле `purpose` с выбранной единицей (80 UTF-8 байт после trim), запрет C0/C1, ClaimTx с `mode` и правилом `fee`, набор стабильных кодов ошибок, требования к одному validation path (mempool/preflight/apply), legacy v1 с адаптером пустой метки.

**Пробелы / частичное покрытие (ожидаемо для Slice A, но нужно учесть в B):**

- Семантика `anchor_ref` и расчёт допустимости `claim_units` сознательно отложены — без этого нельзя считать ClaimTx полностью «замороженной» для реализации; для перехода к Slice B это нормальный долг.
- Политика порога комиссии для `mode = "paid"` названа качественно, без чисел — ожидаемо до Slice B/C.
- Тип представления `sig` указан как «bytes/string» — для канонической подписи в Slice B стоит сузить до одного wire-формата, иначе риск расхождения клиентов.

---

## 3) Style and module shape

Изменения — Markdown RFC; к продакшн-Rust и тестовым идентификаторам слайс не притрагивался. Структура документа логична (scope → схемы → ошибки → сериализация → совместимость → handoff). Незначимое замечание: в разделе ClaimTx можно в будущем (Slice B) явно перечислить канонический порядок полей для подписи, когда появится полная схема.

---

## 4) Safety

**Доверенные границы:** консенсус трактует `purpose` как непрозрачную метку; рекомендация не класть PII в открытом виде — уместно. Риск операторских утечек остаётся вне цепи; спецификация его не усугубляет.

**DoS / лимиты:** лимит 80 байт на `purpose` задаёт верхнюю границу полезной нагрузки на поле; глобальные лимиты тела tx не детализированы здесь (вне scope A).

**Согласованность правил:** требование единого вердикта валидатора в трёх контурах снижает класс расхождений; positive.

**Заметка по совместимости:** legacy v1 с «пустой» нормализованной меткой согласуется с тем, что v2 требует непустой `purpose` после нормализации — разделение по `schema_version`/`tx` форме должно быть строго проверяемо при имплементации, иначе возможна двусмысленность на границе парсера (это скорее предупреждение для кодирования, чем дефект RFC).

---

## 5) Tests

Артефакт `docs/reviews/sprint-v2-1-slice-a-test-report.md` (pwm-testing) подтверждает **тестопригодность** правил `purpose` (лимит, trim, C0/C1, коды), базовой связки `mode`/`fee` и legacy v1; вердикт **PARTIAL** согласован с ревью: полный gate по free-claim/UTC/reorg и семантике `anchor_ref` ждёт Slice B/C, численный порог комиссии для `paid` — вне A. **Регрессий в CI/коде нет** (docs-only). Отдельный риск трассируемости: в `sprint-v2-1-slice-1-test-matrix.md` строка **P-PUR-01** всё ещё допускает «графемы» и `PURPOSE_MAX_CHARS` — **дрейф относительно §2.B Slice A**; pwm-testing рекомендует выровнять матрицу отдельным docs-тикетом — ревью **поддерживает** это как условие чистого traceability до массовой имплементации.

---

## 6) Findings by severity

| Severity | Finding |
|----------|---------|
| **Medium** | `sig` указан допускающе (bytes/string) — перед кодом нужен один канонический wire-тип и правило кодирования в payload для подписи. |
| **Medium** | Правило «одна бесплатная claim за UTC-day» внесено в минимальную валидацию Slice A при том, что time/account state детализируются позже — риск преждевременной фиксации до прояснения chain-time и границ суток (уже намечено в handoff Slice C); зафиксировать в Slice B трактовку UTC-day относительно chain time. |
| **Low** | Дрейф test-matrix (P-PUR-01 vs байты-only в A) — не дефект freeze, но препятствие строгой трассировке «матрица ↔ RFC» до выравнивания (см. test-report). |
| **Low** | `account_id` как произвольная string — при имплементации сверить с существующими типами идентификаторов в кодовой базе (вне объёма данного ревью). |

Критических противоречий внутри freeze-документа и тикета не выявлено.

---

## 7) Verdict

**Approve with nits** — RFC Slice A пригоден как основа для Slice B: scope чёткий, ключевые решения по `purpose` и режимам claim зафиксированы, ошибки и сериализация описаны на уровне, достаточном для следующего нормативного шага. Ниты выше следует закрыть в Slice B (и частично C) до начала широкой имплементации клиентов/узла.

---

## 8) Release / transition recommendation

**Переход к Slice B:** **разрешить**, при условии что Slice B явно разрешит семантику `anchor_ref`, связь `claim_units` со state и канонический формат подписи/payload; до этого не стоит считать ClaimTx готовой к «жёсткой» lock без последующих правок.

**Релиз артефактов:** документация может быть объединена в тот же traceability-коммит, что и обновление тикета (по процедуре оркестратора).

---

## 9) Participation / token estimate

```json
{
  "agent": "pwm-review",
  "result": "PARTIAL",
  "artifacts": "docs/reviews/sprint-v2-1-slice-a-review.md",
  "token_usage": {
    "source": "estimate",
    "input": null,
    "output": null,
    "total": 4200,
    "confidence": "low"
  }
}
```

`PARTIAL` отражает вердикт «approve with nits» и согласован с PARTIAL pwm-testing по ожидаемым пробелам до Slice B/C.

---

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/sprint-v2-1-slice-a-review.md'
git add 'tasks/20260505-v2-s1-slice-a-tx-schema-purpose-claim.json'
git commit -m 'docs(v2-1): Slice A pwm-review report and task traceability'
```
