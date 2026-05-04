# Sprint 14 — Slice 10 Security Contract Review

## Decision
DROP backward compatibility now.

## Contract (v3-only)
- Genesis формат только `schema_version=3`.
- В genesis больше нет plaintext `validator_seeds_hex`.
- `validator_keys[*].enc_seed` хранит зашифрованный seed-контейнер (`kdf + aead`).
- Validator derivation path фиксирован: `m/1000000'/1'`.
- `pwmd --genesis-file` требует passphrase (flag/env/prompt) и расшифровывает ключи на старте.
- Старые/legacy форматы должны hard-fail с понятной ошибкой.

## CLI / Runtime requirements
- `pwm-cli genesis-build` генерирует только v3 и шифрует validator secret material.
- Источники passphrase для genesis encryption/decryption:
  - `--genesis-passphrase`
  - `PWM_GENESIS_PASSPHRASE`
  - интерактивный prompt (для TTY).

## Acceptance checklist
- [ ] pwmd reject missing/legacy/v2 schemas.
- [ ] pwmd reject malformed enc payload and wrong passphrase.
- [ ] pwmd validate fixed derivation path and row/key consistency.
- [ ] pwm-cli genesis-build emits v3 with encrypted validator keys only.
- [ ] docs updated for new operator flow.

## Verdict
`request changes` until v3-only encrypted contract is fully implemented.
