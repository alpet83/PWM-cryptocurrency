# Debug report: attester sync stalls at 4%

Ticket: `tasks/20260517-attester-sync-stall-at-4pct.json`  
Agent: `pwm-debug`  
Verbosity focus: `transport:sync`  
Artifact: `tasks/20260517-attester-sync-stall-at-4pct-debug.md`

## Scope and repro status

I did not start a new live `pwmd` process. The analysis is code-level plus the operator terminal snapshots already running in Cursor.

Observed terminal evidence from `cy-cluster-attester.ps1`:

- Clean/empty attester state starts from genesis and catches up in 32-block batches: `Sync progress 0%`, then checkpoints `1..32`, `33..64`, ... up to `513..544`.
- The run is interrupted at about 4%: `Sync progress 4% rem=11508 goal=12020 mem=512/12020 disk=512/12020`, then `standby sync checkpoint range=513..544`.
- Restart from the same state loads the partial snapshot: `snapshot startup load ok ... tip_h=544 canonical_h=544`, then reports `Sync progress 4% rem=11920 goal=12464 mem=544/12464 disk=544/12464` and no further console progress in the captured terminal.

This makes the stall reproducible enough for diagnosis around partial local snapshot height `544`, large proposer tip `~12k`, and same-shard sync/CUP transition.

## Most likely root cause

The strongest code-level suspect is a CUP range/epoch interaction after the attester restarts from `tip_h=544`. In `sync_live::on_tip`, a large lag triggers catch-up when `lag >= SYNC_CUP_LAG_MIN` before normal header/block sync. `maybe_start_cup` chooses `from_h = local_h + 1` and `to_h = from_h + min(lag, SYNC_CUP_WIN_CAP) - 1`. At `local_h=544`, that becomes approximately `545..4640`. But `on_cup_req` rejects any catch-up request whose start and end heights are not in the same epoch; epoch span is `1000`, so `545..4640` crosses epochs. The responder should NACK with `catchup_epoch`; the requester increments `cup_try/live_stall`, clears CUP, and retries later. A restart resets in-memory `cup_try`, so the node repeats the same invalid cross-epoch CUP window from the persisted `544` snapshot instead of quickly falling back to live hdr/blk sync.

Relevant code areas:

- `crates/pwmd/src/transport/peer_session/sync_live.rs`: `SYNC_CUP_LAG_MIN`, `SYNC_CUP_WIN_CAP`, `maybe_start_cup`, `send_cup_req`, `on_cup_req`, `on_nack`.
- `crates/pwmd/src/snapshot/epoch.rs`: `EPOCH_SPAN = 1000`, `epoch_idx`.
- `crates/pwmd/src/transport/handshake_state.rs`: `SyncPeerState` fields `cup_active`, `cup_try`, `cup_next_ms`, `live_stall`, `wait_hdr_from`, `wait_blk`, `in_hdr`, `in_blk`.

## Plausible root causes to distinguish

1. **CUP window crosses epoch boundary**  
   Expected signature: proposer peer log has `peer sync nack ... reason=catchup_epoch` or requester has `peer sync catchup aborted by nack ... reason=catchup_epoch`; `sync_cup_start_total` and `sync_cup_fail_total` rise, `sync_cup_chunk_total` stays flat. This matches the `544` restart height and `EPOCH_SPAN=1000`.

2. **CUP disk serve gap or epoch read failure below RAM tail**  
   Expected signature: `reason=catchup_gap` on NACK, or `peer sync catchup fail ... reason=chunk_*` on the requester. This would mean epoch JSONL/manifest on proposer cannot serve the range starting at `545`, even if the range is epoch-local.

3. **Live hdr/blk request stuck behind in-flight caps**  
   Code caps are `SYNC_INF_CAP=8`, `wait_hdr_from`, `wait_blk`, `in_hdr`, `in_blk`. NACK decrements in-flight and requeues blocks, but lost responses or a session that keeps `cup_active` true can make `ask_hdr`/`ask_blk` return early. Current `/v1/dev/peers` does not expose these per-peer fields, so logs must infer it from request/response counters.

4. **Fork or hash mismatch loop after partial local snapshot**  
   Expected signature: `peer sync headers rejected ... reason=continuity_start|continuity_break`, `peer sync divergence disconnect ...`, or `sync_fork_conflict_total` rising. This would point to local `tip_h=544` not matching proposer canonical height 544.

5. **Apply failure after valid blocks arrive**  
   Expected signature: `peer sync apply failed node_id=... reason=prev_hash_mismatch|state_root_mismatch|prod_idx_mismatch|bad_sig|tx_root_mismatch|tx_invalid:*`, with `sync_apply_fail_total` rising and `sync_apply_ok_total` flat after 544.

6. **Peer session churn or cluster frame closes the pipe before sync drains**  
   `steady_session` sends heartbeat, cross-shard facts, account views, sync tx batch, sync tip, then cluster proposal/attest frames. If cluster write/read errors close the session around the same point, sync requests may not complete. Expected signature: `last_peer_error`, `last_session_close_reason`, `peer_close_by_reason`, `wire_*_failed`, or cluster logs near the stall.

7. **Init/readiness or snapshot load gate**  
   Less likely in the captured run because `/v1/status` should show `phase=ready` after `snapshot startup load ok`, but still capture `phase`, `ready`, `snapshot_error`, and `last_readiness_reject_*`.

## What to capture

Capture both sides, over the same wall-clock window, starting before attester launch and continuing at least 60s after the 4% line.

Peer logs:

- `logs/**/pwmd-peer-cy-attester-*.log`
- `logs/**/pwmd-peer-cy-proposer-*.log`

Important log lines / patterns:

- `peer sync mode negotiated ... mode=...`
- `peer sync frame ignored ... reason=same_shard_profile_mismatch`
- `peer sync catchup start node_id=... epoch_id=... range=...`
- `peer sync catchup fail node_id=... reason=chunk_bounds|chunk_order|chunk_link|chunk_empty|chunk_hash|chunk_range|chunk_tail|chunk_apply|done_mismatch`
- `peer sync catchup aborted by nack node_id=... reason=... retry=... next_ms=...`
- `peer sync nack node_id=... reason=headers_range|headers_limit|blocks_range|blocks_hash|catchup_range|catchup_epoch|catchup_gap`
- `peer sync catchup progress ... next_height=...`
- `peer sync catchup finish ... last_height=...`
- `peer sync headers rejected ... reason=continuity_start|continuity_break`
- `peer sync divergence disconnect ... local_height=... peer_height=...`
- `peer sync apply ok node_id=... blocks=...`
- `peer sync apply failed node_id=... reason=...`
- `wire_heartbeat_*_failed`, `sync_tip_write_failed`, `cluster_propose_write_failed`, `cluster_attest_write_failed`, `heartbeat_read_failed`

Console/main log lines:

- `Sync progress ... rem=... goal=... mem=... disk=...`
- `standby sync checkpoint range=... flush_iv=...`
- `snapshot startup load ok ... tip_h=... canonical_h=...`
- `snapshot startup: no snapshot row or file`
- `sealed height=...` on proposer
- `seal_suppressed_by_cluster ...` around the same timestamps

HTTP snapshots:

- `GET http://127.0.0.1:3030/v1/head` and `GET http://127.0.0.1:3031/v1/head` to compare proposer vs attester `height`/`tip`.
- `GET http://127.0.0.1:3030/v1/status` and `GET http://127.0.0.1:3031/v1/status` for:
  - `phase`, `ready`, `snapshot_file`, `snapshot_error`
  - `node_id`, `cluster_id`, `cluster_domain_hi`, `deployment_profile`, `seal_role`
  - `lease_state`, `seal_gate_allowed`, `lease_last_tip`, `lease_last_reason`
  - `peer_seed_count`, `peer_listen`, `live_peer_count`, `trusted_relay_peer_count`
  - `peer_session_connected_total`, `peer_session_retrying_total`, `peer_session_disconnected_total`, `peer_session_trusted_total`, `peer_session_untrusted_total`
  - `next_seed_due_ms`, `last_peer_error`, `peer_error_at_ms`
  - `genesis_guard`, `genesis_mismatch_total`, `last_readiness_reject_code`
- `GET http://127.0.0.1:3030/v1/dev/peers` and `GET http://127.0.0.1:3031/v1/dev/peers` for the actual `transport` snapshot:
  - `transport.sync_v1_msg_seen_total`, `transport.sync_v1_msg_drop_total`, `transport.sync_v1_msg_drop_reason_total`
  - `transport.sync_tip_seen_total`, `transport.sync_tip_divergence_disconnect_total`
  - `transport.sync_hdr_req_total`, `transport.sync_hdr_resp_total`
  - `transport.sync_blk_req_total`, `transport.sync_blk_resp_total`
  - `transport.sync_apply_ok_total`, `transport.sync_apply_fail_total`, `transport.sync_fork_conflict_total`
  - `transport.sync_cup_start_total`, `transport.sync_cup_chunk_total`, `transport.sync_cup_done_total`, `transport.sync_cup_fail_total`, `transport.sync_cup_drop_total`, `transport.sync_cup_fail_reason_total`
  - `transport.last_session_close_reason`, `transport.last_reconnect_reason`, `transport.counters.peer_close_by_reason`, `transport.counters.reconnect_decision_by_reason`

Note: `/v1/status` currently exposes only selected peer/session health fields. The full `TransportSnapshot` is returned by `/v1/dev/peers` as `transport`.

## Ordered triage checklist on the same PS1 launchers

1. Stop both PS1-launched nodes cleanly. Keep a copy of the latest `logs/**/pwmd-peer-cy-*.log` and console output.
2. Record proposer head: `GET 127.0.0.1:3030/v1/head` if proposer is still running. If not, restart only `./cy-cluster-proposer.ps1`, wait for `sealed height=...`, then record `/v1/head`.
3. Dirty-state repro: do not delete `tmp/state-cy-attester`. Start `./cy-cluster-attester.ps1`. Confirm the startup line shows `snapshot startup load ok ... tip_h=544` or whatever the partial height is.
4. Immediately capture `attester /v1/status`, `attester /v1/dev/peers`, `proposer /v1/status`, `proposer /v1/dev/peers`.
5. Wait 60s after the first `Sync progress 4%` line. Capture the same four HTTP snapshots again. If `sync_cup_start_total` rises but `sync_cup_chunk_total`/`sync_apply_ok_total` do not, inspect peer logs for `catchup_epoch`, `catchup_gap`, or `nack`.
6. If dirty-state stalls, clean only attester state: move `tmp/state-cy-attester` aside, recreate it empty, then run the same `./cy-cluster-attester.ps1`. Do not change proposer state. Compare whether the node passes `544`.
7. If clean-state passes `544` but dirty-state does not, focus on local partial snapshot compatibility and persisted `tip_hash` at 544 vs proposer block 544.
8. If both clean and dirty stall around the first epoch boundary, focus on CUP epoch-window clamping and proposer epoch disk serving.
9. If peer sessions churn, compare `last_peer_error`, `last_session_close_reason`, `peer_close_by_reason`, and cluster `seal_suppressed_by_cluster` timestamps.

## Fix directions for `pwm-coding`

- Clamp CUP request windows to a single epoch in `sync_live::maybe_start_cup` / `send_cup_req`: calculate the epoch range for `from_h` and set `to_h <= epoch_range(epoch_idx(from_h)).last_h`. Add a regression starting at `local_h=544`, peer tip `> 4096`, asserting the first CUP request ends at `1000`, not `4640`.
- After CUP NACKs with `catchup_epoch`, either force immediate live hdr/blk fallback or reduce the next CUP range to the current epoch. Avoid relying on multiple in-memory `cup_try` retries because restarts reset that state.
- Expose a small per-peer sync debug snapshot under `/v1/dev/peers` or a dev-only endpoint: `tip_h`, `wait_hdr_from`, `wait_hdr_lim`, `pend_blk_len`, `wait_blk_len`, `in_hdr`, `in_blk`, `live_stall`, `cup_active`, `cup_epoch`, `cup_from`, `cup_to`, `cup_next_h`, `cup_try`, `cup_next_ms`. This would turn the current log-only diagnosis into a quick status check.
- Add a specific counter/bucket for CUP NACK reason instead of collapsing all CUP NACKs into `sync_cup_fail_reason_total["nack"]`; preserve the wire reason such as `nack:catchup_epoch`.
- Add tests for epoch-backed `on_cup_req` below RAM tail where the requested range is valid and epoch-local, plus a negative test for cross-epoch request returning `catchup_epoch`.
- Keep cluster proposal/attestation send failures from obscuring sync diagnostics: ensure `last_peer_error` and peer close reasons distinguish `cluster_*_write_failed` from sync NACK/apply failures.

## Debug instrumentation

No temporary Rust instrumentation was added.

- Instrumentation files/hunks: none.
- Reverted: yes.

## Commands and tools run

- Read debug prompt and CQDS skill.
- Used CQDS `cq_help` and `cq_files_ctl start_grep` with `project_id=5` / host FS search for code mapping.
- Read code: `sync_live.rs`, `steady_session.rs`, `inbound.rs`, `peer_session/mod.rs`, `handshake_state.rs`, `metrics.rs`, `handlers_status.rs`, `handlers_peer.rs`, `api/types.rs`, `snapshot/epoch.rs`, CY PS1 launchers.
- Read existing terminal snapshots for proposer/attester. Did not spawn new live `pwmd`.

## Orchestrator ticket record

```json
{
  "agent": "pwm-debug",
  "result": "PARTIAL",
  "verbosity_focus": "transport:sync",
  "instrumentation": {
    "files": [],
    "hunks": 0,
    "reverted": "yes"
  },
  "repro": {
    "commands": [
      "./cy-cluster-proposer.ps1",
      "./cy-cluster-attester.ps1"
    ],
    "deterministic": "not rerun by agent; terminal evidence shows repeat at persisted tip_h=544"
  },
  "artifacts": [
    "tasks/20260517-attester-sync-stall-at-4pct-debug.md"
  ],
  "commands": [
    "CQDS grep/read mapping: PASS",
    "live pwmd run: NOT_RUN"
  ],
  "cleanup": {
    "cleaned": "yes",
    "killed": []
  },
  "token_usage": {
    "source": "estimate",
    "input": 26000,
    "output": 6200,
    "total": 32200,
    "confidence": "medium"
  }
}
```
