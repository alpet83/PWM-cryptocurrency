# Sprint 15 / S3 testing: genesis/hash guardrails

Дата: 2026-04-29
Репозиторий: `P:/opt/docker/PWM-cryptocurrency`

## Scope и проверка

- [x] mismatch latches blocked genesis guard
- [x] user tx-affecting endpoints return blocked response (`503`)
- [x] `/v1/status` exposes effective genesis hash and mismatch diagnostics fields
- [x] no regressions in normal path for status basics

## Что запускалось

1. `cargo test -p pwmd v1_peer_hello_rejects_bad_signature_replay_network_genesis_and_malformed -- --nocapture`
   - Результат: PASS (`1 passed`, `0 failed`)
   - Время процесса: ~8.46s
   - Подтверждение: mismatch (`genesis_mismatch`) корректно фиксируется на peer-hello проверке.

2. `cargo test -p pwmd v1_genesis_mismatch_blocks_user_tx_paths -- --nocapture`
   - Результат: PASS (`1 passed`, `0 failed`)
   - Время процесса: ~7.26s
   - Подтверждение: после genesis mismatch user tx paths возвращают `503` и текст `"user tx blocked"` (в тесте проверены `/v1/tx` и `/v1/roaming-intents`).

3. `cargo test -p pwmd v1_status_exposes_genesis_guard_diagnostics -- --nocapture`
   - Результат: PASS (`1 passed`, `0 failed`)
   - Время процесса: ~9.25s
   - Подтверждение по `/v1/status`: `genesis_guard=blocked`, `effective_genesis_hash` заполнен, а также присутствуют и валидны:
     - `genesis_mismatch_total`
     - `genesis_mismatch_expected_hash`
     - `genesis_mismatch_received_hash`
     - `genesis_mismatch_peer_node_id`
     - `genesis_mismatch_peer_hint`
     - `genesis_mismatch_at_unix_ms`
     - `genesis_guard_recovery_hint`

4. `cargo test -p pwmd v1_status_reports_ -- --nocapture`
   - Результат: PASS (`5 passed`, `0 failed`)
   - Время процесса: ~7.90s
   - Проверены базовые normal-path статус тесты:
     - `v1_status_reports_alias_state_namespace_for_shard`
     - `v1_status_reports_neutral_relay_baseline_without_alias_shard`
     - `v1_status_reports_explicit_domain_state_namespace`
     - `v1_status_reports_loading_and_head_returns_503`
     - `v1_status_reports_ready_degraded_after_snapshot_error`

5. `cargo test -p pwmd v1_status_exposes_split_balance_semantics_contract -- --nocapture`
   - Результат: PASS (`1 passed`, `0 failed`)
   - Время процесса: ~9.90s
   - Дополнительная проверка статус-контракта (`balance_semantics`) без регрессии.

## Итоговый вердикт

**PASS**

Все заявленные guardrails из S15-S3 в указанном scope подтверждены таргетными тестами; дополнительных регрессий в базовых `/v1/status` сценариях не обнаружено.
