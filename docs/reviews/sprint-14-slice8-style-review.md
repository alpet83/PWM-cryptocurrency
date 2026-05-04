## Sprint 14 Slice 8 — style review

### Verdict
`request changes`

### Причина
Hard gate по стилю (`<=4` слова для production non-test идентификаторов в затронутом коде) не пройден.

### Остаточные нарушения (medium)
- `assert_tx_recipient_in_wallet_address_book`
- `load_wallet_yaml_with_upgrade`
- `to_wallet_yaml_with_metadata`
- `format_addr_bruteforce_progress_line`
- `format_addr_bruteforce_result_lines`

### Требование для закрытия
- Переименовать перечисленные production-символы в формат `<=4` слова.
- Повторно прогнать тесты по wallet resume/save/add.
