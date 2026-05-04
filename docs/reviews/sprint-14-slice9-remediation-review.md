# Sprint 14 — Slice 9 remediation review

## Verdict
**approve**

## Closed blockers
- `pwmd` loader теперь строго ветвится по `schema_version`: без широкого silent fallback v2 -> legacy.
- `GENESIS_BLOCK.md` выровнен с фактическим контрактом derivation:
  - v2: `m/0'/<der_idx>`
  - legacy: `m/0'/0'`
- Регрессий в `genesis-build` flow по тестовому покрытию не выявлено.

## Notes
- Покрытие включает таргетные тесты `pwmd genesis_json_*` и `pwm-cli genesis_build_*`.
- Критичных замечаний по remediation не обнаружено.
