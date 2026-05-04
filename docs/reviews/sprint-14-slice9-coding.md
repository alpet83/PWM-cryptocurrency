# Sprint 14 — Slice 9 coding

- Переименован helper `load_resume_index_domain` -> `detect_resume_der_index` с docstring и обновлением call sites/tests.
- В `pwm-cli` добавлена команда `genesis-build` (`wallet -> genesis JSON v2`) с поддержкой encrypted wallet через `--wallet-passphrase` / `PWM_WALLET_PASSPHRASE`.
- В `pwmd` реализован dual-loader genesis: сначала v2 (hex schema), при неуспехе fallback в legacy schema.
- Добавлены проверки v2 на ошибки формата (`invalid hex`, `validator_seeds_hex length mismatch`), legacy compatibility сохранена.
- Обновлены docs: `GENESIS_BLOCK.md`, `pwmd.md`; `docs/genesis_bundle_from_seed.ps1` помечен как deprecated fallback.
