# Sprint 15 Slice S15-S3 Coding

Дата: 2026-04-29  
Слайс: `S15-S3-GENESIS-GUARD`

## Что изменено

- `crates/pwmd/src/transport.rs`
  - Добавлен latched guard state `genesis_guard` в `HandshakeState`.
  - На `genesis_mismatch` в `process_incoming_peer_hello` теперь фиксируется mismatch-диагностика (expected/received hash, peer, timestamp) и включается блокирующий флаг.
- `crates/pwmd/src/api.rs`
  - Добавлен `ensure_user_tx_allowed(...)`: при активном genesis guard возвращает `503` c явным сообщением block/recovery.
  - Применён к user tx путям: `/v1/tx`, `/v1/export-readiness`, `/v1/roaming-intents`, `/v1/roaming-intents/:id/finalize`.
  - Расширен `/v1/status` полями effective genesis/hash и mismatch diagnostics:
    - `effective_genesis_hash`
    - `genesis_guard`
    - `genesis_mismatch_total`
    - `genesis_mismatch_expected_hash`
    - `genesis_mismatch_received_hash`
    - `genesis_mismatch_peer_node_id`
    - `genesis_mismatch_peer_hint`
    - `genesis_mismatch_at_unix_ms`
    - `genesis_guard_recovery_hint`
- `crates/pwmd/src/lib.rs`
  - Добавлены focused tests для S15-S3 acceptance.
- `crates/pwmd/Cargo.toml`
  - Bump `pwmd` build/version marker: `0.1.19 -> 0.1.20` (изменение публичного API-контракта и поведения endpoint guardrails).

## Тесты (focused)

- `v1_genesis_mismatch_blocks_user_tx_paths`
  - Проверяет, что после `genesis_mismatch` user tx пути явно блокируются (`503`) и не работают в false-healthy режиме.
- `v1_status_exposes_genesis_guard_diagnostics`
  - Проверяет, что `/v1/status` публикует effective genesis/hash и mismatch diagnostics, включая recovery hint.

## Ограничения слайса

- Scope ограничен S15-S3: только guardrails join/start/status и блокировка user tx при mismatch.
- Не тронуты задачи S15-S4 (snapshot DB abstraction, backend selector, DB wiring).
- Нет изменений в storage backend модели и DB-интерфейсах.
