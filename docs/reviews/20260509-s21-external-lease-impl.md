## S2.1 external lease backend implementation (MVP)

Дата: 2026-05-09  
Тикет: `tasks/20260509-s21-external-lease-backend.json`

### Что реализовано

1. Добавлена абстракция `LeaseBackend` с CAS-операциями:
   - `acquire`
   - `renew`
   - `release`
   - `takeover`
2. Добавлены реализации:
   - `FileLeaseBackend` (MVP внешний backend)
   - `ProcessLocalLeaseBackend` (явный fallback для test/dev)
3. Формат lease-record в файле:
   - `owner_id`
   - `validator_identity_hash`
   - `term`
   - `fence`
   - `expiry`
   - `last_tip`
   - `updated_at`
4. File backend использует lock + CAS + `tmp + rename`:
   - lock file `<lease_dir>/<validator_identity_hash>.lease.lock`
   - record file `<lease_dir>/<validator_identity_hash>.lease.json`
5. S2 gate в `single_sealer` интегрирован с backend:
   - seal разрешён только при валидном lease из backend
   - backend ошибки -> fail-closed (`seal_suppressed_by_fence`)
6. Observability расширена:
   - `/v1/status`: `lease_backend_mode`, `lease_backend_path`, `lease_last_backend_error`
   - логирование CAS-fail (`seal_lease_cas_failed`) и переходов lease

### Конфигурация и defaults

- `--seal-lease-backend` / `PWM_SEAL_LEASE_BACKEND`  
  Значения: `file | process-local`  
  Default: `file`

- `--seal-lease-dir` / `PWM_SEAL_LEASE_DIR`  
  Default: `<state_root>/leases`

- `--seal-lease-ttl-ms` / `PWM_SEAL_LEASE_TTL_MS`  
  Default: `10000`

- `--seal-takeover-timeout-ms` / `PWM_SEAL_TAKEOVER_TIMEOUT_MS`  
  Default: `8000`

- `--seal-takeover-max-tip-lag` / `PWM_SEAL_TAKEOVER_MAX_TIP_LAG`  
  Default: `1`

### Проверки

- Unit/CAS:
  - `lease_backend::tests::file_acq_then_renew_ok`
  - `lease_backend::tests::file_takeover_cas_gate`
  - `lease_backend::tests::file_release_cas_gate`
- Gate/симуляция same-key:
  - `lease::tests::file_two_node_takeover_sim`
  - `lease::tests::lease_takeover_after_timeout`
  - `lease::tests::old_active_blocked_without_lease`

### Gate condition (one-line)

`single_sealer` seals only when backend-confirmed lease is valid; any lease backend uncertainty/error suppresses sealing (fail-closed).
