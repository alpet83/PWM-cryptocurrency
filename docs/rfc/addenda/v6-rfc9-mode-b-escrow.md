# RFC 9 Addendum (V6): Mode B cross-shard escrow (normative)

**Parent:** [9-crossdomain-roaming.md](../9-crossdomain-roaming.md) §A.5  
**Status:** Normative for MVP v6 — supersedes §A.5 «not implemented» posture for Mode B  
**Depends on:** snapshot v4, [mvp_v6.md](../../plans/mvp_v6.md)

## 1. Summary

Mode B makes `EXPORT` on the source shard **lock** spendable balance until `IMPORT` finalizes on the target or a **timeout** triggers refund. This closes the griefing window described in RFC 9 §A.5 without HTLC/CLTV or settlement chain (§A.4 still deferred).

## 2. State machine

```text
States: None → Locked → Released | Refunded

EXPORT applied:
  - Create CrossShardLock
  - Decrease sender spendable balance by lock amount
  - Record export_id in cross-shard ledger (level 2)

IMPORT applied (valid proof):
  - Transition lock Locked → Released
  - Target shard credits per existing IMPORT rules
  - Source shard removes lock; export_id marked consumed

Timeout (unlock_height reached):
  - Transition lock Locked → Refunded
  - Credit sender per refund_policy (default: same account, full amount)
  - export_id remains consumed for replay guards
```

## 3. Wire types (frozen)

```text
GenCfg.cross_shard_lock_timeout_blocks: u64  // default e.g. 604800 (~7d at 1s blocks)

CrossShardLock {
  export_id: Hash32,
  sender: AccountId,
  amount_pwm: u128,            // JSON: decimal string per RFC 12 / RFC 7
  target_domain: u16,
  refund_account: AccountId,   // default sender
  lock_height: u64,
  unlock_height: u64,          // lock_height + cross_shard_lock_timeout_blocks
  state: Locked | Released | Refunded,
}
```

`EXPORT` tx MUST reference `export_id` deterministically (existing provenance rules). Lock creation is **atomic** with EXPORT apply.

## 4. Griefing boundaries

- Sender cannot double-spend locked amount on source while `state = Locked`.
- Late `IMPORT` after refund MUST reject with `E_EXPORT_LOCK_REFUNDED`.
- Duplicate `IMPORT` for same `export_id` → existing replay reject.
- Target shard `IMPORT` without source lock MAY reject at federation trust layer (level-2 divergence → bridge trust refusal per §A.6).

## 5. Seal / tick (normative)

Refund **MUST** be applied on seal tick when `current_height >= unlock_height` for any `CrossShardLock` in `Locked` state. Lazy refund on unrelated account txs is **not** a consensus path in V6 (deterministic state root at each height).

## 6. Fork compatibility

- Additive v4 only.
- Nodes without Mode B MUST NOT process v4 snapshots with locks (version gate).

## 7. Non-goals (V6)

- Settlement/import-export chain (§A.4).
- HTLC, CLTV, hash preimage paths.
- Cross-shard lock on non-EXPORT paths.
