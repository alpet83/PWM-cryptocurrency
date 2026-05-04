# Sprint 14 — Slice 2 review

Source: independent `pwm-review` pass after coding/testing.

## Verdict (initial pass)

`block`

## Findings

1. **Blocker:** `wallet_created_at_unix_sec` (RFC 10) is dropped on `wallet account add/use` rewrite in v3 path (`crates/pwm-cli/src/wallet.rs`), because v3 model used for save does not preserve this field.
2. **Major risk (non-blocking if accepted):** `wallet account add` currently depends on root `master_seed_hex`, so encrypted v3 flow is not operator-complete (no passphrase/unlock path in command contract).
3. **Minor UX:** `wallet account list` output is less script-friendly than add/use (no explicit status line).

## Recommendation

- First unblock by implementing metadata-preserving v3 rewrite (`wallet_created_at_unix_sec` and future-safe preservation strategy).
- Then decide command contract for encrypted v3 `account add`: either passphrase-based unlock in command or explicit documented restriction to plaintext-only mode.

## Remediation re-review (same slice)

Source: second independent `pwm-review` pass after blocker fixes.

### Updated verdict

`approve with minor`

### Closure notes

- Blocker closed: v3 rewrite now preserves `wallet_created_at_unix_sec` and unknown top-level keys.
- Encrypted `wallet account add` path implemented via passphrase unlock (`--wallet-passphrase`).
- Remaining minor UX note: empty passphrase may surface decrypt-style error text instead of early explicit validation.
