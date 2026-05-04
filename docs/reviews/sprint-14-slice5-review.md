# Sprint 14 — Slice 5 review

Source: independent `pwm-review` pass after coding/testing.

## Verdict

`block`

## Findings

1. **High (blocking):** `load_wallet_yaml` now rewrites v2 wallet files to v3 on read path, introducing write side-effects for read-style commands (`wallet show`, `backup/recover` internals, `tx-send --wallet` load path). This changes operational contract and may fail on read-only permissions.
2. **Medium:** migration write path is non-atomic (`fs::write`), so interruption risks partial file state.
3. **Coverage gap:** missing explicit tests for read-only permission scenarios and for commands expected to be read-only.

## Required follow-up

- Decide contract: either explicit upgrade command / opt-in migration, or keep auto-migrate but document and test read-side write behavior.
- Add read-only permission tests for key wallet commands.
- Consider atomic write strategy for migration persistence.
