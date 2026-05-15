# RFC 0015: Same-Shard Sync v1 Contract

**Status:** Frozen (Sprint V2-8 Slice 0); **Amended 2026-05-08** — §13 (cluster L2 storm guard); **Amended 2026-05-16** — §14 (informative: pwmd JsonFile serving below RAM tail)  
**Version:** 0.1+cluster-storm-guard+jsonfile-serve-note  
**Depends on:**

- `docs/WHITE_SPEC_v0.md`
- `docs/adr/0001-consensus-and-node-stack.md`
- `docs/rfc/8-shard-runtime-identity-and-peering.md`

---

## 1. Abstract

This RFC freezes the minimal normative contract for **same-shard sync v1** before implementation slices.

Scope v1 includes:

- same-shard mempool gossip,
- live chain sync (steady-state head tracking),
- epoch catch-up fallback when live sync cannot close the gap safely.

This document is a protocol/runtime contract for Sprint V2-8 Slice 0 and does not require immediate product-code changes by itself.
Any behavior beyond this contract MUST be treated as deferred until a follow-up RFC revision.

---

## 2. Goals and Non-Goals

### 2.1 Goals

1. Define a wire-level message taxonomy with mandatory minimal fields.
2. Define capability negotiation and version compatibility behavior.
3. Define a deterministic minimal fork-choice for v1 with explicit deferrals.
4. Define anti-DoS limits that bound memory, CPU, and bandwidth risk.
5. Define compatibility behavior for legacy peers.
6. Provide acceptance criteria for implementation slices 1..5.

### 2.2 Non-Goals (v1)

- Cross-shard sync protocol.
- Fast-state/snapshot sync protocol internals.
- Finalized long-range consensus redesign.
- Full p2p reputation system design.

---

## 3. Terminology

- **Local shard**: runtime shard bound by local `domain_hi`.
- **Same-shard peer**: peer with the same `domain_hi` as local runtime (per RFC 0008 identity contract).
- **Live sync**: near-head block synchronization from peers with short bounded lag.
- **Epoch catch-up**: bounded fallback mode that fetches missing ranges in larger chunks to converge back to live sync.
- **Legacy peer**: peer that does not advertise `sync_v1` capability but can still maintain baseline p2p connection.

---

## 4. Scope v1

### 4.1 Functional scope

1. **Mempool gossip (same-shard only)**
   - announce and request tx payloads,
   - deduplicate by `tx_id`,
   - bounded relay fanout.
2. **Live chain sync**
   - tip announcements,
   - header-first alignment,
   - bounded block body fetch.
3. **Epoch catch-up fallback**
   - activated on lag threshold or repeated live-sync miss,
   - range fetch by epoch-aligned windows,
   - return to live sync after lag is below threshold.

### 4.2 Out of scope

- historical archive backfill,
- state snapshot transfer format,
- optimistic execution with speculative branch import.

### 4.3 Cluster mesh and local broadcast (informal model)

Implementations MAY assume a **deployment profile** where several same-shard validators or full nodes share a **dense local segment** (typically low-latency L2/L3 connectivity or an operator-managed “local fanout” path) and connect to the wider network through **sparse edges** (NAT, long RTT, narrow uplinks). In that profile, a transaction may reach all members of the segment through a **local broadcast or equivalent fast path** before normal p2p gossip completes. Without extra rules, naive mempool relay would **re-flood the segment** and multiply traffic (L2/L3 “storm” of duplicate announces and batches). Section 13 normatively constrains relay in that profile; it does **not** introduce new wire message types for v1.

---

## 5. Capability Negotiation and Versioning

### 5.1 Capability keys

Peers participating in this RFC MUST advertise:

- `services` includes `sync`,
- `sync_capabilities` includes `same_shard_sync_v1` (logical capability name; on the wire it is signaled concretely via `sync_profile` in §5.2),
- `protocol_version` (semantic string) and `sync_wire_version` (integer major).

Minimal compatibility rule:

- `sync_wire_version == 1` is REQUIRED for full v1 sync interoperability.
- `sync_profile` in §5.2 is the only normative source for deciding `full_v1` eligibility; the bare token in `sync_capabilities` MUST NOT enable `full_v1` by itself.

### 5.2 Handshake extension contract

Handshake metadata (see RFC 0008) MUST be extended with:

```text
sync_profile: {
  sync_wire_version: u16,
  max_headers_per_msg: u16,
  max_blocks_per_msg: u16,
  max_txs_per_msg: u16,
  supports_epoch_catchup: bool
}
```

Receivers MUST treat the abstract token `same_shard_sync_v1` (§5.1) as present only when `sync_profile` is supplied with `sync_wire_version == 1` and limits are within local policy; missing or incompatible `sync_profile` implies the peer does not participate in full v1 sync negotiation even if other capability lists are present.

### 5.3 Negotiation result

For each peer, runtime computes:

- `sync_mode = full_v1` when peer supports `same_shard_sync_v1` and `sync_wire_version == 1`,
- `sync_mode = legacy_observe` otherwise.

`legacy_observe` peers MAY keep baseline p2p connectivity but MUST NOT be selected for v1 sync data exchange.

### 5.4 Versioning policy

- Backward-compatible field additions in v1 MUST be optional and ignorable.
- Breaking wire changes MUST bump `sync_wire_version` and are out of scope for this RFC.

---

## 6. Wire Message Contract (v1)

All messages MUST include:

- `msg_type`,
- `shard_id` (`domain_hi`),
- `peer_session_id`,
- `seq_no`,
- `timestamp_ms`.

v1 does not require a mandatory per-frame wire `net_zone` qualifier; zone policy is profile/segment-level as defined in §13.

If `shard_id` mismatches local shard, message MUST be dropped and counted as protocol reject.

### 6.1 Mempool messages

1. `TxAnnounce`
   - `tx_ids: Vec<hash>`
2. `TxRequest`
   - `tx_ids: Vec<hash>`
3. `TxBatch`
   - `txs: Vec<TxEnvelope>`

Minimal rules:

- `TxAnnounce` MUST NOT carry full tx bytes.
- `TxBatch` MUST contain canonical signed tx encoding.
- Receiver MUST deduplicate by `tx_id`.

### 6.2 Chain sync messages

1. `TipAnnounce`
   - `head_height: u64`
   - `head_hash: hash`
   - `finalized_height: u64`
2. `HeadersRequest`
   - `from_height: u64`
   - `limit: u16`
3. `HeadersBatch`
   - `headers: Vec<BlockHeader>`
4. `BlocksRequest`
   - `block_hashes: Vec<hash>`
5. `BlocksBatch`
   - `blocks: Vec<BlockEnvelope>`
6. `SyncNack`
   - `reason_code: enum`
   - `retry_after_ms: u32`

### 6.3 Epoch catch-up messages

1. `CatchupRequest`
   - `start_height: u64`
   - `end_height: u64`
   - `epoch_id: u64`
2. `CatchupChunk`
   - `epoch_id: u64`
   - `chunk_index: u32`
   - `headers: Vec<BlockHeader>`
   - `blocks: Vec<BlockEnvelope>`
3. `CatchupDone`
   - `epoch_id: u64`
   - `last_height: u64`

### 6.4 Late/out-of-slot and stale-response handling (normative)

1. A node MUST treat a received block as **late/out-of-slot** when either:
   - its height is below local `finalized_height`, or
   - its proposer does not match the expected proposer for that height per §7.1.
2. Late/out-of-slot blocks MUST NOT advance canonical head and MUST NOT trigger rollback of local finalized prefix; implementations MAY keep them only for diagnostics.
3. `HeadersBatch` / `BlocksBatch` / `CatchupChunk` responses MUST be accepted only if they match an active local request context (peer, range/hash-set, and freshness window).
4. A response outside active context (including delayed duplicates after head/request-window moved) is **stale** and MUST be dropped without side effects, except bounded counters/logging.
5. A stale response MUST NOT modify fork-choice inputs (`head_height`, `head_hash`, `finalized_height`) for canonical selection.

---

## 7. Proposer Model and Fork-Choice v1 (Deterministic Minimal)

### 7.1 Interim proposer selection model (v1)

This RFC follows current PoA devnet assumptions from WHITE/ADR:

1. For each block height there is exactly one expected proposer, deterministically derived as `expected_proposer = validators_fixed_order[height % N]`, where `N` is validator set size and `validators_fixed_order` is the agreed fixed order for the active set.
2. v1 sync does not introduce slot voting or multi-proposer consensus.
3. Consensus decisions for proposer eligibility MUST use only `(height, validators_fixed_order)` and MUST NOT depend on non-deterministic local sources such as `avg_peer_count`, first-seen order, chat order, arrival order, or time modulo peers.
4. A block signed by a non-expected proposer for the same height is out-of-slot for MVP v1, is not valid canonical progress for that height, and MUST NOT be preferred over a valid expected-proposer branch.

### 7.2 Concurrent candidates at the same height

If a node observes multiple candidate blocks at the same height (same or different `prev_hash`), it MUST:

1. first apply mandatory validity checks (header/body/signature/chain linkage),
2. then apply the deterministic branch rule in §7.3 only among candidates that passed validity,
3. keep behavior deterministic and side-effect free (no local timing heuristics).

This case is treated as branch resolution during sync, not as proposer election.

### 7.3 Normative branch rule (v1 fork-choice)

v1 fork-choice MUST be deterministic and apply the tuple order below:

1. higher `finalized_height`,
2. then higher `head_height`,
3. then lexicographically smaller `head_hash` (tie-breaker).

If candidate chain fails header/body validity checks, it MUST be rejected regardless of tuple score.

`finalized_height` is a synchronization boundary signal for branch comparison in v1, not a new finality protocol. For MVP v1, receiver-local PoA finalized prefix is the source-of-truth baseline; network-wide advancement mechanics remain inherited from current PoA operation and are outside this RFC revision.

For MVP v1, receivers MUST apply bounded semantics for `finalized_height`:

1. **Source:** remote `finalized_height` comes from peer `TipAnnounce` and represents peer-local finalized prefix of its canonical branch; locally, the receiver PoA finalized prefix remains the MVP source-of-truth baseline.
2. **Monotonicity:** local `finalized_height` baseline MUST be monotonic non-decreasing; per peer-session, advertised `finalized_height` MUST also be non-decreasing.
3. **Bounded regression handling:** if a peer advertises a lower `finalized_height` than previously accepted in the same session, receiver MUST treat it as stale regression, keep the last accepted value for that peer, and continue sync without rollback side effects.
4. **Bounded use in fork-choice:** receiver MAY use remote `finalized_height` only after normal header/body/link validation and MUST clamp it to `<= remote_head_height`; remote value MUST NOT force rollback below receiver-local finalized baseline.

### 7.4 Deferred decisions (explicit)

The following are intentionally deferred beyond v1:

- consensus upgrade to real multi-proposer competition / voting rounds,
- finalized-height governance redesign (quorum rules, long-range finality protocol),
- cumulative-weight / stake-weight scoring,
- latency-weighted peer trust,
- multi-branch speculative execution heuristics.

Implementations MUST NOT introduce hidden local heuristics that break determinism of section 7.3.

---

## 8. Anti-DoS Limits (Normative Minimum)

Runtime MUST enforce configurable hard caps with safe defaults:

- `max_headers_per_msg <= 512`,
- `max_blocks_per_msg <= 64`,
- `max_txs_per_msg <= 2048`,
- `max_inflight_sync_reqs_per_peer <= 8`,
- `max_catchup_window_blocks <= 4096`,
- `max_msg_bytes <= 4 MiB`.

Behavior on limit violation:

1. drop offending message,
2. increment reject metric by `reason_code`,
3. apply peer penalty/backoff,
4. disconnect on repeated violations above threshold.

Additionally:

- decode and verification MUST be time-bounded,
- duplicate `seq_no` beyond replay window MUST be rejected,
- catch-up requests outside allowed window MUST return `SyncNack`.

---

## 9. Legacy Compatibility

1. Nodes supporting RFC 0015 MUST keep baseline peering with legacy nodes when identity checks pass (RFC 0008).
2. Legacy peers without `same_shard_sync_v1` capability:
   - MAY participate in basic gossip channels outside this contract,
   - MUST NOT be used as source for v1 live sync or catch-up.
3. Mixed-network operation MUST remain safe: unsupported sync messages are ignored or rejected with `SyncNack` without process crash.

---

## 10. Observability Contract

Minimum metrics:

- `sync_mode_peers{mode=full_v1|legacy_observe}`,
- `sync_tip_lag_blocks`,
- `sync_headers_received_total`,
- `sync_blocks_received_total`,
- `sync_catchup_sessions_total{result}`,
- `sync_reject_total{reason_code}`,
- `mempool_sync_tx_dedup_total`.

`pwmd` MAY expose these via transport snapshot keys with equivalent semantics (for example: `sync_tip_seen_total`, `sync_hdr_resp_total`, `sync_blk_resp_total`, `sync_cup_*`, field `sync_v1_drop_reason` / JSON `sync_v1_msg_drop_reason_total`, field `sync_tx_drop_reason` / JSON `sync_tx_drop_reason_total`) as long as operator docs provide explicit mapping (`metrics.rs` lists `serde(rename)` where JSON keys differ).

When §13 (cluster storm guard) is enabled, implementations SHOULD additionally expose:

- `mempool_ingress_kind_total{kind}` — counts by ingress path (`p2p`, `local_broadcast`, `unknown`).
- `mempool_push_suppressed{reason}` (JSON key `mempool_cluster_push_suppressed_total`) — intra-cluster push/announce suppressed after §13.3.
- `mempool_egress_relay_total{peer_class}` — relay toward `external` vs `same_segment` (see §13.4).
- `mempool_ae_round_total{trigger}` — anti-entropy / pull rounds (reconnect, periodic, lag).

Alerting (operator-facing guidance, not wire):

- sudden rise in **bytes per unique `tx_id`** on segment-facing interfaces;
- high `mempool_push_suppressed` (JSON: `mempool_cluster_push_suppressed_total`) **without** corresponding `mempool_egress_relay_total` (possible misclassification: everything suppressed, nothing leaves segment);
- sustained drop in `mempool_ae_round_total` on edge nodes together with rising mempool divergence vs reference peers (possible starvation of §13.5 safeguards).

Minimum logs:

- negotiated sync profile per peer,
- mode transitions (`live` <-> `catchup`),
- fork-choice winner tuple snapshot,
- reject/disconnect events with `reason_code`.

---

## 11. Acceptance Criteria by Slice

### Slice 0 — RFC freeze (docs gate)

The following are satisfied before implementation slices 1–5:

1. RFC header **Status** marks this document frozen for Sprint V2-8 Slice 0, and **Abstract** states normative scope for v1 same-shard sync.
2. Cross-links: RFC 0008 references this document for `services` ⊇ `sync` behavior (`docs/rfc/8-shard-runtime-identity-and-peering.md` §4.1); **Depends on** here lists RFC 0008.
3. Sections 5–10 are present (capability negotiation, wire taxonomy, fork-choice v1, anti-DoS, legacy behavior, observability) with no unresolved contradiction against RFC 0008 handshake identity rules.
4. Sprint plan (`docs/plans/mvp_v2.md`, Sprint V2-8) names this file as the sync v1 contract anchor.

### Slice 1 - Wire foundations

Required now (Slice 1):

- Common wire envelope from Section 6 (`msg_type`, `shard_id`, `peer_session_id`, `seq_no`, `timestamp_ms`) is serialized/deserialized for implemented v1 sync messages.
- Chain subset from Section 6.2 is implemented for wire foundations: `HeadersRequest`, `HeadersBatch`, `BlocksRequest`, `BlocksBatch`, `SyncNack`.
- `shard_id` mismatch rejection is implemented and observable.

Deferred from Section 6 (not required in Slice 1):

- Section 6.1 mempool subset (`TxAnnounce`, `TxRequest`, `TxBatch`) -> Slice 2.
- `TipAnnounce` from Section 6.2 (live head signaling) -> Slice 3.
- Section 6.3 catch-up subset (`CatchupRequest`, `CatchupChunk`, `CatchupDone`) -> Slice 4.

### Slice 2 - Capability negotiation

- Section 5 negotiation yields deterministic `sync_mode`.
- Legacy peers are gated to `legacy_observe`.

### Slice 3 - Live sync

- Tip/header/block flow converges under bounded lag in same-shard network.
- Fork-choice v1 tuple from Section 7 is applied consistently.

### Slice 4 - Epoch catch-up fallback

- Catch-up activates on lag threshold and returns to live mode after convergence.
- Catch-up windows and chunking obey Anti-DoS limits.

### Slice 5 - Hardening and compatibility

- Anti-DoS limit violations produce reject metrics and penalties.
- Mixed peer set (v1 + legacy) stays stable without crashes or unsafe sync source selection.
- Optional: when operators enable §13, `mempool_*` suppression and egress metrics (§10) MUST reflect segment-classified relay; conservative fallback (`unknown` ingress) MUST NOT crash or wedge sync.

---

## 12. Rollout Notes

- Initial rollout target is testnet for Sprint V2-8.
- Default configuration SHOULD keep catch-up enabled but conservative.
- Operators MAY disable catch-up temporarily for diagnostics, while keeping live sync.
- For cluster-heavy topologies, use the **default profile** in §13.6 after enabling §13 behavior.

---

## 13. Cluster mesh: intra-cluster suppression and egress-first relay (normative)

This section applies only when the operator enables **cluster mesh storm guard** (`cluster_mesh_storm_guard_enabled`, §13.6). It refines **mempool gossip** (§4.1, §6.1) behavior; chain sync messages are unchanged.

### 13.1 Goals

1. After a transaction has been delivered to all segment members via a **local fast path**, avoid **re-flooding** the same segment over p2p (`TxAnnounce` / `TxBatch` push).
2. Preserve **reachability** to **external** peers (other segments, bridges, NAT periphery) and **liveness** via explicit **pull / anti-entropy** (§13.5).

### 13.2 Segments and ingress classification

1. **Same-shard segment** (logical): a set of nodes that share both the same `shard_id` (`domain_hi`, RFC 0008) and the same **local broadcast domain** identifier configured or inferred by the operator (e.g. one datacenter L2, one K8s cluster network, one VLAN). Implementations MUST NOT infer segment membership from IP alone without operator policy.
2. Every accepted transaction association MUST record an **ingress kind**:
   - `p2p` — received from a same-shard sync v1 peer over the normative p2p transport;
   - `local_broadcast` — received from a local fast path (multicast, shared bus, or in-process injection API explicitly marked as segment-fanout);
   - `unknown` — cannot be classified.
3. If ingress is `unknown` and segment guard is enabled, implementations MUST apply the **conservative path** (§13.7): behave as today’s dedupe + caps, without assuming segment-wide delivery.

### 13.3 Intra-cluster suppression after local-broadcast receipt

When `ingress_kind == local_broadcast` for `tx_id` at node `N`:

1. For every peer `P` classified as **same segment** as `N`, `N` MUST NOT send **`TxAnnounce` or `TxBatch`** for that `tx_id` (push relay) **for a suppression window** `T_suppress`, except as allowed by §13.5.
2. `T_suppress` MUST cover at least the typical time for all segment members to observe the local fast path (operator-tunable default in §13.6).
3. Per-`tx_id` deduplication (§6.1) remains mandatory; suppression is an **additional** outbound policy on **segment-local** push.
4. Rationale: all segment members are assumed to have been touched by the same fanout; push relay to segment peers would duplicate bytes on already saturated paths.

### 13.4 Egress-only forwarding (segment members)

For the same `tx_id` and `ingress_kind == local_broadcast`:

1. **External peers** (`peer_segment_id != local_segment_id`, or peer flagged as `bridge` / `uplink` in operator policy): `N` MAY push relay (`TxAnnounce` / `TxBatch`) subject to existing **bounded fanout** (§4.1, Anti-DoS §8). This is the **primary** path to propagate the transaction beyond the dense core.
2. **Same-segment peers**: relay push is suppressed per §13.3; recovery uses §13.5 only.
3. For `ingress_kind == p2p`, normal zoning policies (operator tuning) apply; §13.3–13.4 do not force egress-only, but implementations SHOULD still avoid blind fanout growth in dense graphs (see related guidance in `docs/reviews/20260508-mempool-mesh-anti-amplification-proposal.md`).

### 13.5 Exceptions and anti-entropy safeguards

Suppression MUST NOT permanently hide transactions from segment peers that **missed** the local fast path.

1. **Pull remains allowed:** `TxRequest` and responses are always permitted toward any eligible same-shard peer within Anti-DoS caps (§8).
2. **Anti-entropy rounds:** implementations SHOULD run periodic or event-driven **id reconciliation** (compact id sets / announces limited to **missing** ids) toward a small set of segment peers with **strict rate caps**, independent of §13.3 push suppression.
3. **Triggers** for extra anti-entropy toward segment interior: cold start, reconnect after `>T_suppress`, operator signal, or local **mempool divergence** heuristic vs median peer (implementation-defined but MUST be bounded).
4. **Bootstrap / single-member segment edge case:** if the node has no `local_broadcast` ingress path configured, §13.3 is a no-op; behavior reduces to standard gossip.

### 13.6 Default profile (recommended knobs)

Operators deploying **core + sparse periphery** (dense cluster, NAT edges) SHOULD start from:

| Knob | Default (guidance) | Notes |
|------|--------------------|--------|
| `cluster_mesh_storm_guard_enabled` | `false` in generic testnets; `true` only when `local_segment_id` is set | Avoid splitting visibility by accident. |
| `local_segment_id` | unset unless colocated mesh | Stable string per L2/L3 island; must match across intended cohort. |
| `T_suppress` | 500 ms – 2 s | Tuned to local fanout latency; upper bound lowers dup risk, lower bound avoids starving slow members. |
| `max_same_segment_push_fanout_after_local_bc` | `0` | Push to segment peers off for `local_broadcast` origin; use §13.5 for repair. |
| `ae_id_reconcile_period_sec` (same segment) | 30 – 120 | Rare pull/announce of **gaps** only; must respect §8 byte/msg caps. |
| `egress_push_jitter_ms` (toward external) | 10 – 50 | Spreads synchronized bursts from the core. |
| `mempool_per_peer_bytes_per_sec` / `msgs_per_sec` | reuse Anti-DoS tiers | Tighter on bridges and NAT peers (policy). |

### 13.7 Conservative fallback

If the runtime cannot reliably classify `local_broadcast` vs `p2p`, it MUST either:

- disable §13.3–13.4 for affected transactions (treat as `unknown`), or
- require operator acknowledgment that misclassification may cause **duplicate** or **delayed** propagation.

Implementations MUST document which transport hooks satisfy `local_broadcast` for their build.

---

## 14. pwmd / JsonFile: serving history below the in-memory tail (informative, non-normative)

This section **does not extend** the normative wire fields in §6. It documents **current `pwmd` behavior** for operators and integrators when a node stores the chain in **JsonFile epoch mode** (`pwm-data.json` + `epochs/*.jsonl` manifest) and the verified tip height exceeds the in-RAM block tail.

**Context.** The product chain keeps a bounded **deque of recent blocks** in memory (`pwm_core::TAIL_BLOCK_CAP`, currently **1000**). A syncing peer may request headers from `from_height = 1` (or any height below the lowest block still held in RAM). If the serving node answered only from RAM, it could **NACK** header ranges even though its **canonical tip** and on-disk epochs are ahead — the peer would observe a **non-growing local `mem` tip** while still seeing a high remote goal (symptom: «sync stuck at 0% memory»).

**Observed `pwmd` mitigation (post Sprint V2-8 lab hardening).** When RAM does not contain a contiguous run starting at `from_height`, the server **falls back** to **epoch JSONL** (manifest metadata + per-epoch files) to build `SyncHeadersBatch`, full block batches, and catch-up chunk windows, subject to the same linkage and hash checks as the RAM path. Block fetch requests may include an optional parallel **`block_heights`** list (serde-default absent for backward compatibility) so the server can load by height without a legacy **hash-only scan** of epoch files; peers that omit heights may still be served via a slower compatibility path.

**Operator pointers.**

- Snapshot layout and trust-load behavior: `docs/guide-node-storage-and-snapshot.md` (tail at startup) complements this section (outbound serving during sync).
- Code slice and review: `tasks/20260515-slice-sync-serve-below-ram-tail.json`, `docs/reviews/20260515-sync-serve-below-ram-tail-slice.md`, `docs/reviews/20260516-sync-serve-docs-and-style-review.md`.
- **CY lab (three nodes):** `cy-cluster-common.ps1`, `cy-cluster-proposer.ps1`, `cy-cluster-attester.ps1`, `cy-cluster-follower.ps1` — follower is **not** in the RFC16 quorum but uses the same `--seal-lease-backend process-local` convention as the quorum processes to avoid stale file-lease contention on Windows repeat runs; peer mesh is symmetric (each process seeds the other two listen addrs). See also `docs/reviews/20260512-cy-cluster-windows-peer-bind.md`.

**Compatibility.** Optional `block_heights` is a **backward-compatible** JSON extension on the existing blocks request message; legacy peers omit the field. Normative names of wire messages in §6 remain the source of truth for taxonomy.
