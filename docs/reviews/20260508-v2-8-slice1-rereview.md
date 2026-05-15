# Повторное ревью: V2-8 документы после `3d1422c` (Slice 1 traceability + validator competition в RFC 15)

**Дата:** 2026-05-08  
**Агент:** pwm-review  
**Базовый коммит правок:** `3d1422c` (`docs(rfc): clarify validator competition model and close slice1 traceability nits`)  
**Тикеты:** `tasks/20260508-v2-sprint8-slice1-wire-schema.json`, `tasks/20260508-validator-competition-review.json`  
**Входы:** `docs/rfc/15-same-shard-sync-v1.md`, `docs/plans/mvp_v2.md`, предыдущие отчёты `docs/reviews/20260508-v2-8-slice1-review.md`, `docs/reviews/20260508-validator-competition-in-shard.md`

---

## 1. Scope recap

Пакет `3d1422c` закрывает документальные ниты первого ревью Slice 1: в RFC 15 явно зафиксированы **Required now vs Deferred** для §6 (приёмка Slice 1 больше не конфликтует с литеральной формулировкой «все сообщения раздела 6»); в §5.1 уточнено, что **`sync_profile` — единственный нормативный источник** для `full_v1`, а голый токен в `sync_capabilities` не должен включать полный v1; в §7 добавлен блок **proposer model + конкурентные кандидаты + fork-choice + граница `finalized_height` + deferred**; в `mvp_v2.md` (Sprint V2-8 Slice 1) добавлена трассируемость на RFC 15 §11.

Продукционный Rust в этом коммите не менялся; оценка — **спецификация и трейсабилити слайса**, плюс согласованность с архитектурным вопросом «конкуренция валидаторов» на уровне текста RFC.

---

## 2. Requirements fit

**Slice 1 traceability (цели 2 и 3):** выполнено. Подмножество wire для Slice 1 привязано к §6.2 (`Headers*`/`Blocks*`/`SyncNack` + общий envelope), mempool §6.1, `TipAnnounce` и catch-up §6.3 разнесены по слайсам 2–4; противоречие с первым ревью (`20260508-v2-8-slice1-review.md`, §2) снято.

**`sync_capabilities` vs `sync_profile`:** выполнено: добавлена нормативная строка, что профиль на wire обязателен для решения о `full_v1`, список возможностей сам по себе недостаточен.

**Секция RFC про «validator competition» / fork-choice относительно прежних рекомендаций (`20260508-validator-competition-in-shard.md`):**

- Зафиксированы: одна ожидаемая роль пропозера на высоту в рамках текущей PoA-модели, запрет предпочитать ветку с «не тем» пропозером каноническому прогрессу на той же высоте, порядок действий при нескольких кандидатах (сначала валидность, затем §7.3), детерминированный кортеж fork-choice, явный defer для multi-proposer / governance финализации / весов.
- Частично открыто по сравнению с «полной продакшн-готовностью всх спецификаций контура»: **как именно заполняется и согласуется `finalized_height` в `TipAnnounce` в PoA-devnet** остаётся на уровне «граница синхронизации, наследуется от PoA», без операторского численного правила в этом же коммите; **точная формула `height → producer_idx` по-прежнему живёт в WHITE/коде** — RFC даёт семантику ожидаемого пропозера, но не дублирует процентную арифметику индекса. Для «замороженного v1» этого достаточно, если Slice 3 зафиксирует приёмочное правило ingress/apply и политику `finalized_height` в runbook или узком дополнении (как уже рекомендовалось в архитектурном обзоре §4).

---

## 3. Style and module shape

Изменения только в Markdown/JSON задач; к продакшен-коду и именованию идентификаторов Rust коммит не относится.

---

## 4. Safety

На уровне спецификации усилены детерминизм ветвления и запрет скрытых эвристик; уточнение `sync_profile` снижает риск **ложного** включения `full_v1` по одному только флагу в списке возможностей.

Оставшийся **операционный** риск — расхождение интерпретаций `finalized_height` между реализациями до явного операторского правила — не устранён текстом `3d1422c` полностью, но снят как блокер для **документального** закрытия нити «конкуренция на уровне RFC §7» vs «отсутствие любого пропозер-контракта».

---

## 5. Tests

Не применимо к коммиту `3d1422c` (нет кода). Регрессия по ранее принятому Slice 1 по-прежнему опирается на `docs/reviews/20260508-v2-8-slice1-testing.md` и коммит реализации `eb5fc5a`.

---

## 6. Verdict

**Approve with nits** для целей оркестратора:

- **Slice 1 тикет:** можно переводить в **`done`** — документальные блокеры первого ревью закрыты; оставшиеся ниты первого код-ревью (`SyncProfileAnnounce` vs handshake-only, расширение тестов shard gate — см. `20260508-v2-8-slice1-review.md`) **не откатываются** этим коммитом и остаются backlog для следующих слайсов, но **не блокируют** закрытие Slice 1 по согласованной приёмке RFC §11.
- **Validator-competition / RFC §7:** нормативный минимум для «узкого патча» выполнен; **nit:** зафиксировать в следующей итерации операторское/PoA-правило для `finalized_height` в tip-трафике и выровнять ссылками WHITE↔RFC точную формулу индекса пропозера, если в спорных местах WHITE и ядро расходятся.

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts:
  review_md: docs/reviews/20260508-v2-8-slice1-rereview.md
commits_reviewed:
  - 3d1422c
token_usage:
  source: estimate
  input: null
  output: null
  total: 6500
  confidence: medium
```

**Однострочный вердикт для цитирования:** `PASS_WITH_NITS — RFC15 §5.1/§7/§11 и план V2-8 согласованы; Slice1 done; nit: operational finalized_height + cross-ref prod_idx formula for PoA.`

---

_End of rereview._
