# Sprint 15 S3.11 Review

## Verdict
`approve with nits`

## Findings
- Contract intent is satisfied: foreign account authoritative values are peer-backed and explicit unknown-state is surfaced.
- Trust boundary is preserved: foreign authoritative cache merges only from trusted stateful sessions.
- TUI now reflects unknown foreign state as `???` and no longer implies false certainty.

## Nits
- Add dedicated API tests for `home_lookup_status=ok/not_found/unavailable` transitions.
- Add one e2e test with two shards where peer is dropped mid-run and TUI transitions from known to `???`.
