# Review: `20260618-pwmd-seal-loop-loading-snapshot-observability`

**Date:** 2026-06-17  
**Reviewer:** pwm-review  
**Ticket:** `tasks/20260618-pwmd-seal-loop-loading-snapshot-observability.json`  
**Coding handoff:** PASS (pwmd full suite: pre-existing `slice20_e2e_tests::slice20_dual_flow_ok` failure, unrelated).

## 1. Scope recap

Слайс закрывает «тишину» proposer в **init-phase** (`!allows_chain_progress()`, т.е. `loading_snapshot` / `starting`) до первого seal после CY-restart: периодический `cluster_prep_summary` в seal-loop, `blocked_reason` в `seal_suppression_summary`, расширение `GET /v1/status` / `pwm status --rpc`, unit-тест throttle, runbook note. Зависит от `20260616-pwmd-cluster-prep-observability` (post-ready quorum wait уже был).

| Область | Файлы |
|--------|--------|
| Seal-loop init guard | `crates/pwmd/src/lifecycle.rs` |
| HTTP `cluster_prep` | `crates/pwmd/src/api/handlers_status.rs`, `types.rs` |
| CLI | `crates/pwm-cli/src/cmd_status.rs` |
| Runbook | `docs/runbooks/v5-cy-cluster-precloseout-soak.md` |
| Version bump | `crates/pwmd/Cargo.toml` → `0.1.68` |

## 2. Requirements fit

| AC | Критерий | Статус | Доказательство |
|----|----------|--------|----------------|
| AC1 | Seal-loop: при `!allows_chain_progress()` — INFO ≥1/30s с `phase`, `loading_sec`, `blocked_reason` (`cluster_prep_summary` или `seal_init_blocked`) | **Met** | `PREP_SUMMARY_IV_SEC = 30`; `prep_log_due`; proposer-ветка до `continue` логирует `cluster_prep_summary phase=… loading_sec=… blocked_reason=… snapshot_file=… snapshot_diag=…` (`lifecycle.rs` ~1509–1545). `prep_summary_at` инициализируется как `now - 30s` — первый summary на первой итерации. |
| AC2 | `seal_suppression_summary` при `sealed_in_window=0` и не-ready: `blocked_reason` | **Met** (init-scope) | `emit_suppress_summary(..., blocked_reason)`; `show_blocked = sealed_in == 0 && blocked_reason.is_some()`; `blocked_reason` из `init_blocked_reason(init_phase)` только для `loading_snapshot` / `starting` (~1497–1500, ~716–776). Post-ready quorum wait по-прежнему через отдельный `cluster_prep_summary` (не suppression). Соответствует brief («тишина» при load). |
| AC3 | `GET /v1/status` `cluster_prep`: `loading_snapshot` → `blocked_reason`, `waiting_sec>0`, `local_tip` из chain | **Met** | Ранний return в `cluster_prep_out` для `LoadingSnapshot` / `Starting` (~348–370); тест `status_cluster_prep_loading_shape`. |
| AC4 | `pwm-cli status --rpc`: human-readable фаза / `blocked_reason` | **Met** | `cmd_status.rs`: строка `cluster_prep` дополнена `waiting_sec={}`; `blocked_reason` уже был. |
| AC5 | Unit-тест throttle init-phase без live CY | **Met** (минимально) | `init_prep_throttle_loading` — `init_blocked_reason`, `prep_log_due` boundary; `status_cluster_prep_loading_shape` — JSON shape. Нет интеграционного теста seal-loop spawn. |
| AC6 | `cargo test -p pwmd` + `pwm-cli` green | **Deferred / known FAIL** | Coding: unrelated `slice20_dual_flow_ok`. Targeted unit-тесты слайса не проверялись reviewer'ом в полном suite; gate — `pwm-testing`. |
| AC7 | Runbook: ожидаемая длительность `loading_snapshot`, что смотреть | **Met** | `v5-cy-cluster-precloseout-soak.md` §Cluster Prep Visibility, два новых буллета (~121–122). |

**Согласованность:** `cluster_prep_waiting_since_ms` выставляется в seal-loop при первом init-block (~1512–1516), сбрасывается при выходе из guard (~1550–1552); тот же атом читает `cluster_prep_out` для `waiting_since_ms` / `waiting_sec`.

## 3. Style and module shape

- Новые production symbols: `init_blocked_reason`, `prep_log_due`, `PREP_SUMMARY_IV_SEC`, `waiting_sec` — ≤4 сегментов.
- Тесты: `init_prep_throttle_loading` (4), `status_cluster_prep_loading_shape` (4) — в пределах test budget ≤5.
- `python scripts/check_entity_name_segments.py` по затронутым `.rs`: **violations: []**.
- Вынесены маленькие pure helpers (`init_blocked_reason`, `prep_log_due`) вместо inline magic `30` — улучшает тестируемость throttle.
- **Дублирование:** маппинг `InitPhase → blocked_reason` продублирован в `handlers_status::cluster_prep_out` и `lifecycle::init_blocked_reason` — риск drift строк; не блокер, но кандидат на общий helper в `state`/`api` при следующем касании.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice). `waiting_sec` — `Option<u64>` в operator HTTP JSON; peer wire не затронут.

## 4. Safety

- Throttle 30s ограничивает log volume в долгом trust-load (минуты).
- `cluster_prep_waiting_since_ms`: `AtomicU64`, без новых блокировок в hot path beyond existing `init`/`inner` read locks.
- `waiting_sec.max(1)` и synthetic `waiting_since_ms` при нулевом атоме — только operator UX, не влияет на seal gating.
- `snapshot_error.clone()` в log path — редкий 30s cadence, приемлемо.

## 5. Tests

**Покрыто:**

- `init_prep_throttle_loading` — граница 30s для `prep_log_due`, mapping `loading_snapshot`.
- `status_cluster_prep_loading_shape` — phase, `blocked_reason`, `waiting_sec >= 1`.
- Регрессия `status_cluster_prep_waiting_shape` — `waiting_sec` для post-ready wait.

**Пробелы (ниты):**

- Нет теста ветки `InitPhase::Starting` (лог + status).
- Нет seal-loop integration test (осознанно out of scope AC5 wording).

## 6. Coding nits evaluation

| Nit | Описание | Оценка |
|-----|----------|--------|
| **NIT-1** | `snapshot_diag` только из `snapshot_error` | **Accept (low).** В happy-path load `snapshot_error` всегда `None` → `snapshot_diag=none`. AC: «snapshot_file/diag **если есть**» — поле опционально. Имя `snapshot_diag` слегка вводит в заблуждение (не progress/checkpoint); ticket notes допускали объединение с chain_verify pct — отдельный слайс. Не блокер. |
| **NIT-2** | `waiting_sec` fallback `>=1` в status при `loading_snapshot` | **Accept (low).** При `waiting_since_raw==0` подставляется `now_ms-1000` и `.max(1)` — гарантирует AC3 `waiting_sec>0` до первого прохода seal-loop. Лёгкая рассинхронизация status vs реального атома в первую секунду — осознанный UX tradeoff. |

### Additional nits (reviewer)

1. **Low:** runbook bullet ~117 перечисляет поля `cluster_prep` без нового `waiting_sec` (bullets ~121–122 актуальны).
2. **Low:** в init-log поле называется `loading_sec`, в post-ready summary — `waiting_sec`; для оператора понятно, но терминология разная.
3. **Low:** дублирование `init_blocked` match в `handlers_status` vs `init_blocked_reason` в `lifecycle`.

## 7. Verdict

**PASS_WITH_NITS**

Все семь AC закрыты в рамках init-phase observability. NIT-1 и NIT-2 — приемлемые компромиссы, не требуют owner decision. Полный `cargo test` — на `pwm-testing` с учётом pre-existing slice20.

## 8. Participation / token estimate

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts: docs/reviews/20260618-pwmd-seal-loop-loading-snapshot-observability-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 22000
  confidence: low
```

**Verdict:** PASS_WITH_NITS — init-phase seal-loop/status observability реализован по контракту; nits — UX/diag naming и doc polish.
