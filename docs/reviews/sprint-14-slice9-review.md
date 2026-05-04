# Sprint 14 — Slice 9 review

## Verdict
`request changes`

## Findings
1. **High**: в `load_genesis_bundle` fallback с v2 на legacy срабатывает слишком широко (на любую ошибку parse v2), что маскирует ошибки формата/версии.
2. **Medium**: в документации конфликт по derivation contract: `docs/pwmd.md` описывает `v2: m/0'/<der_idx>`, но в `GENESIS_BLOCK.md` местами остаётся формулировка про фиксированный `m/0'/0'` как общий путь.

## Required remediation
- Сделать строгую ветвизацию parse:
  - если `schema_version` указан и это v2, не падать в legacy fallback;
  - legacy fallback только для файлов без `schema_version`.
- Выровнять `docs/GENESIS_BLOCK.md` под фактический контракт:
  - v2: `m/0'/<der_idx>`;
  - legacy: `m/0'/0'`.
