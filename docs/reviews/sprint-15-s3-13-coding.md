# Sprint 15 S3.13 — Federation table (coding handoff)

## Delivered

- **Runtime store:** `Inner.federation` (`FederationTable`) in `crates/pwmd/src/federation.rs`: merge rules per S3.11 review (height monotonic, `last_seen` via `max`, lower height updates only seen/source), TTL **60s**, `expires_at = last_seen + ttl_ms`.
- **Trust:** updates only from paths that already treat peer data as trusted (`merge_cross_shard_facts`-style): outbound seed HTTP/wire hello after `process_incoming_peer_hello(..., true)`, trusted wire heartbeats, `trust_peer_for_test`. Inbound TCP hello stays `provenance_trusted = false` → **no** federation writes; HTTP `/v1/peer/hello` unchanged (`false`) → **no** federation writes from HTTP peer hello.
- **Wire:** optional `chain_tip_height`, `federation_shard_id` on `NodeHello` (included in signing payload when `Some`, skipped when `None` for compat). `PeerWireMsg::Heartbeat` extended with the same optional fields (serde default).
- **Local row:** `GET` snapshot path calls `merge_local_status` so the local shard appears with `source: "status"` and local `node_id`.
- **Sweep:** `spawn_federation_sweep_loop` (~1s interval) evicts rows with `now >= expires_at`; `federation_http_snapshot` builds JSON counts then sweeps the map.
- **API:** `GET /v1/federation/shards` → `generated_at_unix_ms`, `ttl_sec`, `view_health`, `expected_shard_count`, `active_shard_count`, `stale_shard_count`, `rows` matching the review contract (`fresh` = `now < expires_at` before eviction).
- **Version:** `pwmd` crate **0.1.29 → 0.1.30** (new public HTTP route + response shape).

## `expected_shard_count`

- Handler currently passes **`null`** (`None`): no dedicated operator config or genesis-derived federation size is wired yet.
- **Genesis note:** current `GenCfg` does not expose a canonical “number of shards”; deriving an expectation would require an explicit policy (e.g. fixed federation registry or domain catalog). Until then, `view_health` uses **partial** when `expected` is `None` and there are zero active rows; **complete** when there is no expectation but at least one fresh row; **stale** when any row is past expiry.

## Tests (pwm-coding scope)

- Unit tests in `federation.rs`: merge behavior, TTL sweep, `view_health` labels, fallback shard key.

## Optimization note

- Federation logic lives in `federation.rs` instead of growing `api.rs`/`transport.rs` merge loops; transport only forwards trusted signals. Further cleanup could fold heartbeat merge helpers to reduce duplicated `merge_remote_hb` call sites.

## CQDS index rebuild

- Background `rebuild_index` for project **5** was **not** enqueued: MCP `user-cqds_mcp_mini` connection failed from this environment.
