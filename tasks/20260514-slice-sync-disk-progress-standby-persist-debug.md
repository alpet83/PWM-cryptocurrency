# Debug: sync disk progress / standby persist

Ticket: `tasks/20260514-slice-sync-disk-progress-standby-persist.json`
Agent: `pwm-debug`
Result: `PASS`
Verbosity-focus: `transport:sync` (CQDS/read-only only; no runtime log escalation, no source instrumentation)

## Diagnosis

CY attester can report memory sync as 100% while `tmp/state-cy-attester/pwm-data.json` is absent/stale because current sync progress and disk persistence measure different things:

- memory progress is `local_h` vs peer tip (`sync_prog_snap`), and `peer_tip_h == 0` is treated as complete;
- sync disk persistence calls `periodic_snap_save` only after a whole applied batch, using the final batch tip height;
- `periodic_snap_save` writes only when `autosnap_hit(height)` is true, i.e. `height > 0 && height % 100 == 0`;
- live/catch-up sync batches are capped at 32 blocks, so a batch can cross height 100 without ending at 100 (`32,64,96,128`), causing the sync checkpoint to be skipped while the proposer seal path still hits exact block 100.

## Code Evidence

- `crates/pwmd/src/snapshot/epoch.rs:9-10`: `SNAP_CHK_BLK_IV = 100`.
- `crates/pwmd/src/lifecycle.rs:31-35`: `AUTOSNAPSHOT_BLOCK_INTERVAL` aliases `SNAP_CHK_BLK_IV`; `autosnap_hit(h)` is `h > 0 && h % AUTOSNAPSHOT_BLOCK_INTERVAL == 0`.
- `crates/pwmd/src/lifecycle.rs:211-228`: `periodic_snap_save(...)` returns `None` unless `autosnap_hit(height)`; on hit it logs `autosnapshot checkpoint hit source=... interval=... height=...` and calls `backend.save_seal_persist(..., SealPersistMode::Periodic)`.
- `crates/pwmd/src/lifecycle.rs:491-516`: seal path precomputes rollback only for `autosnap_hit(now_h + 1)`, seals one block, then calls `periodic_snap_save(..., h, "seal")`; proposer naturally hits every exact mod-100 height.
- `crates/pwmd/src/transport/peer_session/sync_live.rs:859-882`: `apply_blk_batch` applies every block in `blocks`, then calls `periodic_snap_save(..., tip_h, "sync_apply")` once, after the batch, using only final `tip_h`.
- `crates/pwmd/src/transport/peer_session/sync_live.rs:17,276-285,894`: live block request/batch cap is `SYNC_BLK_REQ_CAP = 32`; `on_blk_batch` accepts up to `blk_cap.min(32)`, so final batch tips can skip checkpoint boundaries.
- `crates/pwmd/src/transport/peer_session/sync_live.rs:1053-1055,1191-1210`: catch-up chunks also use up to 32 rows and call `apply_blk_batch`; they have the same boundary-skip behavior.
- `crates/pwmd/src/transport/peer_session/sync_live.rs:60-72`: `sync_prog_snap(local_h, peer_tip_h)` returns `{ pct: 100, rem: 0 }` when `peer_tip_h == 0` or `local_h >= peer_tip_h`.
- `crates/pwmd/src/transport/peer_session/sync_live.rs:101-115`: progress log target is explicit `target: "pwmd::sync"` and logs `История синхронизирована на ...`.
- `crates/pwmd/src/transport/peer_session/sync_live.rs:987-993,1210-1217,1282-1289`: successful sync/catch-up logs use `target: "pwmd::peer"` for apply/catch-up status, while progress remains `pwmd::sync`.
- `crates/pwmd/src/snapshot/store.rs:89-99` and `crates/pwmd/src/snapshot/io.rs:882-885`: JsonFile `save_seal_persist` writes epoch tail to tip and rewrites checkpoint summary (`pwm-data.json`) via `json_file_seal_persist`.

## Edge Cases Confirmed

- `peer_tip_h == 0` means "unknown/genesis peer tip" in progress math but currently logs as 100%, which can be false confidence before disk state exists.
- A synced in-memory tip equal to peer tip is not evidence that a disk checkpoint happened; there is no tracked `last_snapshot_height` in product code (`last_snapshot_height` only appears in this ticket).
- Existing unit coverage (`sync_prog_snap_caps_tip`) explicitly expects `sync_prog_snap(0, 0) == 100%`, and `batch_cross_ckpt_writes_snap` only covers a batch ending exactly at height 100, not a batch crossing 100 and ending at 128.

## Recommended Fix Outline

1. Split memory progress from disk progress: add a tracked `last_snapshot_height` updated after successful snapshot load/save and include `mem_tip`, `peer_tip`, and `disk_tip/last_snapshot_height` in `pwmd::sync` logs.
2. Do not report complete sync progress for `peer_tip_h == 0` as a normal 100% completion; either suppress progress until a non-zero peer tip is known or log an "unknown peer tip" state.
3. For `SealRole::Standby` sync apply, persist before height 100: height 1 and every 10 blocks, with a named constant next to `AUTOSNAPSHOT_BLOCK_INTERVAL`; keep proposer seal path at mod-100.
4. In `apply_blk_batch`, detect checkpoint crossings inside the batch, not only the final tip, or run the standby checkpoint policy after each successfully applied block/boundary. Preserve rollback semantics around snapshot write failures.
5. Add regression tests for: `peer_tip_h == 0` progress, a sync batch crossing height 100 without ending at 100, and standby sync persistence before 100.

## Participation / Token Estimate

- `agent`: `pwm-debug`
- `result`: `PASS`
- `verbosity_focus`: `transport:sync`
- `instrumentation`: none; `reverted: yes` (no product-code instrumentation added)
- `repro`: code-level read-only diagnosis; no runtime repro executed; deterministic from code paths
- `artifacts`: `tasks/20260514-slice-sync-disk-progress-standby-persist-debug.md`
- `commands`: CQDS `cq_help`, `cq_project_ctl list_projects`, targeted `cq_files_ctl start_grep`; local file reads; no cargo/test commands
- `cleanup`: cleaned yes; no processes started, no product-code edits
- `token_usage`: `{ "source": "estimate", "input": 24000, "output": 4500, "total": 28500, "confidence": "medium" }`
