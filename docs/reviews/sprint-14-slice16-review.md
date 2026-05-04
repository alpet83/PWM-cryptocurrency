# Sprint 14 — Slice 16 review (independent)

## Verdict
`request changes`

## Findings
1. **Medium**: inconsistency в `flow/recent` taxonomy — transition в `relayed` пишет `kind=roaming_status:export`.
2. **Medium**: docs route inventory неполный (`/v1/roaming-intents/:id/finalize` не отражён в верхнем списке), RFC не синхронизирован.
3. **Low**: не хватает edge-тестов для finalize/retry/error semantics.

## Required remediation
- Выровнять `kind` по фактическому transition (`relayed`).
- Обновить `docs/pwmd.md` и `docs/rfc/9-crossdomain-roaming.md`.
- Добавить тесты на finalize + retry после 500 и terminal statuses.
