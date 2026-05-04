# Sprint 14 Slice28 Review

## Verdict
`approve with nits`

## Summary
The stale TUI `*` marker is removed from Owner rows, and selected-owner signing now derives from wallet seed + account metadata before considering any root signing-key fallback.

The original mismatch shape (`m/0/105053` selected while flattened root signing key is stale DO material) is covered by regression and no longer blocks the selected CY account.

## Nits
- Add a focused encrypted-v3 regression using decrypted payload seed with stale payload/root signing key, closer to `tmp/genesis.yaml`.
- Narrow `docs/pwm-tui.md` wording: missing master seed blocks non-root/multi-account derivation, while a verified legacy/root-key fallback may still work for compatible single-root cases.
