# 2026-05-08 Divergence Guard HOTFIX (same-shard sync v1)

## Scope implemented

- Added divergence detection only for sync-enabled full_v1 same-shard path in `SyncTipAnnounce` handling:
  - trigger condition: `peer_tip_height == local_tip_height` and `peer_tip_hash != local_tip_hash`.
- Added explicit disconnect reason: `sync_tip_divergence`.
- Added per-peer reconnect cooldown marker with default floor `60_000ms`.
- Added dedicated metric counter: `sync_tip_divergence_disconnect_total`.
- Preserved existing behavior for:
  - different heights,
  - profile mismatch / legacy_observe path,
  - non-sync modes.

## Operator-facing behavior

- On divergence, node logs `warn` with local/peer heights and hashes plus cooldown.
- Session close reason is recorded as `sync_tip_divergence` and flows through reason-labeled close metrics (`peer_close_by_reason`).

## Local validation

- `cargo fmt`
- `cargo test -p pwmd tip_divergence`
- `cargo check -p pwmd`
- `python scripts/check_rust_fn_name_segments.py ...` for touched Rust files

## 2026-05-08 follow-up: inbound symmetry + settled anchor

- Cooldown mapping is now symmetric for divergence disconnects:
  - outbound path still uses direct `seed_key`;
  - inbound path resolves `seed_key` by `node_id` via `transport.seed_peers[*].last_node_id`.
- Cooldown floor is unchanged: `max(reconnect_runaway_cooldown_ms, 60_000ms)`.
- `SyncTipAnnounce` now carries `finalized_hash` (optional) paired with `finalized_height`.
- Divergence rule now prefers settled anchor when present:
  - if `lag == 0` and `finalized_hash` is present and local node has that `finalized_height`, compare anchor hashes first;
  - anchor mismatch => `sync_tip_divergence` disconnect;
  - anchor match => do not disconnect on mutable tip hash mismatch.
- Fallback when settled anchor is unavailable (`finalized_hash` missing / anchor not resolvable locally):
  - keep legacy safe behavior: only same-height tip-hash mismatch can trigger divergence disconnect.

## 2026-05-08 follow-up: peer compatibility + wire stabilization

- Wire `u128` fields used by peer payloads (`CrossShardFacts.amount`, `AccountViews.balance_pwm`) now emit canonical hex strings (`0x...`) to avoid `serde_json` unsupported large-number decoding paths.
- Wire decode remains backward compatible:
  - accepts canonical `0x...`;
  - accepts legacy decimal strings;
  - accepts legacy non-negative numeric values when they fit JSON integer support.
- Handshake classification now applies explicit guard wrappers early:
  - same-shard (`domain_hi` equals local): requires strict `cluster_id` equality to local cluster id;
  - same-shard/inter-shard with malformed `sync_profile` are rejected with explicit reasons (`*_sync_profile_incompatible`).
- Sync routing now uses explicit lane reasons:
  - inter-shard sync frames are dropped under `inter_shard_sync_forbidden`;
  - same-shard non-full_v1 frames are dropped under `same_shard_profile_mismatch`.
- Inbound `HelloAck` rejection now propagates concrete reason text instead of generic `hello_rejected`.
