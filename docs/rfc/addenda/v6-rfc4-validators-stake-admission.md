# RFC 4 Addendum (V6): Stake-gated active validator set

**Parent:** [4-validators-and-finality.md](../4-validators-and-finality.md)  
**Status:** Normative for MVP v6 (V6-1 freeze)  
**Depends on:** [mvp_v6.md](../../plans/mvp_v6.md), snapshot v4

## 1. Summary

V6 introduces **stake-gated admission** to an **active** validator index list while preserving the full **registered** set from genesis for wire compatibility. Seal and RFC16 proposer selection use **active** indices only.

This addendum does **not** replace `Chain::seal` with CometBFT/BFT (see [CONCEPT_ROADMAP.md](../../CONCEPT_ROADMAP.md) §MVP V6/V7).

## 2. Definitions

```text
RegisteredValidatorSet  // GenCfg.vals: Vec<ValidatorEntry> — static wire, never deleted in V6
ActiveValidatorIndices  // Vec<u16> — indices into GenCfg.vals, recomputed each epoch boundary
ValidatorAccount        // Account bound to validator pubkey; holds staked_pwm (existing stake model)
```

## 3. GenCfg parameters (frozen)

```text
min_validator_stake: u128     // minimum staked_pwm on validator account; JSON: decimal string per RFC 12 / RFC 7
epoch_length_blocks: u64       // genesis-defined; MUST be > 0
```

Shard state (snapshot v4):

```text
epoch_counter: u64
active_validator_indices: Vec<u16>
```

## 4. Epoch boundary algorithm

At block height `h` where `h % epoch_length_blocks == 0` and `h > 0`:

1. Increment `epoch_counter`.
2. For each index `i` in `0..GenCfg.vals.len()`:
   - Resolve validator account stake `staked_pwm` for pubkey `GenCfg.vals[i].pubkey`.
   - If `staked_pwm >= min_validator_stake`, include `i` in `active_validator_indices`.
   - Else exclude `i` (inactive for this epoch).
3. If `active_validator_indices` is empty, block seal MUST fail with profile-defined halt (devnet: reject block production; test harness MAY use fallback single bootstrap index — MUST be explicit in test genesis only).

**Bootstrap:** genesis may include validators below threshold; they are **inactive** until stake rises. Entries are **not** removed from `GenCfg.vals`.

## 5. Seal and finality

- Proposer selection uses `active_validator_indices` only ([RFC 16 addendum](v6-rfc16-multi-proposer-rotation.md)).
- Finality threshold profiles operate over active set size, not registered set size.
- Stake changes mid-epoch affect admission only at the **next** epoch boundary (no instant ejection mid-epoch in V6).

## 6. Slashing stub

Evidence records ([ADR 0010](../../adr/0010-slashing-evidence-stubs.md)) do not remove validators from active set in V6.

## 7. Fork compatibility

- Additive snapshot v4 fields; v3 nodes reject v4 snapshots.
- `min_validator_stake = 0` recovers V5-like behavior (all registered validators active).

## 8. Non-goals (V6)

- Dynamic validator registration tx.
- Auto-unstake on inactivity.
- Cross-shard validator sets.
- CometBFT validator updates.

## 9. Snapshot trust-load (implementation note)

Snapshot v4 persists `epoch_counter` and `active_validator_indices` at the summary checkpoint so **JsonFile trust-default** cold start does not replay genesis→tip for proposer schedule (see [guide-node-storage-and-snapshot.md](../../guide-node-storage-and-snapshot.md) §Design alignment).

- Trust path: proposer checks on the loaded tail use the persisted active set when no epoch boundary falls inside the tail window.
- Audit path: `--snapshot-verify-chain` or summary/manifest lag still forces full replay.
- Stake-gated admission semantics (§4) are unchanged; only startup validation cost was brought in line with the epoch+tail snapshot model.
