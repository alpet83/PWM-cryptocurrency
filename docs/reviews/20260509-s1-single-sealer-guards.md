# S1 single-sealer guards: coding report

Дата: 2026-05-09  
Тикет: `tasks/20260509-single-sealer-failover-profiles.json`  
Срез: `S1`

## Что реализовано

1. Runtime profile в `pwmd`:
   - Добавлен `deployment_profile` с default `single_sealer`.
   - Добавлен явный `multi_sealer_experimental` (non-default).
   - Добавлены CLI/env toggles:
     - `--deployment-profile` / `PWM_DEPLOYMENT_PROFILE`
     - `--seal-role` / `PWM_SEAL_ROLE`
2. Hello/status identity signals:
   - `validator_identity_hash`
   - `node_instance_id`
   - `seal_role`
   - (дополнительно) `deployment_profile`
3. Same-key strict baseline:
   - В `single_sealer` конфликт `active/active` с тем же `validator_identity_hash` отклоняется.
   - `active/standby` разрешён.
   - Policy применён в inbound handshake path (`process_incoming_peer_hello`) и поэтому одинаково работает для connect/reconnect.
4. Логи и метрики:
   - Добавлен явный reject reason code: `same_validator_active_conflict`.
   - Для policy reject инкрементируется `reject_reason_total["same_validator_active_conflict"]`.
   - На старте добавлен операторский warning при `multi_sealer_experimental`.

## Тесты S1

Добавлены/обновлены targeted тесты:

- `profile_default_single_sealer`
- `reject_same_validator_active_active`
- `allow_same_validator_active_standby`
- `hello_propagates_identity_signals`
- `status_exposes_identity_signals`

Плюс `cargo check -p pwmd` в зелёном состоянии.

## Операционные заметки

- Для standby в MVP достаточно:
  - `--deployment-profile single-sealer`
  - `--seal-role standby`
- `--debug-disable-seal-loop` остаётся совместимым fallback, но приоритетная ручка роли теперь `--seal-role`.
