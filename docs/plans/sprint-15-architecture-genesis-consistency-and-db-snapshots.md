# Sprint 15 Plan: Genesis Consistency and Optional DB Snapshots

## Goals
- Eliminate ambiguous foreign-balance semantics across shards.
- Make cross-shard export/import readiness explicit and deterministic.
- Add optional snapshot backend in DB while keeping JSON as fallback.

## Workstreams

### 1) Cross-Shard Balance Semantics
- Define clear API fields:
  - `local_state_balance`
  - `authoritative_home_balance`
  - `spendable_on_this_shard`
- Decide default UI behavior for foreign addresses:
  - hide by default, or
  - show with explicit `local_view_only` marker.
- Add source-side preflight policy before export:
  - target recipient initialized/readiness proof required.

### 2) Genesis/Bootstrap Consistency Guardrails
- Enforce effective genesis hash visibility in `/v1/status`.
- Add startup/runtime guardrails for shard join consistency.
- Document operator runbook for mismatch and recovery.

### 3) Snapshot Storage Backend (Optional DB)
- Introduce backend abstraction:
  - `SnapshotStore::JsonFile`
  - `SnapshotStore::Db` (optional)
- Keep deterministic serialization contract for replay validation.
- Add migration strategy:
  - read old JSON,
  - write selected backend,
  - keep rollback path.

## Deliverables
- Architecture RFC with selected balance semantics.
- Protocol/UX contract for cross-shard readiness and failure recovery.
- Technical design for DB snapshot backend with compatibility matrix.
- Incremental implementation slices with tests and rollback checkpoints.

## Risks
- Hidden coupling between shard-local UX and protocol truth.
- Operational complexity if multiple balance notions are exposed without strict labels.
- DB backend introducing non-determinism if serialization is not normalized.

## Exit Criteria
- No ambiguous foreign-balance display in default UX.
- Cross-shard flow has explicit readiness checks and deterministic failure handling.
- Snapshot backend switchable (JSON/DB) with replay-consistent behavior.
