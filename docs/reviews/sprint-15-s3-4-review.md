# Sprint 15 S3.4 Review: one-window peer relay

## Verdict
`approve with nits`

## Что подтверждено
- One-window направление реализовано частично: source-side клиент может идти через свою ноду, а `pwmd` пытается довести handoff/import до target peer.
- Manual fallback сохранён.
- Genesis-fetch stub безопасен как status-only: он не заменяет локальный genesis молча.

## Final remediation result
1. Inbound/dev `NodeHello` больше не выдаёт provenance trust.
2. Provenance trust привязан к configured outbound seed context или test-only helper.
3. Forged/self-attested handoff registration отклоняется без target mutation.
4. Genesis guard покрывает `/v1/export-provenance`.
5. Relay failure оставляет intent `Exported` + `last_error`; `Relayed` сохраняется только после успешной доставки.

## Nits / follow-ups
- В документации явно держать правило: manual handoff register теперь требует trusted peer context.
- Отдельно от trust-boundary закрыть CLI source-only import preflight gap.

## Missing tests
- Forged handoff rejected.
- `/v1/export-provenance` blocked under genesis mismatch.
- Relay failure does not persist as successful `Relayed`.
- Two-node one-window happy path without target RPC knowledge by user.
