# Sprint 14 Slice 20 — Coding

## Что изменено

- Исправлена семантика маршрутизации same-hi:
  - добавлен общий helper `pwm_core::tx::same_hi_domain`;
  - `pwm-cli` (`tx-send`) и `pwm-tui` (`is_cross_domain_route`) переведены на same-hi правило вместо сравнения полного `u16` домена.
- Исправлен критичный non-atomic commit в roaming-path:
  - в `pwmd` API удалён паттерн `apply_tx` + `seal(vec![])` для `EXPORT/IMPORT`;
  - теперь используется `chain.seal(vec![tx])`, чтобы block payload содержал реальный tx и replay был консистентным.
- Добавлен rollback на late failure (snapshot persist):
  - перед roaming commit создаётся backup runtime-состояния;
  - при ошибке `snapshot save` выполняется rollback chain/state/roaming/flow и возвращается `500` без внешне наблюдаемого debit/nonce изменения.
- Унифицирован label в guard-логах:
  - `tx routing guard` теперь пишет runtime-shard label (`CY|DO` по `domain_hi`) вместо legacy `A|B`.
- Усилена debug-наблюдаемость commit:
  - добавлен лог `tx commit delta` с `tx_id`, sender и `balance/nonce before->after`.

## Регрессии/тесты

- Добавлен тест в `pwm-core`: `same_hi_domain_checks_only_hi_byte`.
- Добавлен тест в `pwm-tui`: `same_hi_route_is_local_not_roaming`.
- Обновлены/добавлены тесты `pwmd`:
  - `v1_tx_accepts_export` теперь проверяет, что sealed block содержит tx payload;
  - `v1_roaming_intent_create_and_get_status` проверяет tx payload в блоке;
  - `v1_roaming_intent_returns_500_when_snapshot_save_fails` проверяет rollback (nonce/balance без частичного debet);
  - новый `v1_tx_export_rollback_keeps_sender_state_on_snapshot_fail` проверяет отсутствие partial sender debit/nonce при failed commit.

## Команды и результаты

- `cargo fmt` — OK.
- `cargo check` — OK.
- Таргетные тесты:
  - `cargo test -p pwm-core same_hi_domain_checks_only_hi_byte` — OK.
  - `cargo test -p pwm-tui same_hi_route_is_local_not_roaming` — OK.
  - `cargo test -p pwmd v1_tx_accepts_export` — OK.
  - `cargo test -p pwmd v1_tx_export_rollback_keeps_sender_state_on_snapshot_fail` — OK.
  - `cargo test -p pwmd v1_roaming_intent_create_and_get_status` — OK.
  - `cargo test -p pwmd v1_roaming_intent_returns_500_when_snapshot_save_fails` — OK.
