# Sprint 14 — Slice 10 Coding Report

Дата: 2026-04-28

## Что реализовано

- `pwmd` genesis loader переведён на **v3-only** контракт:
  - принимается только `schema_version=3`;
  - legacy/v2 форматы hard-fail с явными ошибками;
  - добавлена загрузка `validator_keys[*].enc_seed` (envelope `kdf + aead`);
  - enforced fixed path `m/1000000'/1'` (`der_idx = 1`) и row/key consistency checks.
- В `pwm-cli genesis-build` реализован выпуск только v3:
  - `validator_seeds_hex` удалён из output;
  - добавлен `validator_keys[*].enc_seed` с шифрованием через общий wallet crypto helper;
  - добавлен passphrase flow для genesis encryption: `--genesis-passphrase`, `PWM_GENESIS_PASSPHRASE`, TTY prompt.
- В `pwmd --genesis-file` добавлен passphrase flow для decryption:
  - `--genesis-passphrase`, `PWM_GENESIS_PASSPHRASE`;
  - TTY prompt fallback;
  - в non-tty режиме отсутствие passphrase завершает старт с понятной ошибкой.

## Тесты

- Позитив:
  - `pwm-cli`: `genesis_build_generates_rows_from_wallet_accounts` (v3 output + encrypted validator keys).
  - `pwmd`: `genesis_json_v3_roundtrip_encrypted_validator_key`.
- Негатив:
  - wrong passphrase (`genesis_json_v3_rejects_wrong_passphrase`);
  - malformed encrypted payload (`genesis_json_v3_rejects_malformed_payload`);
  - unsupported schema (`genesis_json_rejects_unsupported_schema_version`);
  - path mismatch (`genesis_json_v3_rejects_path_mismatch`).

## Обновлённые docs

- `docs/GENESIS_BLOCK.md` — операторский v3-only encrypted flow и troubleshooting.
- `docs/pwmd.md` — `--genesis-file` contract + passphrase requirement.
- `docs/pwm-cli.md` — `genesis-build` v3 output + passphrase inputs.
