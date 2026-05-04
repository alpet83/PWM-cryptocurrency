# Sprint 14 — Slice 11 final review

## Verdict
`request changes`

## Blocker
- **High**: после decoupling возможна тихая потеря награды, если `validator acct` отсутствует в `funding.rows` (reward path no-op вместо явного fail/invariant).

## Additional findings
- **Medium**: в части путей сохраняется двойной источник (`rows` vs `funding.rows`), риск divergence.
- **Medium**: docs местами не согласованы с v4-only контрактом.

## Required remediation
1. Ввести явный инвариант для reward semantics (минимум: fail-fast если validator acct отсутствует в funding rows, либо deterministic auto-create и тесты).
2. Довести docs (`MVP-checklist`, `GENESIS_BLOCK`, др.) до строгой согласованности с v4.
