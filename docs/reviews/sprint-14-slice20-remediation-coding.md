# Sprint 14 — Slice 20 remediation (coding)

## Исправлено (закрытие блокеров)
1. `pwm-core`: `TRANSFER` и `IMPORT` больше не “seal skip”-ятся из‑за отсутствующего/неинициализированного получателя на target‑стороне.
   - При первом входящем переводе создаётся детерминированный stub‑аккаунт получателя.
   - Первый явный `Init` теперь корректно “привязывает” подпись к stub (без `BadSignature`).
2. `pwm-cli`: `tx-import` теперь ретраитит запрос при `400 invalid import: export_id is not known` (гонка доставки export‑provenance на target).
3. `pwmd`: `tx routing guard` безусловно печатает runtime shard label (`CY`/`DO`) из `domain_hi`, без legacy `A|B`.
4. `pwmd`: лог `tx commit delta` переведён на `info` уровень, чтобы быть видимым в стандартных тест/рантайм лог-уровнях.
5. Версия `pwmd` bumped до `0.1.11`, т.к. поведение публичных tx‑эндпоинтов изменилось (ошибки `account not found`/`NotInitialized` превращаются в успешный коммит).
6. `pwmd`: `validate_snapshot` теперь не валидирует `AccountId` для uninitialized stub‑аккаунтов, чтобы allow‑auto‑create получателей на первом входящем transfer.

## Добавленные/обновлённые тесты
- `pwm-core`: `apply_tx_transfer_creates_uninitialized_recipient_stub`
- `pwm-core`: `apply_tx_import_creates_uninitialized_destination_stub`
- `pwmd`: `snapshot_roundtrip_loads_after_transfer_to_missing_recipient_stub`
- `pwmd`: `snapshot_roundtrip_loads_after_export_only_tx`
- `pwmd`: `shard_label_for_domain_hi_maps_to_expected_runtime_labels`
- `pwm-cli`: `tx_import_retries_until_export_id_known`

## Команды (выполнить/проверить)
- `cargo fmt`
- целевые `cargo test -p pwm-core -p pwmd -p pwm-cli` по добавленным тестам

