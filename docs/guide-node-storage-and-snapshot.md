# Guide: node storage and snapshot load modes

Audience: node operator / integrator running `pwmd` with local JsonFile or ClickHouse persistence.

## Directory layout

By default `pwmd` stores node state under `--state-root state` and chooses a namespace from the effective runtime identity:

- neutral default: `state/neutral/<listen-addr>/pwm-data.json`;
- explicit domain mode: `state/domain-hi-0xNN/pwm-data.json`;
- legacy `--shard A|B`: `state/shard-a|shard-b/pwm-data.json`;
- explicit override: `--data-file <PATH>`.

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

## Tail load

At runtime the in-memory chain may keep only a recent tail of blocks, bounded by `TAIL_BLOCK_CAP`. On restart, normal JsonFile loading reads the summary state and only the recent tail from `epochs/`, then checks that the tail links to the stored parent block and to manifest `tip_hash`.

This means old blocks remain on disk for audit/rebuild, but routine startup does not deserialize and replay every epoch file.

## Load modes

### Normal: trust-default

Default JsonFile startup trusts the local summary plus manifest as the disk checkpoint. It validates genesis identity, summary/manifest agreement, tail linkage, PoA header signatures, `tx_root`, and final state root. It does not replay all historical transactions.

Use this for ordinary restarts on a trusted disk.

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
