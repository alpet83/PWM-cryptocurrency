# MVP V4 final closeout review (2026-05-17)

**Agent:** pwm-review (independent documentation/traceability gate).  
**Ticket:** `tasks/20260517-v4-sprint6-closeout.json`.  
**Scope:** closeout artifacts — `docs/plans/mvp_v4.md`, `docs/reviews/20260517-v4-integrated-smoke.md`, `docs/MVP-checklist.md`, `docs/CONCEPT_ROADMAP.md`, `docs/GLOSSARY.md`, `CHANGELOG.md`; sprint reviews V4-1..V4-5 cited by plan/checklist (no full re-read of code slices).

**Addendum — re-review after nit closure (same day):** оркестратор синхронизировал верхний вердикт smoke (`Current verdict: PASS` + история PARTIAL), перевёл **`v4-sprint-6-closeout`** во фронтматтере плана в **`completed`**, добавил **`ActivationMode`** в алфавитный указатель глоссария. Повторная верификация: **`PASS`** (см. §6).

---

## 1. Scope recap

Заявленная цель V4-6: интегрированный devnet/smoke gate после политикового релиза, обновление чеклиста/дорожной карты/глоссария/changelog, финальный pwm-review при явном разделении backlog V5+.

Тикет фиксирует конвейер: первый прогон smoke **PARTIAL** (два падения `pwmd --lib` по JSON `Null`), правка контракта JSON в **pwm-coding** (`crates/pwmd/src/api/types.rs`, `crates/pwmd/src/transport/metrics.rs`), повторный smoke **PASS**, оркестраторская правка документов **PASS**.

---

## 2. Requirements fit

**План `mvp_v4.md`:** спринты V4-1..V4-6 во фронтматтере **`completed`**; текст плана и roadmap/checklist/changelog согласованы.

**Критерии V4-6 в плане:**

| Критерий | Оценка |
|----------|--------|
| Все критерии V4 из roadmap покрыты или явно отложены | **Да.** Импорт/emergency parity вынесены в backlog в плане V4-4/V4 scope и повторены в глоссарии (`Emergency routing` → ограничение V4). |
| Integrated smoke после фикса pwmd зафиксирован честно | **Да.** Сверху — **Current verdict: PASS**, блок **Initial run history** сохраняет PARTIAL и таблицу с FAIL как архив; Addendum с финальной матрицей PASS сохранён; тикет в `notes` явно перечисляет внешний scope (full workspace / manual TUI / soak). |
| Policy без недетерминированных колбэков / внешних сервисов | **На уровне документов** — согласовано с формулировками roadmap §ограничение и плана V4 (pure evaluator). Код не перепроверялся в этом слайсе. |
| Bug bounty scope для policy/cosign обозрим | **Да.** В `docs/CONCEPT_ROADMAP.md` таблица приоритетов по версиям явно включает **V4: policy engine bypass, cosign bypass**. |

**Честные пробелы (не противоречат заявленному scope V4-6):** полный `cargo test --workspace`, ручной TUI smoke, долгий devnet soak — помечены как optional hardening в checklist и roadmap.

---

## 3. Style and module shape

Ревью **документов и тикетов**, не прод-кода Rust. Стиль артефактов согласован: traceability от тикета к smoke и к строкам MVP-checklist §0v4.

Ранее отмеченные **механические ниты закрыты:** актуальный вердикт smoke читается с первого экрана; YAML спринта 6 — **`completed`**; в **`docs/GLOSSARY.md`** в указателе латиницы есть строка **`A ActivationMode`**.

---

### Wire JSON / u128

**Scope этого closeout-слайса по коду:** по тикету затронуты **HTTP JSON ответы / снимки метрик** узла (`api/types`, `transport/metrics`), восстановление имён полей для ожиданий тестов `transport_peer` (генesis guard / dev-peers), а не описание изменений **peer-framed** JSON (`PeerWireMsg`, sync catch-up batches) в данном документе.

**u128 / serde_json:** данное ревью **не** включает построчный аудит сериализации в `types.rs`/`metrics.rs`. Общее правило репозитория: любые **`u128`** на JSON-границе (включая REST, если когда-либо появятся суммы/сырые целые) должны использовать явные serde-помощники; derive-only `u128` на любой сетевой или межузловой JSON-поверхности — высокий риск регрессии **`u128 is not supported`**. Для закрытия V4 как релиза политики **достаточно**, что текущий интегрированный gate после фикса зелёный; подтверждение отсутствия новых «голых» `u128` на затронутых JSON структурах остаётся на стороне уже принятого pwm-coding pwm-testing PASS.

**RFC:** нормативный wire-контракт V4 для `PolicyTx.fee` и родственных полей закреплялся в RFC-слайсе V4-1 (вне этого файла); данный closeout-документ новых RFC полей не вводит.

---

## 4. Safety

Документально зафиксированы ограничения emergency routing (same-shard `Transfer`; без parity для `Import`), необратимость финализации и требование cosign для emergency activation — снижает риск ложных операторских ожиданий.

Риск **продуктовой** безопасности в этом слайсе не переоценивался: правки JSON контракта в pwmd уже прошли отдельный coding/testing PASS по тикету.

---

## 5. Tests

Smoke-отчёт консистентен с checklist §0v4: перечисленные команды включают `pwmd --lib`, pwm-core, полный pwm-cli, policy filters, snapshot bench compile; финальный PASS после фикса задокументирован в Addendum.

Явно **не** входило в gate: full workspace tests, отдельные `pwmd` integration binaries (`--test`), manual TUI/soak — это отражено честно.

---

## 6. Verdict

**PASS** (повторная проверка после auto-close нитов).

Инцидентные ниты первого прохода устранены в документации/плане/глоссарии. **Не блокирует вердикт, опционально для оркестратора:** в корне **`tasks/20260517-v4-sprint6-closeout.json`** по-прежнему можно выставить `status: done` и заполнить `artifacts.review_md` на этот отчёт при окончательном закрытии тикета (метаданные конвейера, не содержание V4).

Отделение **V5/backlog** без изменений: явно в **CHANGELOG** и смежных документах.

---

## 7. Participation / token estimate

**Первый проход closeout:** см. исторические оценки в delegations тикета.

**Re-review (этот проход):**

```yaml
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260517-v4-final-closeout-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 6500
  confidence: medium
```

---

## Glossary

В ревью-проходах **`docs/GLOSSARY.md`** субагентом pwm-review текст не редактировался; оркестратор добавил индексную строку **`ActivationMode`**. Тематический блок **MVP V4** по-прежнему покрывает закрытие релиза.

---

**Однострочный вердикт для оркестратора:** `PASS` — механические ниты первого PASS_WITH_NITS закрыты; финальный gate V4-6 по документам согласован.

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260517-v4-final-closeout-review.md'
# git add 'tasks/20260517-v4-sprint6-closeout.json'
git commit -m 'docs(v4): final closeout pwm-review re-review PASS'
```
