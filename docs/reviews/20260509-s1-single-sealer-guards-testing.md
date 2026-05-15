# S1 single-sealer guards: отчёт `pwm-testing`

Дата: 2026-05-09  
Тикет: `tasks/20260509-single-sealer-failover-profiles.json`  
Проверяемые коммиты: `c9ecad6` (реализация), `1f30db6` (обновление тикета под S1 coding).

## Вердикт

**PASS** — все целевые автотесты и `cargo check -p pwmd` завершились успешно (exit code 0).

## Preflight

- Скрипт: `powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/dev/preflight_target_debug.ps1`
- Результат: **PASS** (`target/debug` ≈ 226 464 982 байт, ниже порога 4096 MiB).

## CQDS / MCP

- Перед долгими процессами сверена справка MCP: `cq_help` → `cq_process_ctl#spawn` (канонический контракт CQDS).
- Прогон тестов выполнен локально через `cargo` на хосте Windows (укороченная матрица из тикета); **`cq_process_ctl`** для этого прогона не потребовался.

## Матрица приёмки S1

| № | Критерий | Как проверено | Доказательство |
|---|-----------|----------------|----------------|
| 1 | Дефолтный профиль `single_sealer` | `cargo test -p pwmd profile_default_single_sealer` | `test config::tests::profile_default_single_sealer ... ok` |
| 2 | Active/active тот же validator → отказ со строкой-причиной | `cargo test -p pwmd reject_same_validator_active_active` | Ошибка `same_validator_active_conflict`: `assert_eq!(err, "same_validator_active_conflict")` в `incoming_hello` tests |
| 3 | Active/standby тот же validator → разрешено | `cargo test -p pwmd allow_same_validator_active_standby` | `PeerClass::Native`, тест `... ok` |
| 4 | Identity-сигналы в hello | `cargo test -p pwmd hello_propagates_identity_signals` | `transport::dial::...::hello_propagates_identity_signals ... ok`; покрыты `deployment_profile`, `seal_role`, `validator_identity_hash`, `node_instance_id` |
| 5 | Identity-сигналы в HTTP status | `cargo test -p pwmd status_exposes_identity_signals` | `api::handlers_status::tests::status_exposes_identity_signals ... ok` |
| 6 | Регрессия по затронутым путям (выборочно) | Тот же прогон + `cargo check -p pwmd` | Пять юнит-транспорт/status тестов по затронутым модулям + компиляция пакета без ошибок; **полный** `cargo test -p pwmd` в этом прогоне не выполнялся |

## Команды (выполнено из корня репозитория)

```text
powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/dev/preflight_target_debug.ps1
cargo test -p pwmd profile_default_single_sealer -- --nocapture
cargo test -p pwmd reject_same_validator_active_active -- --nocapture
cargo test -p pwmd allow_same_validator_active_standby -- --nocapture
cargo test -p pwmd hello_propagates_identity_signals -- --nocapture
cargo test -p pwmd status_exposes_identity_signals -- --nocapture
cargo check -p pwmd
```

## Замечания по охвату

- Код отказа для dual-active зафиксирован как стабильная строка **`same_validator_active_conflict`** (совместимо с метриками/причинами hello-reject в транспорте).
- Для «нет очевидной регрессии» достаточно узкой матрицы из реализации; полный прогон `cargo test -p pwmd` при необходимости — отдельным шагом CI или оркестратора.

## Snapshot benches

Не требовались для S1 (slice не про snapshot/CH); не запускались.
