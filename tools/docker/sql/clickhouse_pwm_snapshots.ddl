-- ClickHouse DDL for pwmd prototype snapshot store (see sprint-15 slice 5 smoke).

CREATE DATABASE IF NOT EXISTS pwm_snapshots;

CREATE TABLE IF NOT EXISTS pwm_snapshots.node_snapshot
(
    row_key String,
    inserted_at DateTime64(3) DEFAULT now64(3),
    snapshot_json String
)
ENGINE = MergeTree
ORDER BY (row_key, inserted_at);
