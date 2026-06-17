# RFC 15 Addendum (V6): Peer sync scoring (lightweight)

**Parent:** [15-same-shard-sync-v1.md](../15-same-shard-sync-v1.md)  
**Status:** Normative for MVP v6 (operator-local first)  
**Out of scope:** RFC 15 non-goal «full p2p reputation system»

## 1. Summary

V6 adds a **lightweight integer score** per peer to bias sync/backfill peer selection and surface misbehaviour to operators. Scores are **not** consensus state in the default V6 profile.

## 2. Default profile (non-consensus)

Stored in `pwmd` operator-local cache (persistent optional):

```text
PeerSyncScore {
  peer_id: PeerId,           // stable handshake identity
  score: i32,               // initial 0
  last_updated_unix: u64,
}
```

Events (additive):

| Event | Δ score |
|-------|---------|
| Successful live sync round | +1 |
| Served valid blocks to us | +1 |
| Timeout / stale tip after handshake | -2 |
| Invalid block hash or fork mismatch | -5 |
| Bridge trust refusal contributor | -10 |

Selection: prefer higher score among peers meeting `sync_v1` capability and lag bounds.

## 3. Optional consensus table (deferred unless review mandates)

If a future slice requires consensus-visible scores, snapshot v4 MAY add `GenCfg`-bounded `peer_score_table`. **V6 default profile does not include this table.**

## 4. Relation to slashing evidence

Peer score adjustments MUST NOT seize stake. Severe faults MAY generate operator alerts and optional `EvidenceTx` stub ([ADR 0010](../../adr/0010-slashing-evidence-stubs.md)) — separate paths.

## 5. Non-goals (V6)

- Token rewards for good peers.
- Gossip propagation of scores across shards.
- On-chain registration of peer reputation.
