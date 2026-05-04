# Sprint 14 — Slice 6 remediation (coding)

Исправлен блокирующий дефект сохранения schema v3:

- Create-path (`wallet init`, `wallet import-seed`, `addr-bruteforce`) остаётся на strict overwrite через `save_new_wallet_yaml_v3`, без merge с существующим содержимым destination.
- Upgrade-path (`--upgrade-wallet`) теперь пишет v3 строго (без merge), чтобы не переносить в итоговый файл устаревшие v1/v2 поля и неизвестный legacy baggage.
- Merge-preserve сохранён только для update-path, где это ожидаемо (`wallet account add`, `wallet account use`), чтобы не терять future metadata в уже v3 файлах.

Добавлены тесты в `crates/pwm-cli/src/wallet.rs`:

- create-path overwrite на существующем файле удаляет legacy/unknown top-level поля;
- upgrade persistence удаляет legacy/unknown top-level поля при записи v3.
