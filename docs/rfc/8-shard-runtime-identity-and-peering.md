# RFC 0008: Shard Runtime Identity and Peering

**Status:** Draft  
**Version:** 0.1  
**Depends on:**

- `docs/WHITE_SPEC_v0.md`
- `docs/rfc/1-address-format.md`
- `docs/rfc/6-policy-engine.md`
- `docs/adr/0001-consensus-and-node-stack.md`

---

## 1. Abstract

This RFC defines the next increment for shard-aware runtime networking:

- launch-time shard/cluster parameterization,
- deterministic node self-identification in network metadata,
- peer classes (`native` vs `foreign`),
- priority policy for native links,
- anti-spoof identity checks,
- minimal observability surface for operations.

This document is **spec/architecture only**.  
It does not introduce runtime behavior changes by itself.

---

## 2. Terminology

### 2.1 `spec-level geo-shard`

Protocol-level shard semantics tied to a **fixed `domain_hi` value** (high byte of `domain_code`), as defined in `WHITE_SPEC_v0`.

### 2.2 `runtime shard instance`

A running `pwmd` process bound to a concrete shard cluster identity and network context.

### 2.3 `domain cluster`

Operational and protocol grouping for one fixed `domain_hi`, including validators, state scope, and preferred peer neighborhood.

### 2.4 `native shard` (for a runtime)

A shard cluster whose identity matches the runtime's configured cluster identity.

### 2.5 `foreign shard` (for a runtime)

A shard cluster whose identity does not match the runtime's configured cluster identity.

---

## 3. Non-Goals

- Define cross-shard finality certificate internals.
- Replace existing transaction routing semantics from `WHITE_SPEC_v0`.
- Introduce heuristic range splits for shard semantics.

**Normative prohibition:** `domain_hi` range heuristics (including `0x80 split`) MUST NOT be used as source of shard identity, routing, or policy truth.

---

## 4. Runtime Launch Parameterization

Each runtime shard instance MUST start with explicit identity configuration.

### 4.1 Required identity parameters

`cluster_identity`:

- `domain_hi` (`u8`) - canonical shard-cluster identifier at protocol level.
- `cluster_id` (`string`) - operator-facing stable label for same cluster (for logs/discovery UX).

`network_identity`:

- `network_id` (`string`) - network namespace (`devnet`, `testnet-v1`, etc.).
- `genesis_hash` (`hex`) - optional but RECOMMENDED anchor to prevent cross-network accidental peering.

`node_identity`:

- `node_id` (`string`) - stable node identifier in this network.
- `node_pubkey` (`hex`) - transport/auth key for identity verification.

`advertised_capabilities`:

- `protocol_version` (semantic version string),
- `tx_features` (list; e.g. `["local_transfer_v1", "export_import_v1"]`),
- `services` (list; e.g. `["mempool", "gossip", "sync"]`).

Normative linkage: when `services` includes `sync`, same-shard sync behavior for v1 is specified by `docs/rfc/15-same-shard-sync-v1.md` (RFC 0015).

### 4.2 Launch consistency rules

1. Runtime MUST reject startup when required identity fields are missing.
2. Runtime MUST log effective identity tuple on startup:
   - `network_id`,
   - `domain_hi`,
   - `cluster_id`,
   - `node_id`,
   - `capability fingerprint`.
3. Runtime MUST NOT derive cluster identity from address ranges or runtime heuristics.

---

## 5. Node Self-Identification in Handshake

### 5.1 Handshake metadata envelope

Each outbound and inbound peer handshake MUST include:

```text
NodeHello {
  network_id: string,
  genesis_hash: hex,
  cluster: {
    domain_hi: u8,
    cluster_id: string
  },
  node: {
    node_id: string,
    pubkey: bytes
  },
  capabilities: {
    protocol_version: string,
    tx_features: Vec<string>,
    services: Vec<string>
  },
  nonce: bytes,
  timestamp_ms: u64,
  signature: bytes
}
```

Where `signature` covers all fields above except `signature`.

### 5.2 Identity acceptance gate

Connection MAY proceed only when:

- `network_id` matches local runtime configuration,
- if enabled, `genesis_hash` matches local genesis anchor,
- signature verifies against advertised `pubkey`,
- `timestamp_ms` within configured skew window,
- `nonce` not replayed inside replay window,
- `node_id` is not locally denied (optional static/operator denylist).

On failure, connection MUST be dropped with reason-coded log and metric.

---

## 6. Peer Classes

After successful handshake, runtime MUST classify peer:

- `native`: `peer.cluster.domain_hi == local.cluster.domain_hi`
- `foreign`: otherwise

`cluster_id` is auxiliary metadata and MUST NOT override `domain_hi` for class assignment.

---

## 7. Peer Priority Policy (Native First)

### 7.1 Policy objective

Maintain stronger connectivity and faster recovery inside native cluster, while preserving controlled foreign connectivity for cross-shard messaging and liveness.

### 7.2 Connection budget and queues

Runtime SHOULD keep separate budgets:

- `native_outbound_target`,
- `foreign_outbound_target`,
- `native_inbound_soft_limit`,
- `foreign_inbound_soft_limit`.

Dial queue priority:

1. Native peers with healthy recent history.
2. Native peers with stale history.
3. Foreign peers required by configured minimum diversity.
4. Other foreign peers.

### 7.3 Reconnect and backoff

- Native peers use tighter reconnect intervals and lower max backoff.
- Foreign peers use wider intervals and higher max backoff.
- Reconnect jitter is REQUIRED for both classes.

### 7.4 Gossip and relay weighting

For bounded dissemination channels, runtime SHOULD apply higher weight to native links:

- higher initial fanout quota for native peers,
- lower but non-zero quota for foreign peers,
- class-aware retry order preferring native first.

### 7.5 Failover behavior

If native peer count drops below `native_min_live`:

- runtime MAY temporarily borrow from foreign budget,
- runtime MUST continue native recovery attempts,
- runtime MUST emit degraded-state signal in logs/metrics.

---

## 8. Security and Anti-Spoof Requirements

### 8.1 Identity integrity

- Handshake metadata MUST be signed.
- `node_id -> pubkey` binding SHOULD be stable and persisted in peer store.
- Unexpected key rotation for known `node_id` MUST be treated as suspicious event (policy: reject or quarantine).

### 8.2 Cluster claim validation

- Peer cluster class MUST be derived from signed `domain_hi` claim.
- Local node MUST NOT infer "native" from endpoint naming, IP, or operator tags.

### 8.3 Replay and downgrade protections

- Nonce replay window is REQUIRED.
- Capability negotiation MUST reject unsupported mandatory protocol versions.
- Runtime MUST log attempted downgrade or incompatible capability sets.

---

## 9. Observability Requirements

Minimum metrics:

- `p2p_peers_connected{class=native|foreign}`
- `p2p_dial_attempts_total{class,result}`
- `p2p_handshake_reject_total{reason}`
- `p2p_reconnect_backoff_seconds{class}`
- `p2p_gossip_msgs_sent_total{class,topic}`
- `p2p_native_degraded_state` (`0|1`)

Minimum structured logs:

- startup identity summary,
- handshake accept/reject with reason and peer identity tuple,
- peer class transitions (if any),
- native budget underflow and recovery events.

Operational SLO examples (non-normative):

- native connection ratio above threshold,
- handshake rejection spike alert by reason,
- degraded native state duration alert.

---

## 10. Historical note: legacy CLI `--shard` and `state/shard-*` paths

Earlier iterations exposed a dev-only `--shard A|B` mapping and snapshot directories `state/shard-a` / `state/shard-b`. That compatibility surface and those paths are **removed** from the supported operator contract; production-style and MVP docs assume **explicit cluster-bound launch** (`--network-id`, `--domain-hi` / `--cluster-domain-hi`, `--cluster-id`, `--node-id`) or **neutral relay baseline**, with snapshot namespaces `domain-hi-0xNN` and `neutral/<listen-tag>` only.

Protocol routing invariants from `WHITE_SPEC_v0` and `RFC 0006` are unchanged; no range-based `domain_hi` heuristics apply in identity logic.

---

## 11. Acceptance Criteria (for next implementation stage)

Implementation is accepted when all criteria below are met:

1. Runtime requires explicit cluster/network/node identity fields at launch.
2. Handshake metadata includes signed identity envelope and capability set.
3. Peer classification (`native`/`foreign`) is deterministic by `domain_hi` equality only.
4. Priority policy is implemented across:
   - dial queue,
   - reconnect/backoff,
   - gossip weighting,
   - degraded-state failover signaling.
5. Anti-spoof checks enforce signature, replay window, and network/genesis compatibility.
6. Minimal metrics/logs from Section 9 are emitted and documented.
7. Operator docs describe explicit/neutral launch only (no legacy `--shard` / `state/shard-*` paths).
8. No contradiction with `WHITE_SPEC_v0` and `RFC 0006` shard semantics.
9. No use of range heuristics (`0x80 split` or analogs) in identity/routing/priority logic.

---

## 12. Open Questions

1. Should `genesis_hash` be mandatory in all handshake profiles, or optional for local dev?
2. Should `node_id -> pubkey` rotation policy be strict reject by default or quarantine-first?
3. What minimum foreign diversity budget is required for resilient cross-shard signaling in v1 testnet?

