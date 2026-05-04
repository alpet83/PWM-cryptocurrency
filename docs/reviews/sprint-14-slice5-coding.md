# Sprint 14 — Slice 5 (coding): automatic wallet migration

## Scope

- Реализована авто-миграция кошелька `schema_version: 2` в `schema_version: 3` при загрузке через CLI wallet flow.
- Поведение выполнено в `crates/pwm-cli/src/wallet.rs` внутри `load_wallet_yaml`.

## What changed

- Добавлена функция миграции `migrate_wallet_v2_to_v3`, которая:
  - формирует `accounts` из активного legacy-аккаунта;
  - переносит `mode`, `created_at_unix_sec`, country label, `address_book`;
  - сохраняет секретный контракт по режимам:
    - `plaintext_dev`: переносит plaintext-поля как есть;
    - `encrypted`: переносит только encrypted-поля (`encrypted_payload_b64`, `kdf_*`, `aead_nonce_b64`) без неожиданной записи секретов в plaintext-поля.
- В `load_wallet_yaml` после нормализации legacy полей добавлен шаг:
  - если исходный schema = 2, файл автоматически переписывается как v3 и повторно парсится как v3 до продолжения выполнения команды.
- Снят искусственный блок на чтение `schema v3` в режиме `encrypted` (раньше возвращалась ошибка `schema v3 decrypt not implemented`).

## Tests added

- `load_wallet_yaml_auto_migrates_plaintext_v2_to_v3`
- `load_wallet_yaml_auto_migrates_encrypted_v2_to_v3`

Оба теста проверяют, что:
- загрузка проходит успешно;
- файл на диске становится `schema_version: 3`;
- контракт секретов сохраняется (для encrypted — расшифровка по passphrase работает, plaintext-поля не заполняются неожиданно).

## Notes

- Изменение выполнено минимально и локально: без изменения CLI API и без изменения формата v3 сверх необходимого для миграции.
