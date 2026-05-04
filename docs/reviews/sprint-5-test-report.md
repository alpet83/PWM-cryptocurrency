# Sprint 5 Test Report (slice #8)

Дата: 2026-04-24  
Исполнитель: `pwm-testing` (independent verification)

## Verdict

**PASS**

Slice #8 (`soak profile hooks + closeout readiness`) подтвержден: полный `pwmd` прогон зелёный, новые сценарии long-run soak/periodic aggregation/runaway safety stop+resume покрыты тестами, additive контракт `/v1/dev/peers` совместим, регрессий по tx-path инвариантам и no-range policy не выявлено.

## Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`).

## Проверка slice #8: soak profile hooks + closeout readiness

- **long-run tick behavior (bounded soak rollups)**:
  - `real_transport_soak_rollups_are_bounded_and_periodic` подтверждает bounded long-run rollups (`soak_counter_cap`) и стабильное поведение на серии тиков.
- **periodic health aggregation**:
  - `real_transport_soak_rollups_are_bounded_and_periodic` подтверждает периодическую агрегацию health snapshot по `transport_soak_health_interval_ticks` и корректный `health_last_tick`.
- **runaway reconnect safety stop + cooldown resume**:
  - `real_transport_runaway_guard_stops_then_resumes_attempts` подтверждает включение runaway guard на streak limit и возобновление dial attempts после cooldown.
- **additive `/v1/dev/peers` soak/churn/transport fields**:
  - `v1_dev_peers_exposes_transport_snapshot` подтверждает совместимость additive полей transport/churn/soak (`seed_rotation_cursor`, `tick_attempt_budget`, `last_tick_attempts`, `soak.loop_ticks_capped`, `soak.runaway_stop_total`).

## Regression check

- **tx-path invariants**: регрессий не выявлено; зелёные `v1_tx_accepts_signed_init`, `v1_tx_rejects_domain_mismatch`, `v1_tx_rejects_wrong_shard_for_sender_domain_hi`, `v1_tx_rejects_cross_shard_transfer_on_local_path`, recipient prefilter (`reserve/witness/unknown`) и body-limit guard.
- **no range heuristics**: инвариант сохранён; зелёный `policy_classification_uses_only_domain_equality_no_ranges`.
- **dev endpoint compatibility**: `/v1/peer/hello` и `/v1/dev/peers` остаются совместимыми; зелёные `v1_peer_hello_accepts_and_classifies_native`, `v1_peer_hello_classifies_foreign_and_exposes_reject_counters`, `v1_dev_peers_exposes_transport_snapshot`.

## Изменённые файлы в рамках проверки

- `docs/reviews/sprint-5-test-report.md` (обновлён под slice #8 testing gate)
- `docs/reviews/sprint-5-status-note.md` (testing gate синхронизирован для slice #8 closeout readiness)
- `tasks/20260424-sprint5-orchestrated.json` (добавлены delegation + mini_report_slice8_testing)

## Risks / findings for pwm-review

- Blocking issues не обнаружены.
- Residual risk: long-run проверка выполнена в controlled test harness; отдельный операторский real-network soak остаётся желательным перед окончательным closeout.
