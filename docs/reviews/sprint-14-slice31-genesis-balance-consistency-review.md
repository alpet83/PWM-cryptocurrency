# Sprint 14 Slice31 Genesis Balance Consistency Review

## Verdict
`request changes`

## Key Findings
- Both shards load full genesis funding locally; this is not a simple "DO did not import genesis" bug.
- Balance/API views are local-state views; operators can misread foreign-account visibility as authoritative remote truth.
- Source `EXPORT` can be accepted before target shard is ready to import/credit for a specific recipient, creating stuck-funds UX and perceived inconsistency.

## Risks
- **High:** source export without strict target-readiness preflight.
- **Medium:** ambiguous foreign-balance visibility semantics.
- **Medium:** limited operator recovery ergonomics for expired intents.

## Near-Term Recommendations
1. Add explicit source-side preflight against target readiness before export.
2. Mark/hide foreign balances as local-view-only in UI/API.
3. Enforce operational guardrails: same genesis bundle/hash across shards, surfaced in status.
4. Strengthen runbook for failed/expired roaming intents.

## Long-Term Direction
- Move toward proof-based / two-phase cross-shard settlement with authoritative home-shard semantics and explicit state labels (`local_state_balance`, `authoritative_home_balance`, `spendable_on_this_shard`).
