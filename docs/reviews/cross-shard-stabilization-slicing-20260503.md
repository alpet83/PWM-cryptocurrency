# Cross-Shard Stabilization Slicing (MVP, 2026-05-03)

Scope: slicing/design artifact only for conveyor `pwm-coding -> pwm-testing -> pwm-review`.
No Rust product changes in this artifact.

## Fixed decisions (owner)

- Cross-shard import/export stays in MVP.
- Manual re-submit is not acceptable as recovery contract.
- MVP must provide automatic reimport/backfill after target cleanup/rollback/loss.
- Target replay must stay deterministic from genesis + blocks.
- Dedicated settlement/import-export chain is a future stage, not immediate MVP gate.

## Slice A — Reproduce and lock mismatch

### Goal
Find and lock first deterministic mismatch point (`first_bad_height`) and tx class.

### Coding scope
- Add narrow instrumentation around snapshot replay verification path with `PWM_SNAPSHOT_VERIFY_CHAIN=1`.
- Emit one concise structured line for first mismatch:
  - height,
  - block hash,
  - tx kind (`Export`/`Import`/other),
  - classification (`missing_provenance`, `state_root_divergence`, `manifest_summary_drift`, `other`).
- Add deterministic test fixture for minimal cross-shard scenario that reproduces mismatch before fix.

### Testing gate
- `cargo test -p pwmd snapshot_roaming -- --nocapture`
- Focused e2e scenario with `PWM_SNAPSHOT_VERIFY_CHAIN=1` records stable `first_bad_height`.

### Acceptance
- Reproduction fails before fix and is stable (no flaky height drift).
- Output includes first bad height and classification without verbose logs.

### Risk
- Over-logging can hide first root cause; keep single "first mismatch" event.

## Slice B — Deterministic target provenance in block path

### Goal
Stop replay-critical mutation of `State.exported_registry` outside blocks.

### Options and tradeoff
1. `MirroredExport` / `ExportProvenance` tx on target, then `Import`.
   - Pros: explicit chain fact separation.
   - Cons: extra tx ordering/state machine complexity in MVP.
2. `Import` with embedded provenance (preferred for MVP).
   - Pros: single deterministic state transition, simpler idempotency, smaller scope.
   - Cons: larger import payload and stricter validation envelope.

### Preferred MVP mechanism
- Keep handoff endpoint as transport-only pending material (non replay-critical side-state).
- `handoff_register` MUST NOT mutate `State.exported_registry`.
- `State.exported_registry` updates only during sealed block application of `Import` with embedded provenance.

### Testing gate
- Unit/integration tests for:
  - reject `Import` with malformed/missing provenance,
  - deterministic replay after restart without snapshot seeding hacks,
  - duplicate import idempotent rejection.

### Acceptance
- No replay-critical state mutation outside block path.
- Replay uses genesis + blocks only for import-critical provenance.

### Risk
- Backward compatibility of old handoff payloads; add explicit error contract and migration note.

## Slice C — Automatic reimport/backfill after cleanup

### Goal
Auto-recover missing cross-shard facts after local cleanup/rollback/loss.

### Coding scope
- Peer trust gate: allow backfill only from peers matching `network_id` and `genesis_hash`.
- Discovery scope: fetch only facts affecting local shard (`target_domain/domain_hi` relevant to node).
- Build idempotent local inclusion:
  - if missing, submit deterministic local `Import` (with provenance),
  - if already consumed, no balance mutation.
- Slice C contract: return tx-path validation outcome + counters (`discovered/imported/skipped/rejected/untrusted`) for each backfill batch.
- Full replay validation-after-backfill is deferred to Slice D tooling and the testing gate (not a Slice C runtime hard requirement).

### Testing gate
- Two-node recovery scenario:
  - target cleaned/rolled back,
  - reconnect to trusted source peer,
  - auto-backfill restores missing import exactly once.
- Negative: mismatch peer identity blocks backfill.

### Acceptance
- Recovery works without manual re-submit.
- Balance/history converges deterministically after auto-backfill.
- Operator sees explicit batch outcome counters; replay/deep integrity check is covered by Slice D/testing gate.

### Risk
- Duplicate remote facts; must stay idempotent by `export_id` and payload match.

## Slice D — Offline repair and safe rewrite

### Goal
Provide deterministic repair path to last reproducible height `H`.

### Coding scope
- Offline command/tooling:
  1. find last reproducible height by replay,
  2. truncate epoch files above `H`,
  3. rewrite manifest (`canonical_h`, `tip_hash`, epoch `last_h`),
  4. rewrite summary/checkpoint at `H`,
  5. run validation-after-write.
- Backup-first workflow and explicit failure codes.
- Реализация (2026-05-03): `crates/pwmd/src/bin/pwmd_snap_repair.rs` + `snapshot::repair_json_epochs` (offline-only, backup по умолчанию, режимы `--to-height` и `--auto-last-good`).

### Testing gate
- Corrupt-tail fixture:
  - detect first bad height,
  - repair to `H-1`,
  - successful reload and continued sealing.

### Acceptance
- No manual JSON editing required.
- Repair output is reproducible and validated.

### Risk
- Incorrect `tip_hash` rewrite can create hidden fork illusion; enforce post-repair verification.

## Slice E — Docs/RFC/checklist closeout + future note

### Goal
Align docs/spec/checklist with new MVP contract and keep settlement-chain as next-stage note.

### Coding scope
- Update review/doc artifacts to reflect deterministic import provenance and auto-backfill.
- Add checklist rows for stabilization slices and acceptance gates.
- Add RFC section stubs for:
  - deterministic target provenance in block path,
  - automatic backfill contract,
  - offline repair/crash-fast operator path,
  - future settlement-chain direction (non-blocking for MVP).

### Testing gate
- `pwm-review` verifies docs vs behavior and no contradiction with active MVP checklist.

### Acceptance
- Docs define one coherent recovery/determinism contract.
- Future settlement-chain is documented as optional next stage only.

### Slice E closeout (final sync, docs-only)

- RFC/docs/checklist synchronized with implemented A-D contract:
  - deterministic target path is `Import` with embedded provenance,
  - `handoff_register` is transport/pending-only and non-root for replay-critical state,
  - automatic backfill contract includes trust gate (`network_id` + `genesis_hash`) and explicit outcome counters,
  - offline operator contract uses `pwmd-snap-repair` (backup-first, validate-after-write),
  - settlement/import-export chain remains explicitly next-stage.
- No Rust product code required for Slice E.
- Commit baseline for implemented architecture shift tracked through Slice D closeout commit `669a41a`.

## Cross-slice risks

- Economic divergence risk if import is missing while export already spent.
- Side-state drift risk if any replay-critical state still mutates outside block.
- Recovery drift risk if backfill accepts untrusted peer facts.
- Operator risk if repair remains manual or partially documented.

## Global acceptance criteria for this shift

1. First mismatch is reproducible and classified (Slice A).
2. Target provenance is replay-deterministic via block tx path only (Slice B).
3. Cleanup/rollback recovers automatically through trusted, idempotent backfill (Slice C).
4. Offline repair can restore last reproducible canonical state safely (Slice D).
5. Checklist/RFC/review artifacts reflect implemented contract and test gates (Slice E).

## Post-slice protocol note (docs-only, not a new slice)

- **Source-side lock / conditional finalization** (export as escrow until import finality or timeout) is **out of scope** for the A–E stabilization track; it is recorded as a **future protocol direction** in `docs/rfc/9-crossdomain-roaming.md` Appendix A.5 and cross-linked from `WHITE_SPEC_v0.md` §7.4.
- Rationale: MVP has no policy layer for cross-shard **finality gating** on the source; target `IMPORT` is validated **unconditionally** within the implemented rules. Implementation should wait for a separate spec (proof interface, timeout/refund, compatibility).
