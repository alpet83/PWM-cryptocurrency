# Sprint 14 Slice 6 — coding note

Дата: 2026-04-28

Сделано (scope-tight):
- `wallet init`, `wallet import-seed`, `addr-bruteforce` переведены на immediate write в schema v3 (без изменения read-compat v2/v3 и контракта `--upgrade-wallet`).
- user-visible ключ pretty account id унифицирован как `id_pretty` в соответствующих CLI-выводах.
- Для `addr-bruteforce` добавлен resume от существующего `--wallet-out`: старт поиска с `max_derivation_index + 1`, что исключает повторный перебор с нуля.

Покрытие тестами:
- обновлены/добавлены unit-тесты на v3 create-path, naming consistency (`id_pretty`) и resume-индекс для `addr-bruteforce`.
