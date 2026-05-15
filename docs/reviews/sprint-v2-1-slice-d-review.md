# Sprint V2-1 — Slice D: independent review gate (API contract freeze)

**Дата:** 2026-05-05  
**Ревьюер:** `pwm-review`  
**Входы:** [sprint-v2-1-slice-d-api-contract-freeze.md](./sprint-v2-1-slice-d-api-contract-freeze.md), [sprint-v2-1-slice-d-test-report.md](./sprint-v2-1-slice-d-test-report.md), `tasks/20260505-v2-s1-slice-d-api-errors.json`  
**Ограничение:** docs-only; `crates/*` не менялись и не ревьюились как код.

---

## 1) Scope recap

Slice D заявляет RFC-freeze **wire/API слоя отказов**: стабильное соответствие «семантика → `error.code` (`E_*`) → `response_class`», четыре класса ответа (`VALIDATION_ERROR`, `POLICY_REJECT`, `STATE_CONFLICT`, `TEMPORARY_UNAVAILABLE`), трассируемость к decision classes Slice C, контракт симметрии `preflight` / `mempool` / `apply` (D-CON-1..3), минимальная норма JSON reject-shape с обязательными полями и burn-related строками в таблице §3. Тикет `mvp_checklist`: «§1 Спецификация и решения». Бриф тикета дополнительно перечисляет **claim/burn/purpose/free-day**; в freeze явно зафиксированы `tx_kind` только **`claim|burn`**.

---

## 2) Requirements fit

**Соответствие заявленной цели слайса (API mapping + симметрия фаз + минимальный JSON):** высокое. Таблица §3 согласована с классами §2; D-CON блоки закрывают pre-state/snapshot drift и burn parity; decision log §6 отражает выполненные обязательства freeze-документа.

**Пробелы / частичное покрытие:**

- **Purpose-транзакции:** в брифе тикета участвуют «purpose», в нормативном JSON §5 — только `claim|burn`. Если purpose — отдельный вид входа на API, для закрытия тикета без двусмысленности нужна явная фиксация в документе (расширение `tx_kind`, отдельная сцена ответа или явное «purpose сводится к claim с …»). Иначе **несводимость** брифа к freeze.
- **Free-day:** семантика в основном покрыта строкой `E_FREE_CLAIM_DAILY_LIMIT` и классом `POLICY_REJECT`; отдельного finding не требует, если сеть трактует free-day как режим claim.
- **Артефакт pwm-testing для Slice D:** [sprint-v2-1-slice-d-test-report.md](./sprint-v2-1-slice-d-test-report.md) даёт **PASS** на **тестопригодность нормы** и явно фиксирует отсутствие `cargo`/исполняемых тестов в слайсе (docs-only). Замечания отчёта по `trace_id` (формат не зафиксирован) и success-shape вне D **согласуются** с finding L1 и с handoff freeze §7.

---

## 3) Style and module shape

Продакшн-Rust в слайсе отсутствует. Стиль документа выдержан: предикаты с идентификаторами (`D-CON-*`), таблица + примеры JSON, явные ссылки на Slice C. Рекомендация для следующей редакции: одна фраза о **формате** `trace_id` (например, обязательная непустая строка; допустимые профили UUID vs opaque — на усмотрение реализации), чтобы клиенты не зашились на примерах с разной формой в §5.

---

## 4) Safety

На уровне спецификации полезно: отделение `TEMPORARY_UNAVAILABLE` для `E_ANCHOR_STATE_UNAVAILABLE`, явное требование не подменять такие отказы произвольным generic error при drift (D-CON-2), разведение validation vs policy vs state conflict, burn-кейсы без сведения к «внутренней» ошибке. Новых доверенных границ или крипто-примитивов документ не вводит. Риски внедрения (несовпадение mempool/apply при операционных послаблениях) явно ограничены оговоркой про consensus-critical ветки (D-CON-1).

---

## 5) Tests

Исполняемые тесты `crates/*` слайсом не добавлялись — согласуется с docs-only. Отчёт `pwm-testing` перечисляет test-gaps (golden JSON после реализации, симметрия D-CON, drift C-ANC-D, осознанно слабая фиксация `message`) и вердикт **PASS** на уровне RFC; это приемлемо как **testing-gate для docs-only** при условии последующей автоматизации по таблице §3 после кода.

---

## 6) Findings by severity

### Low

- **L1.** Примеры `trace_id` чередуют UUID-стиль и строки вида `trc-*` — для freeze достаточно одной нормативной ремарки о допустимых форматах (см. §3 Style).
- **L2.** Условная формулировка для `E_BURN_POLICY_REJECT` («если policy включена») корректна, но при интеграции стоит явно связать с флагами Slice C, чтобы не было «мёртвого» кода в клиентах.

### Medium

- **M1.** Расхождение **брифа тикета** (presence **purpose**) и **нормативного `tx_kind`** в freeze (`claim|burn` только): до уточнения документа или тикета gate **полного** соответствия формулировке спринта остаётся двусмысленным.

### High

- **Нет** для объёма docs-only freeze-документа.

---

## 7) Verdict

**Approve with nits** — документ пригоден как **baseline API/error contract** для Slice D: таблица стабильных кодов, классы ответов, симметрия фаз и минимальный JSON контракт сформулированы ясно; burn-scope закрыт явными строками. Ниты: purpose vs `tx_kind`; уточнение профиля `trace_id` (см. test-report §2 п.6).

---

## 8) Release recommendation (спринтовый gate)

**Разрешить дальнейшую работу** (редакции docs / матрицу негативных кейсов / реализацию в `crates/*`) на базе текущего freeze с условием: (1) снять двусмысленность **purpose** относительно API shape; (2) после кода — закрыть перечисленные в test-report gaps (golden JSON, D-CON, drift); (3) при реализации — проверить однозначное восстановление `response_class` из `error.code` без дублирования смыслов в разных фазах.

---

## 9) Participation / token estimate (`pwm-review`)

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/sprint-v2-1-slice-d-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 3800
  confidence: low
```

**Обоснование `PASS`:** freeze-документ и testing-gate (docs-only) согласованы; остаются низко-/среднесрочные ниты (purpose vs `tx_kind`, профиль `trace_id`), не блокирующие RFC baseline.

---

## 10) Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/sprint-v2-1-slice-d-review.md'
git add 'tasks/20260505-v2-s1-slice-d-api-errors.json'
git commit -m 'docs(slice-d): pwm-review report and task traceability'
```
