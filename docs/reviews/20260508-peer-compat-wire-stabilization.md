# 2026-05-08 Peer compatibility + wire stabilization

## Scope

- Ticket: `tasks/20260508-peer-compat-and-wire-stabilization.json`.
- Focus:
  - stabilize wire decode/encode for payload values above `u64`;
  - harden handshake compatibility matrix in early classification path;
  - keep legacy observe behavior where protocol path allows non-sync operation.

## Implemented changes

### 1) Wire stabilization for `u128` fields

- `CrossShardFacts.amount` and `AccountViews.balance_pwm` now use canonical wire serialization as hex string (`0x...`).
- Decoder keeps compatibility:
  - accepts canonical hex (`0x...`);
  - accepts legacy decimal string;
  - accepts legacy non-negative integer JSON where parser supports it.
- Added/updated wire decode tests for:
  - decode from legacy decimal and canonical hex;
  - canonical hex emission on encode path;
  - negative values rejection behavior unchanged.

### 2) Compatibility matrix hardening in handshake/classification

- `process_incoming_peer_hello` now classifies same-shard vs inter-shard immediately after base hello validation and applies class-specific guards.
- Same-shard strictness:
  - reject when peer `cluster_id` differs from local cluster id (`same_shard_cluster_id_mismatch`).
- Capability negotiation checks:
  - reject malformed/incompatible sync profile for same-shard (`same_shard_sync_profile_incompatible`);
  - reject malformed/incompatible sync profile for inter-shard (`inter_shard_sync_profile_incompatible`).
- Inbound path now returns explicit reject reasons in `HelloAck.reason` (instead of generic `hello_rejected`).

### 3) Early guard routing in steady sync lane

- Inter-shard sync frames are explicitly classified and dropped as `inter_shard_sync_forbidden`.
- Same-shard non-full-v1 sync frames are explicitly dropped as `same_shard_profile_mismatch`.
- Added test coverage for inter-shard sync-tx drop routing reason.

## Residual risks / follow-ups

- Legacy numeric compatibility still depends on JSON parser integer limits; canonical interop target is hex string.
- Same-shard strict `cluster_id` checks may reject historically misconfigured peers; operationally desirable, but rollout should monitor reject counters.
- This slice rejects malformed `sync_profile`, but does not yet add a wider policy matrix for `protocol_version`/`tx_features` beyond existing mandatory-field checks.
