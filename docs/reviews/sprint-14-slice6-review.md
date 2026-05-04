# Sprint 14 — Slice 6 review

Source: independent `pwm-review` pass after coding/testing.

## Verdict

`block`

## Findings

1. **High (blocking):** create-path writes (`wallet init`, `wallet import-seed`, `addr-bruteforce`) use merge-based save logic and may inherit stale/unknown fields from existing destination file, violating clean-create semantics and risking sensitive field carry-over.
2. **Minor:** naming split remains (`id_pretty` user-visible vs internal `account_id_human`), acceptable with docs clarity.
3. **Coverage gap:** no explicit test ensuring create-path on existing file does not preserve unrelated legacy fields.

## Required follow-up

- Separate save strategy for create-path: strict overwrite with canonical v3 payload (no merge with old file).
- Keep merge-preserve logic only for update-paths where it is intended (account add/use).
- Add regression tests for create-path overwrite behavior on pre-existing wallet files.
