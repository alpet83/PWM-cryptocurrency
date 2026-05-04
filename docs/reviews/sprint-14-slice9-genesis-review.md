# Sprint 14 — Slice 9 genesis review

## Verdict
`approve with changes`

## Scope
- Переименование `load_resume_index_domain` -> более понятное имя.
- Удаление `addr-derive` из актуального genesis workflow в `GENESIS_BLOCK.md`.
- Переход `--genesis-file` к лаконичному hex-формату с совместимостью legacy.
- Замена `docs/genesis_bundle_from_seed.ps1` на нативный генератор в `pwm-cli`.

## Contract
1. Переименовать helper в `detect_resume_der_index` + добавить docstring контракта.
2. Добавить в `pwm-cli` нативную команду `genesis-build` (wallet -> genesis JSON).
3. В `pwmd` сделать dual-loader: сначала новый hex-v2 формат, затем legacy byte-array формат.
4. Переписать `GENESIS_BLOCK.md`/`docs/pwmd.md` под новый путь; `addr-derive` и ps1 вывести из основного сценария.
5. Оставить backward compatibility для старых `--genesis-file`.

## Acceptance
- `pwm-cli genesis-build` генерирует рабочий bundle из wallet (в т.ч. encrypted path с passphrase).
- `pwmd --genesis-file` принимает новый v2 и legacy форматы.
- Документация синхронизирована с новым CLI workflow.
