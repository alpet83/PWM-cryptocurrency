## Sprint 14 Slice 7 polish coding note

- Убрана неиспользуемая legacy-функция `load_wallet_resume_start_index`; runtime использует domain-aware путь без dead-code warning.
- В `load_wallet_resume_start_index_for_domain` fallback для отсутствующего target-domain изменён на `0` (fresh scan), при этом `overwrite_wallet=true` поведение `start=0` сохранено.
- Для `addr-bruteforce` добавлен форматированный вывод:
  - progress-строка с отступом в 4 пробела;
  - разделитель `-------------` перед result-блоком;
  - все result-строки с отступом в 4 пробела.
- В result-блоке `addr-bruteforce` ключ `account_id_hex` заменён на `id_hex` (schema-v3 semantics).
- Обновлены/добавлены тесты:
  - fallback `target absent -> 0`;
  - формат output (`separator`, 4-space indent, `id_hex` key).
