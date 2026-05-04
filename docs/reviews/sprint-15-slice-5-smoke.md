# Sprint 15 Slice 5 — Smoke: ClickHouse snapshot (pwmd)

Prereq: build `pwmd` with `--features clickhouse-snapshot`.

## 1. Start ClickHouse

From repo root:

```bash
docker compose -f tools/docker/pwmd-clickhouse-compose.yaml up -d
```

First boot runs `tools/docker/sql/clickhouse_pwm_snapshots.ddl` (database `pwm_snapshots`, table `node_snapshot`).

## 2. Run pwmd against CH

Example (devnet identity + HTTP base):

```bash
cargo run -p pwmd --features clickhouse-snapshot -- \
  --snapshot-backend clickhouse \
  --clickhouse-url http://127.0.0.1:8123 \
  --clickhouse-database pwm_snapshots \
  --clickhouse-table node_snapshot \
  --network-id devnet --domain-hi 16 --cluster-id c1 --node-id n1
```

Row key defaults to `network|0xHH|cluster|node|<genesis_state0_digest_hex>`. Override with `--snapshot-store-key` / `PWM_SNAPSHOT_STORE_KEY` (ascii alnum plus `._-|+:` only; slash `/` is not allowed).

## 3. Optional cargo check against live CH

With the server up:

```bash
set PWM_CLICKHOUSE_TEST_URL=http://127.0.0.1:8123
cargo test -p pwmd --features clickhouse-snapshot ch_ping_env -- --nocapture
```

## 4. Default build (no Docker)

Without the feature, `cargo test --workspace` ignores CH; behavior stays JSON file at `--data-file`.
