# Sprint 14 — Slice 7 polish review

## Findings
- Подтверждён warning: `load_wallet_resume_start_index` не используется в runtime.
- Подтверждено несоответствие resume-fallback: при отсутствии целевого кластера используется `global_max + 1`, а не `0`.
- В `addr-bruteforce` progress/result вывод сейчас без 4-space отступов и без разделителя.
- В результирующем выводе всё ещё есть ключ `account_id_hex` вместо `id_hex`.

## Required fixes
1. Удалить/ограничить legacy `load_wallet_resume_start_index`, чтобы не было dead_code warning.
2. В `load_wallet_resume_start_index_for_domain` fallback для отсутствующего target-кластера сделать `0`.
3. Отформатировать консольный вывод `addr-bruteforce`:
   - 4 пробела перед progress/result строками;
   - линия `-------------` перед result-блоком.
4. В result-блоке использовать `id_hex` вместо `account_id_hex`.
5. Обновить тесты под новую fallback-семантику и формат/ключи вывода.

## Verdict
`request changes`.
