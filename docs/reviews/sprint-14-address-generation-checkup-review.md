# Sprint 14 — Address generation checkup (review seed)

Source: prior review findings + product request for wallet write semantics hardening.

## Scope

- `addr-bruteforce`: default wallet write semantics must be safe-add (append), not destructive replace.
- `addr-derive`: support complete wallet creation flow via explicit `--wallet-out` while preserving stateless mode without it.
- Wallet account lifecycle: removal only through dedicated command with clear guardrails.
- CLI UX/help consistency for wallet passphrase and write behavior.

## Seed findings to validate

1. Existing wallet path in `addr-bruteforce` was treated destructively by default.
2. No dedicated `wallet account remove` command for controlled deletion.
3. `addr-derive` lacked explicit wallet write path; flow was output-only.
4. Help/UX could mislead around when passphrase affects writes and when file mutation occurs.

## Expected acceptance checks

- `addr-bruteforce` appends account when wallet exists by default.
- Optional explicit destructive mode is opt-in only.
- `addr-derive --wallet-out` creates/appends wallet correctly; without `--wallet-out` keeps stateless stdout behavior.
- `wallet account remove` blocks last-account removal and applies deterministic active-account fallback.
