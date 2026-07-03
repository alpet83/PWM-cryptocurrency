# Финальный re-gate: V2-8 Slice 5 — после хотфикса `23a183f`

**Тикеты:** `tasks/20260508-v2-sprint8-slice5-observability-chaos-docs.json`, `tasks/20260508-v2-slice5-hotfix-profile-drop-test.json`  
**HEAD (проверено):** `23a183fafc761822aae8feed72e3673c88de7456` — хотфикс **`test(pwmd): align tx_batch_profile_drop with profile_mismatch reason`** является вершиной истории.  
**Прослеживаемость FAIL:** `docs/reviews/20260508-v2-8-slice5-rereview.md`, `docs/reviews/20260508-v2-slice5-hotfix-testing.md` (база **`53fab53`**).  
**Вход pwm-testing rerun:** `docs/reviews/20260508-v2-slice5-hotfix-testing-rerun.md` (HEAD **`23a183f`**, зелёные команды — согласуется с независимой проверкой ниже).

## 1. Scope recap

Повторное закрытие гейта Slice 5 после интеграции pwm-coding: снятие блокера **`peer_session::tests::tx_batch_profile_drop`** (согласование ожидаемого reason-кода **`profile_mismatch`** с нормализацией в `route_sync_stub` для legacy `full_v1=false`), подтверждение отсутствия регрессий в таргетных транспортных тестах. Содержательный scope Slice 5 (метрики, chaos-ориентированные проверки, runbook, RFC 15) заявлен закрытым в prior pwm-coding/pwm-testing; данный артефакт фиксирует **финальный** результат re-gate на зелёном дереве.

## 2. Requirements fit

**Блокер снят:** коммит **`23a183f`** меняет ожидание в `tx_batch_profile_drop` на **`profile_mismatch`**, что соответствует заявленной цели hotfix-тикета (без изменения продуктового поведения, только выравнивание теста). Требования первичного ревью и re-gate по устранению расхождения `shard_mismatch` vs фактическая ветка — **выполнены** на текущем HEAD.

## 3. Style and module shape

Дифф хотфикса — точечная правка assert в тесте + обновление метаданных тикета в том же коммите; отдельного расширенного ревью стиля прод-модулей в рамках этого финального гейта не требовалось. Замечание из первого ревью (nit: дубликат `add_bucket` vs `add_str_u64_bucket` в metrics) **без изменения статуса** — вне scope hotfix.

## 4. Safety

Новых рисков безопасности от правки ожидания в юнит-тесте нет; поведение сетевого/синх-пути не менялось в этом коммите.

## 5. Tests

Локально (workspace `P:\opt\docker\pwm-protocol`, HEAD **`23a183f`**):

| Команда | Результат |
|---------|-----------|
| `cargo test -p pwmd peer_session::tests` | **15 passed; 0 failed** |
| `cargo test -p pwmd prod_` | **4 passed; 0 failed** |

**`tx_batch_profile_drop`** входит в набор и завершается **ok**. Регрессий относительно прежнего профиля **14 PASS / 1 FAIL** не наблюдается.

## 6. Verdict

**approve** — финальный re-gate **пройден**; хотфикс **`23a183f`** на HEAD; блокер **`tx_batch_profile_drop`** устранён; таргетные `peer_session::tests` и `prod_*` зелёные.

**Код для оркестратора:** **PASS**.

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/20260508-v2-8-slice5-rereview-final.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 9000
  confidence: medium
```

---

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260508-v2-8-slice5-rereview-final.md'
git add 'tasks/20260508-v2-sprint8-slice5-observability-chaos-docs.json'
git add 'tasks/20260508-v2-slice5-hotfix-profile-drop-test.json'
git commit -m 'docs(v2-8-s5): final re-gate PASS — slice5 tickets done'
```
