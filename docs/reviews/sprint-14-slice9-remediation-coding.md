# Sprint 14 - Slice 9 remediation (coding)

## Scope
- `pwmd` genesis loader now uses strict schema branching in `load_genesis_bundle`.
- `docs/GENESIS_BLOCK.md` derivation contract aligned with runtime behavior.

## Implemented
1. Strict parser branching:
   - If `schema_version` is present:
     - `2` -> parse as v2 only (no legacy fallback).
     - any other value -> explicit unsupported schema error.
     - non-integer value -> explicit schema type error.
   - Legacy parse is used only when `schema_version` is absent.
2. Error quality:
   - invalid JSON payload/root shape is reported explicitly;
   - invalid v2 payload now reports `invalid v2 payload`.
3. Tests:
   - added regression test proving no fallback from explicit v2 to legacy;
   - added regression test for unsupported schema value;
   - updated existing v2 invalid hex assertion to strict v2 path.
4. Docs:
   - derivation contract clarified:
     - v2: `m/0'/<der_idx>`;
     - legacy: `m/0'/0'`.
