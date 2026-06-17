# Guide: node storage and snapshot load modes

Audience: node operator / integrator running `pwmd` with local JsonFile or ClickHouse persistence.

## Directory layout

By default `pwmd` stores node state under `--state-root state` and chooses a namespace from the effective runtime identity:

- neutral default: `state/neutral/<listen-addr>/pwm-data.json`;
- explicit domain mode: `state/domain-hi-0xNN/pwm-data.json`;
- explicit override: `--data-file <PATH>`.

Historical note: older trees may still contain `state/shard-a` / `state/shard-b` directories from pre–domain-first tooling; current `pwmd` does not write or prefer those paths.

For JsonFile epoch storage, `pwm-data.json` is the summary file. It contains canonical state, genesis identity rows, roaming/cross-shard state, `blocks_stored = "epochs"`, and `checkpoint_height`. Older inline snapshots may still contain the full `blocks` array in `pwm-data.json`; those are legacy/compat inputs.

The epoch layout next to the summary is:

```text
<snapshot-dir>/
  pwm-data.json
  epochs/
    pwm-epochs-manifest.json
    block_e0.json
    block_e1.json
    ...
```

`epochs/block_e*.json` files are JSONL block shards. Each epoch file covers up to `EPOCH_SPAN = 1000` block heights. The manifest records `canonical_h`, `tip_hash`, and epoch file ranges. The summary `checkpoint_height` is the state checkpoint height; in a healthy trust-default load it must equal manifest `canonical_h`.

`SNAP_CHK_BLK_IV = 100` controls periodic summary/checkpoint rewrite on the seal path. It is separate from `EPOCH_SPAN`: checkpoints are denser than epoch-file boundaries.

## Epoch manifest schema contract (V3-2)

- `epochs/pwm-epochs-manifest.json` uses its own schema stream (`schema_v`) and is independent from:
  - genesis bundle `schema_version`;
  - snapshot wire `version` in `pwm-data.json`.
- Current runtime contract accepts only `schema_v = 1` (`EPOCH_MAN_SCHEMA_CUR` in code).
- Any other value is rejected with explicit `unsupported epoch manifest schema ...` error.
- V3-2 does not introduce Bootstrap Snapshot / cleanup-chain / pruning semantics; those remain deferred.

## Tail load

At runtime the in-memory chain may keep only a recent tail of blocks, bounded by `TAIL_BLOCK_CAP`. On restart, normal JsonFile loading reads the summary state and only the recent tail from `epochs/`, then checks that the tail links to the stored parent block and to manifest `tip_hash`.

This means old blocks remain on disk for audit/rebuild, but routine startup does not deserialize and replay every epoch file.

## Load modes

### Normal: trust-default

Default JsonFile startup trusts the local summary plus manifest as the disk checkpoint. It validates genesis identity, summary/manifest agreement, tail linkage, PoA header signatures, `tx_root`, and final state root. It does not replay all historical transactions.

Producer schedule checks in trust mode are `O(tail)` and use persisted snapshot state (`active_validator_indices`, `epoch_counter`) as the checkpoint source. If no epoch boundary falls inside the loaded tail, proposer checks use the persisted active set for the whole tail. If an epoch boundary is inside the tail window, trust validation runs only a sequential epoch-file pass from that boundary to tip (no genesis→tip replay).

**Genesis anchor (ADR 0008):** trust load checks that the snapshot is tied to the same genesis as `--genesis-file`:

- `genesis_state_root` and `gencfg_digest` must match the loaded `GenCfg` when `genesis_anchor` is present;
- single-validator `genesis_anchor.signature` (fool-guard against careless disk edits);
- when `checkpoint_height >= 1`, **block height=1** is loaded from epochs and verified (`prev_gen`, header hash, PoA sig, light replay of txs into `state0()`) — **even on legacy bypass** (see env below).

Legacy snapshots without `genesis_anchor` are **migrated on load** (warn + persist on next save) when preflight passes and the node has a validator signing key; otherwise use `--snapshot-verify-chain` once.

**Emergency legacy bypass (unsafe):** `PWM_SNAPSHOT_ANCHOR_MIGRATE=1` allows trust load of an old snapshot **without** `genesis_anchor` and **without** signing a new anchor at load. This does **not** skip block@1 preflight when `checkpoint_height >= 1`, and does **not** replace `--snapshot-verify-chain` for a full genesis→tip audit. Use only for one-off recovery; prefer migrate-on-load or verify-chain.

If block 1 was pruned from disk, startup fails with `missing genesis anchor block1 (pruned)` until epoch `e0` is restored.

See [adr/0008-snapshot-genesis-anchor-light.md](adr/0008-snapshot-genesis-anchor-light.md).

Use this for ordinary restarts on a trusted disk.

Operational SLO target: trust-start validation on a ~125k tip should stay well below previous 15–20 minute behavior; target on typical dev/beta hardware is under 60 seconds.

#### Design alignment (trust-load, closed 2026-06)

**Intent (since epoch snapshots + tail cap):** routine cold start reads summary state at tip and only the last `TAIL_BLOCK_CAP` blocks from `epochs/` — not a genesis→tip replay.

**Gap (V6-3 through 2026-06-17):** `validate_snapshot_trusted` still ran `trust_tail_prod_idx` over heights `1..tip` to derive proposer schedule, re-reading epoch JSONL per height. Symptom: `snapshot_load_mode=trust` with `epochs_ms` ~50 ms but `validate_ms` ~1.1M ms (~17–20 min @125k tip); seal loop blocked until load finished.

**Fix (`20260619-pwmd-trust-load-fastpath-proposer-validation`):** trust validation is **O(tail)** — uses persisted snapshot v4 `active_validator_indices` / `epoch_counter` (RFC V6-3). Full genesis replay remains only for `--snapshot-verify-chain` or `summary_manifest_lag` forced verify. Progress: log target `pwmd::startup::snapshot`, `stage=trust_validate`.

**Operator check:** after restart, `snapshot startup load ok` should show small `validate_ms` (seconds, not minutes) and the node reaches `ready` quickly enough for seal; see `cluster_prep_summary` / `sealed height=` in proposer logs.

### Audit: full replay

Set either:

```bash
pwmd --snapshot-verify-chain
```

or:

```bash
PWM_SNAPSHOT_VERIFY_CHAIN=1 pwmd
```

Truth values other than empty/`0`/`false`/`no`/`off` enable audit mode. Audit mode loads all epoch blocks and runs full genesis-to-tip replay validation. It is slower but stronger when checking disk integrity.

### Focused V3 replay determinism gate

Use this lightweight command in local runs and CI when you need a deterministic replay gate without running the full workspace suite:

```bash
cargo test -p pwmd --lib v3_replay_det_gate_ok
```

What it catches:

- replay reproducibility on the same fixture chain (state root/tip hash must match between two replay runs);
- regressions in replay path that can desync `Epoch Snapshot` validation.

It does **not** by itself exercise JsonFile `epochs/` manifest I/O; pair coverage with `epoch_man_v` / epoch persistence tests when changing manifest on-disk format.

### Automatic fallback

If `pwm-data.json` says `checkpoint_height = H1` while the manifest says `canonical_h = H2` and `H2 > 0`, `pwmd` forces full verification. This catches the important partial-write case where epoch files advanced but the summary checkpoint lagged.

## ClickHouse

ClickHouse snapshot load currently remains full replay. The JsonFile trust-default option does not weaken CH validation; CH reconstructs from stored blocks/checkpoints and validates by replay. Treat JsonFile and CH as different operational modes until a CH-specific trust-checkpoint contract is introduced.

## Troubleshooting

| Symptom | Meaning | Action |
|---|---|---|
| Missing `epochs/pwm-epochs-manifest.json` | JsonFile epoch manifest is absent; the node may be on legacy inline format or the epoch store is incomplete. | If this is expected legacy data, start normally and let runtime persistence migrate as designed. If epochs should exist, inspect/restore the snapshot directory. |
| Summary lags manifest | `checkpoint_height` differs from manifest `canonical_h`. | Let automatic full verify run. If it repeats every restart, check disk permissions and interrupted writes. |
| Corruption suspicion | State root mismatch, invalid tail linkage, bad `tip_hash`, or unexpected degraded startup. | Restart once with `--snapshot-verify-chain`; if it fails, restore from a known-good snapshot or rebuild from genesis/test data. |
| Slow audit startup | Full replay is enabled or forced. | Use normal mode for routine restarts after the audit cause is resolved. |
