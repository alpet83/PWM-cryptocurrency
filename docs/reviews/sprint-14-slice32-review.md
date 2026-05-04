# Sprint 14 Slice32 Review

## Verdict
`approve with nits`

## Summary
- TUI больше не скрывает own-address из `address_book` в списке Receivers.
- На уровне протокола self-transfer (`to == sender`) отклоняется в `validate_tx_shape` с `InvalidTransfer`.
- Изменения не конфликтуют с recipient-init gate и текущим cross-shard flow.

## Nits
1. Добавить e2e/API проверку self-transfer reject через `pwmd` endpoint.
2. Добавить TUI сценарный тест на F6 self-recipient с проверкой пользовательского текста ошибки.
