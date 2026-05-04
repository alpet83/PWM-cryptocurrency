# Sprint 1 Status Note (Kickoff)

**Sprint:** Sprint 1 - Two-Shard Runtime Foundation  
**Window:** 2026-04-24 .. 2026-05-08  
**Current phase:** kickoff

## Kickoff Summary

- Sprint checklist generated and published: `docs/reviews/sprint-1-checklist.md`.
- Baseline constraints locked for execution:
  - strict-upgrade from v0 account core,
  - protocol local `TRANSFER` gate by comparing `domain_hi(sender)` vs `domain_hi(receiver)` (cross-domain stays on explicit `EXPORT/IMPORT` track),
  - Phase1 recipient prefilter (reserve/witness/unknown-domain) as a separate layer from shard/process partitioning,
  - pinned devnet **process shard** map: Regulatory accounts -> shard A process, TNC accounts -> shard B process (not a `0x80` threshold),
  - no hidden UTXO pivot.
- Role split confirmed:
  - implementation: `pwm-coding`,
  - verification: `pwm-testing`,
  - coherence gate: `pwm-review`,
  - integration/go-no-go: orchestrator.

## Next Delegation Sequence

1. `pwm-coding` - implement Sprint 1 shard runtime foundation and demo path.
2. `pwm-testing` - run Sprint 1 regression/smoke and negative scenarios.
3. `pwm-review` - validate coherence against baseline docs and constraints.
4. Orchestrator - capture verdict (`ready | partial | blocked`) and update sprint carry-over.

## Escalation Rules Active

- Immediate stop on:
  - protocol constraint violations,
  - shard isolation break,
  - blocking review findings unresolved.
- Manual visual validation by owner is requested only if TUI/performance stability doubts appear.

## Open Items at Kickoff

- Final file-level task slicing for coding/testing pass.
- Confirmation of shard config naming and state path conventions for reproducible demo.
