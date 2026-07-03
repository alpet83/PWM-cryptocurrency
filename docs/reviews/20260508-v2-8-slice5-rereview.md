# Re-review: V2-8 Slice 5 — observability, chaos validation, operator docs (post-hotfix gate)

**Tickets:** `tasks/20260508-v2-sprint8-slice5-observability-chaos-docs.json`, `tasks/20260508-v2-slice5-hotfix-profile-drop-test.json`  
**Baseline coding commit (slice):** `9029fb0`  
**Prior review:** `docs/reviews/20260508-v2-8-slice5-review.md` (**FAIL**: stale `tx_batch_profile_drop` expectation)  
**Inputs checked:** prior FAIL report; ticket JSON; `docs/reviews/20260508-v2-8-slice5-testing.md`; **`docs/reviews/20260508-v2-slice5-hotfix-testing.md`** (pwm-testing: на `HEAD` **`53fab53`** хотфикс в Rust **не подтверждён**, те же **FAIL** команды — согласуется с независимой проверкой ниже).

## 1. Scope recap

Повторное закрытие гейта после заявленного хотфикса: подтвердить снятие блокера **`peer_session::tests::tx_batch_profile_drop`** (ожидание reason-кода после нормализации в `route_sync_stub`: ветка `!full_v1 || !same_shard` → **`profile_mismatch`**, не `shard_mismatch`), убедиться в отсутствии новых регрессий в модуле `peer_session::tests`, при успехе — обновить тикеты.

Фактическое состояние дерева: последнее изменение **`crates/pwmd/src/transport/peer_session/mod.rs`**, затрагивающее этот тест, по истории git — коммит **`9029fb0`**; отдельного коммита pwm-coding с правкой ожидания теста **не видно**.

## 2. Requirements fit

**Блокер первичного ревью не устранён:** в `tx_batch_profile_drop` по-прежнему проверяется ключ **`shard_mismatch`** в `sync_tx_drop_reason_total`, тогда как для сценария `route_test(..., full_v1: false, ...)` реализация увеличивает **`profile_mismatch`** (`route_sync_stub`, ветка `if !full_v1 || !same_shard`). Это ровно расхождение из первичного FAIL, hotfix-testing и таблицы pwm-testing по Slice 5.

Содержательная часть Slice 5 (метрики, runbook, RFC-согласование) по-прежнему выглядит согласованной с предыдущим ревью; повторная проверка **прод-кода не менялась** — менялась бы только ожидаемость теста хотфиксом.

## 3. Style and module shape

Новых изменений в прод-коде в рамках ре-гейта нет; повторный прогон скрипта имён на diff не требовался. Замечание из первого ревью (дубликат `add_bucket` vs `add_str_u64_bucket`) остаётся nit без изменения статуса.

## 4. Safety

Без изменений относительно первого ревью; отдельных новых рисков от «отсутствующего» хотфикса нет — регресс только в **красном CI/юнитах**.

## 5. Tests

Выполнено локально (workspace `P:\opt\docker\pwm-protocol`):

| Команда | Результат |
|---------|-----------|
| `cargo test -p pwmd tx_batch_profile_drop` | **FAIL** (`assertion left == right`: для ключа `shard_mismatch` получено `None`) |
| `cargo test -p pwmd peer_session::tests` | **14 PASS / 1 FAIL** — единственное падение **`tx_batch_profile_drop`** |

**Новых регрессий** относительно отчёта pwm-testing (**14/1**) не появилось: профиль тот же, хотфикс **не интегрирован**.

## 6. Verdict

**request changes** — повторный гейт **не пройден**: блокер **`tx_batch_profile_drop`** сохраняется; отчёт pwm-testing по хотфиксу фиксирует то же на **`HEAD` `53fab53`**; отдельного коммита pwm-coding с заменой ожидания в **`peer_session/mod.rs`** не наблюдается (последнее изменение модуля для этого сценария остаётся в базе **`9029fb0`**).

**Краткий код для оркестратора:** **FAIL**.

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: FAIL
artifacts:
  - docs/reviews/20260508-v2-8-slice5-rereview.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 8000
  confidence: medium
```

---

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260508-v2-8-slice5-rereview.md'
git add 'tasks/20260508-v2-sprint8-slice5-observability-chaos-docs.json'
git add 'tasks/20260508-v2-slice5-hotfix-profile-drop-test.json'
git commit -m 'docs(v2-8-s5): pwm-review re-gate FAIL — hotfix test still missing'
```
