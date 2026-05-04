-- PWM incremental snapshots (design lock: docs/reviews/sprint-15-slice-7-plan.md §3).
--
-- Topology: one ClickHouse database per network id (sanitized); physical table names use the
-- cluster/domain suffix `__0xHH`. Logical chain identity is `row_key` (network + domain + genesis).
--
-- ReplacingMergeTree deduplicates on merge by ORDER BY key, keeping the row with the greatest
-- `inserted_at`. Therefore **ORDER BY must include `row_key`** for `blocks` and `checkpoints`:
-- otherwise two logical chains sharing one physical table could corrupt each other at the same
-- `height` / `(genesis_digest, checkpoint_height)`. Prefer separate DBs/tables per deployment;
-- the sort key is the safety net when co-location happens.

CREATE TABLE IF NOT EXISTS {database}.`blocks__0x01`
(
    row_key String,
    height UInt64,
    block_hash String,
    prev_hash String,
    ts UInt64,
    prod_idx UInt32,
    tx_count UInt64,
    state_root String,
    payload_json String,
    inserted_at DateTime DEFAULT now()
)
ENGINE = ReplacingMergeTree(inserted_at)
ORDER BY (row_key, height);

CREATE TABLE IF NOT EXISTS {database}.`checkpoints__0x01`
(
    row_key String,
    genesis_digest String,
    checkpoint_height UInt64,
    state_root String DEFAULT '',
    state_json String,
    roaming_json String,
    cross_shard_json String,
    shard_balance String DEFAULT '',
    inserted_at DateTime DEFAULT now()
)
ENGINE = ReplacingMergeTree(inserted_at)
ORDER BY (row_key, genesis_digest, checkpoint_height);

-- Validator checkpoint acceptance (append-only intent; no runtime DELETE/UPDATE).
CREATE TABLE IF NOT EXISTS {database}.`validators_accept__0x01`
(
    row_key String,
    checkpoint_height UInt64,
    validator_id String,
    checkpoint_digest String,
    sig String,
    accepted_at DateTime DEFAULT now()
)
ENGINE = MergeTree()
ORDER BY (row_key, checkpoint_height, validator_id);

-- Legacy monolithic import / migration reader (optional).
CREATE TABLE IF NOT EXISTS {database}.node_snapshot
(
    row_key String,
    snapshot_json String,
    inserted_at DateTime DEFAULT now()
)
ENGINE = ReplacingMergeTree(inserted_at)
ORDER BY (row_key);
