# S2: lease/fencing failover (single sealer)

Дата: 2026-05-09  
Тикет: `tasks/20260509-single-sealer-failover-profiles.json`  
Срез: `S2` (lease/fencing active-standby for same-validator clones)

## Что реализовано

- Добавлен lightweight lease/fencing runtime (`crates/pwmd/src/lease.rs`) с полями:
  - `owner_id` (`node_instance_id`)
  - `term`
  - `expires_at_ms`
  - `last_tip`
  - `fence`
- В `single_sealer` seal-loop теперь проходит только при валидной локальной аренде.
- Standby/takeover логика:
  - takeover разрешается только после `expires_at_ms + takeover_timeout_ms`;
  - takeover блокируется при stale tip (`local_tip + max_tip_lag < lease.last_tip`);
  - takeover поднимает `term` и `fence`.
- Возврат старого active без аренды блокируется (self-fence to standby).

## Сигналы и наблюдаемость

- Hello/heartbeat/status получили lease/fencing поля:
  - `lease_owner_id`, `lease_term`, `lease_expires_at_ms`, `lease_last_tip`, `lease_fence`.
- Добавлены state/gate поля в `/v1/status`:
  - `lease_state`, `seal_gate_allowed`, `lease_last_reason`.
- Добавлены counters в status:
  - `lease_acquire_ok`, `lease_renew_ok`, `lease_loss_total`, `lease_reject_total`, `lease_takeover_ok`.
- Логи:
  - `seal_lease_acquired`
  - `seal_lease_renewed`
  - `seal_lease_lost`
  - `seal_suppressed_by_fence`
  - `seal_takeover_committed`

## Новые operator knobs

- `--seal-lease-ttl-ms` / `PWM_SEAL_LEASE_TTL_MS` (default `10000`)
- `--seal-takeover-timeout-ms` / `PWM_SEAL_TAKEOVER_TIMEOUT_MS` (default `8000`)
- `--seal-takeover-max-tip-lag` / `PWM_SEAL_TAKEOVER_MAX_TIP_LAG` (default `1`)

## Проверки

- `cargo test -p pwmd lease_renew_ok_same_owner`
- `cargo test -p pwmd lease_takeover_after_timeout`
- `cargo test -p pwmd old_active_blocked_without_lease`
- `cargo check -p pwmd`

## Ограничение MVP

Текущий lease backend — process-local in-memory coordinator (в рамках текущего runtime процесса). Для multi-process или multi-host HA нужен следующий шаг: внешний shared backend (file lock / KV / coordinator service) или wire-authoritative lease exchange с устойчивым источником истины.
