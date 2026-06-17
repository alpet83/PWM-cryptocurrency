# Review: `20260616-pwmd-cluster-prep-observability`

**Date:** 2026-06-17  
**Reviewer:** pwm-review  
**Bridge:** `.cqds/team-tasks/done/20260616-pwmd-cluster-prep-observability.json` — `result_payload.result=PASS`, `ticket_id` matches.

## 1. Scope recap

Слайс добавляет операторскую видимость **cluster prep** после CY-restart: периодические логи proposer/attester, блок `cluster_prep` в `GET /v1/status`, команда `pwm status --rpc …`, заметка в runbook и unit-тесты без live CY.

Затронутые файлы (по `result_payload.files_touched` и проверке дерева):

| Область | Файлы |
|--------|--------|
| Состояние ожидания | `crates/pwmd/src/state.rs`, `crates/pwmd/src/bootstrap.rs` |
| Proposer seal loop | `crates/pwmd/src/lifecycle.rs` |
| Attester catch-up | `crates/pwmd/src/transport/peer_session/sync_live.rs` |
| HTTP status | `crates/pwmd/src/api/types.rs`, `crates/pwmd/src/api/handlers_status.rs` |
| CLI | `crates/pwm-cli/src/cmd_status.rs`, `cli_cmd.rs`, `cli_dispatch.rs`, `lib.rs` |
| Runbook | `docs/runbooks/v5-cy-cluster-precloseout-soak.md` |

Предыдущий отчёт (FAIL 0/7) устарел: код cluster prep присутствует в рабочем дереве.

## 2. Requirements fit

| AC | Критерий | Статус | Доказательство |
|----|----------|--------|----------------|
| AC1 | Proposer: при `WaitingAttester` summary ≥30s wall с `live_synced_attesters`, `live_connected`, `proposer_tip`, `attester_tip_max`, `max_tip_lag`, `waiting_sec` | **Met** | `lifecycle.rs`: `prep_summary_at` инициализируется как `Instant::now() - 30s` (стр. ~1365); в ветке `SealPreflight::WaitingAttester` — `cluster_prep_waiting_since_ms`, `waiting_sec`, `info!(cluster_prep_summary …)` при `prep_summary_at.elapsed() >= 30s` (стр. ~1500–1536). |
| AC2 | Attester sync: при `lag>0` progress ~10s с `local_h`, `head_h`, `blocks_behind`, `percent_complete` | **Met** | `sync_live.rs`: `SYNC_STALL_LOG_MS = 10_000`, `sync_stall_tick` (стр. ~156–175); при `stall_hit` — `sync_catchup_progress` с `percent_complete` из `sync_prog_snap` (стр. ~727–741). |
| AC3 | `GET /v1/status`: `cluster_prep` — все поля контракта | **Met** | `types.rs` `ClusterPrepOut` (стр. ~114–127); `handlers_status.rs` `cluster_prep_out` + включение в `StatusOut` (стр. ~242, 316, 333–354). |
| AC4 | `pwm-cli`: human-readable `cluster_prep` через `--rpc` | **Met** | `cmd_status.rs` `run_status`; глобальный `--rpc` в `cli_cmd.rs`; `Cmd::Status` → `cli_dispatch.rs`. |
| AC5 | Unit-тесты: throttle + JSON shape, без live CY | **Met** (с нитами) | `handlers_status::tests::status_cluster_prep_waiting_shape`; `sync_live::tests::sync_stall_tick_10s`. |
| AC6 | `cargo test -p pwmd` + `pwm-cli` green | **Deferred to pwm-testing** | Reviewer прогнал targeted lib-тесты (см. §5); полный suite — на конвейере testing. |
| AC7 | Runbook: CY restart — логи и `pwm status` до `sealed height=` | **Met** | `docs/runbooks/v5-cy-cluster-precloseout-soak.md` §Cluster Prep Visibility (стр. ~113–120). |

**Контрактные поля `/v1/status`:** `phase`, `ready_for_seal`, `sync_n`, `live_n`, `peer_tip_max`, `local_tip`, `blocks_behind_max`, `waiting_since_ms`, `blocked_reason` — все присутствуют в `ClusterPrepOut` и заполняются в `cluster_prep_out`.

**Согласованность лог ↔ status:** `waiting_sec` в логе proposer и `waiting_since_ms` в API используют один атом `cluster_prep_waiting_since_ms` (устанавливается при входе в wait, сбрасывается в 0 при готовности attester quorum).

## 3. Style and module shape

- Новые идентификаторы (`cluster_prep_out`, `cluster_prep_waiting_since_ms`, `ClusterPrepOut`, `sync_stall_tick`, `cmd_status`) укладываются в лимит ≤4 сегментов для production-кода.
- `check_entity_name_segments.py` по затронутым путям: **violations: []**.
- `cmd_status.rs` имеет минимальный `//!` banner; `ClusterPrepOut` без избыточных комментариев — ок для тонкого DTO.
- Логика cluster prep вынесена в `cluster_prep_out` и локальный throttle в seal loop — без раздувания `main.rs`.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice). `cluster_prep` — HTTP operator JSON с `u64` полями; peer `sync_live` не меняет wire-типы.

## 4. Safety

- `cluster_prep_waiting_since_ms`: `AtomicU64`, без блокировок в hot path seal loop.
- Throttle 30s / 10s снижает log spam (раньше оператор видел «тишину» или редкий dedup per-height).
- `cmd_status.rs`: ошибки RPC/JSON → `exit_user_error` (как в остальном pwm-cli); без новых trust boundaries.
- `blocks_behind_max` вычисляется как `peer_tip_max.saturating_sub(local_h)` — для proposer это ожидаемый операторский сигнал; не max по всем peer-id (имя поля слегка шире семантики, не блокер).

## 5. Tests

**Покрыто:**

- `status_cluster_prep_waiting_shape` — фаза `waiting_attester`, `ready_for_seal=false`, `waiting_since_ms`, `blocked_reason`.
- `status_exposes_identity_signals` — регрессия `cluster_prep.phase=ready` на dev net.
- `sync_stall_tick_10s` — 10s cadence при неизменном `rem`.

**Reviewer execution** (`CARGO_TARGET_DIR=F:/pwm-test/PWM-cryptocurrency`):

- `cargo test -p pwmd --lib cluster_prep` → 1 passed.
- `cargo test -p pwmd --lib sync_stall_tick` → 1 passed.

**Пробелы (ниты, не блокеры):**

- Нет unit-теста на 30s cadence `cluster_prep_summary` в `lifecycle` (только ручная инспекция `prep_summary_at`).
- Нет mocked HTTP теста для `pwm status` / `cmd_status::run_status` (тонкая обёртка над JSON).

## 6. Verdict

**PASS_WITH_NITS**

Реализация закрывает все семь acceptance criteria. Bridge `PASS` с корректным `ticket_id` подтверждён. Ниты — только усиление тестового покрытия и уточнение семантики `blocks_behind_max`; продуктовых/протокольных решений не требуют.

### Nits (optional follow-up)

1. **Low:** добавить unit-тест на 30s throttle `cluster_prep_summary` (extract helper или clock injection в seal-loop dedup).
2. **Low:** mocked `pwm status` test в `pwm-cli` (проверка одной строки `cluster_prep …`).
3. **Low:** в runbook или `ClusterPrepOut` doc — одна фраза, что `blocks_behind_max = peer_tip_max - local_tip` на данном узле.

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts: docs/reviews/20260616-pwmd-cluster-prep-observability-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 28000
  confidence: low
```

**Verdict:** PASS_WITH_NITS — cluster prep observability реализован по контракту; полный `cargo test` — на pwm-testing.
