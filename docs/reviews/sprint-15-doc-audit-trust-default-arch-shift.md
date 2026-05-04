# Sprint 15 — doc audit: trust-default snapshot architecture shift

## Intro

Post-change `JsonFile` startup no longer treats a full genesis-to-tip replay as the normal path. The default loader reads the summary `pwm-data.json`, loads only the recent block tail from `epochs/`, and validates it through `validate_snapshot_trusted`: genesis identity, summary/manifest agreement, manifest `tip_hash`, tail linkage, PoA headers, and persisted state root at the tip.

Full chain replay remains available for audit/recovery through `--snapshot-verify-chain` or truthy `PWM_SNAPSHOT_VERIFY_CHAIN`. If the summary checkpoint lags the manifest `canonical_h`, the loader forces full verification even in default mode. ClickHouse remains a full-replay path; `SnapshotLoadOpts` does not weaken CH validation.

## Stale/misleading

- `docs/pwmd.md` — previously described snapshot load as always `load_snapshot + validate_snapshot` with full replay. Needs explicit dual-mode text and the `SNAP_CHK_BLK_IV` checkpoint wording.
- `docs/reviews/sprint-15-arch-trust-checkpoint-rescan-review.md` — historical pre-change review. Keep as design input, but add a banner pointing here and to `sprint-15-snapshot-trust-boundary-review.md`.
- `docs/reviews/sprint-15-slice-6-bench.md` — benchmark wording still says JsonFile load is full replay only. Update to distinguish current normal trust-default load from audit replay and CH.
- `tasks/20260503-*` — the architecture review task describes the old behavior correctly for its date, but follow-up references should mark it as pre-change.
- `tasks/20260430-*` — older snapshot benchmark/checkpoint tasks may still frame epochs/checkpoints as not yet used for tail startup. Treat as historical unless a task was already amended.

## Needs minor refresh

- `README.md` — add a short operator-facing paragraph and link to the new storage guide.
- `docs/MVP-checklist.md` — storage row can be expanded later from “JSON snapshot `blocks + state`” to JsonFile summary + epochs + trust/audit modes.
- `docs/CODEBASE_INDEX.md` — generated index still points to `snapshot.rs` as a monolithic doc concept and should be regenerated after the next code index/docs pass.
- `docs/reviews/sprint-15-runtime-persist-P0-review.md` — still accurate for P0 API-save split, but its “not in this PR” line is now historical after trust-default startup landed.
- `docs/reviews/sprint-15-s3-15-1-review.md` — “long load” note remains useful as pre-optimization context; add/cross-reference only if that review is reused in planning.
- `docs/runbook-store-snapshots.md` — add one sentence: JsonFile now has trust-default/tail load; ClickHouse still full replay.

## Already aligned

- `crates/pwmd/src/snapshot/io.rs` comments and implementation: `SnapshotLoadOpts`, `validate_snapshot_trusted`, forced full verify on summary/manifest lag.
- `crates/pwmd/src/snapshot/store.rs` comments: CH load is full replay and ignores JsonFile trust-load weakening.
- `crates/pwmd/src/snapshot/epoch.rs` comments: `SNAP_CHK_BLK_IV = 100` vs `EPOCH_SPAN = 1000` distinction is explicit.
- `tasks/20260504-s15-snapshot-trust-default-api-save-split.json` brief/acceptance already names the target architecture.

## Gaps

- Add an end-user guide for node storage and snapshot load modes: `docs/guide-node-storage-and-snapshot.md`.
- Consider a later generated docs pass for `docs/CODEBASE_INDEX.md`.
- Consider a small MVP-checklist update once the Sprint 15 doc sweep is merged.
- Keep the historical architecture review linked instead of rewriting it, so pre-change reasoning remains auditable.

---

## Participation / token estimate

- `agent`: `pwm-coding`
- `result`: `PASS`
- `token_usage`: estimate, total ~1800, confidence low
