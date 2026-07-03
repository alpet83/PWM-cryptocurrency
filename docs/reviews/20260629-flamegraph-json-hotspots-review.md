# Analysis: JSON/string hotspots on pwmd hot paths (37c2ae8)

- date: 2026-06-29
- ticket: `20260629-flamegraph-json-hotspots-review`
- baseline commit: `37c2ae8` (MSVC ramp flamegraph context)
- scope: `crates/pwmd/src/` — lifecycle, pipeline/worker, api handlers, transport wire
- type: read-only hotspot analysis (no code changes)

## 1. Scope recap

Flamegraph under ramp load shows string/JSON dominance. Ticket asks for a prioritized inventory of `serde_json`, `String`/`format!`, and unguarded logging on:

- seal loop (`spawn_seal_loop`)
- tx validation (`worker.rs`, `first_bad_tx_ctx`)
- RPC (`/v1/tx`, status)
- P2P codec (`transport/peer_session/wire.rs`)
- `block_timing` JSONL

Evidence: static read of `37c2ae8` tree; `cargo` profiling not re-run in this session.

## 2. Prioritized hotspot table

| Priority | Location | Hot-path frequency | Description | Suggested fix direction |
|----------|----------|-------------------|-------------|-------------------------|
| **High** | `transport/peer_session/wire.rs:193-197` | Every P2P frame (heartbeat, cluster, sync) | `serde_json::to_vec` on full `PeerWireMsg` enum; length-prefixed JSON wire | Binary/framed codec (bincode/postcard) or pre-sized buffer pool; split “control JSON” from block payloads |
| **High** | `transport/peer_session/mod.rs:684-716`, `:759-763` | Cluster propose per attester × heartbeat (~1–1.5s) | `mk_cluster_prop` clones `State`, embeds `tail_blocks: Vec<SyncBlockWire>` with `block: Some(blk.clone())` — full blocks+txs serialized to JSON on wire | Send hash/refs only on steady propose; cap/zero `tail_blocks` when tip caught up; separate compact `ClusterProposeV2` without inline `Block` |
| **High** | `transport/peer_session/wire.rs:223-235` | Every inbound frame | `read_wire_msg` allocates `Vec<u8>` payload + `serde_json::from_slice` | Reuse payload buffer; zero-copy parse for fixed headers; reject/defer large JSON on hot path |
| **High** | `lifecycle.rs:881-904` | Every sealed tx (INFO, unconditional) | `log_tx_commit_delta` — per-tx `hex::encode` ×2 + `info!` with formatting at default INFO level | Gate behind `tracing::enabled!(Level::INFO)` or sample (1/N); move to `debug!`; structured fields without hex unless enabled |
| **High** | `api/handlers_tx.rs:30`, `:70-77` + `pipeline/worker.rs:341-343` | Every `POST /v1/tx` (ramp) | Axum `Json<SignedTx>` deserialize + `validate_tx_shape` in handler, then **again** in worker `precheck_client` | Skip handler `validate_tx_shape` when routing to worker (worker-only validation); or cache sig-check bit on job |
| **High** | `lifecycle.rs:1771-1808`, `:1994-1995` + `block_timing.rs:131-189` | Seal poll every 10ms while gate pending; once per non-empty seal | `seal_pt.checkpoint*` inserts `name.to_string()` into maps each poll; on seal `json_stats_with_precision` builds JSON `String` for `profile_json` | Use `&'static str` checkpoint keys (no alloc); checkpoint only on seal attempt not every poll; lazy profile JSON or numeric arrays |
| **High** | `block_timing.rs:920-933`, `:937-956` | Per non-empty sealed block (+ flush polls) | `serde_json::to_string(row)` + `sync_all`; `trim_jsonl_tail` **reads entire JSONL** when >1500 lines | Append-only without full-file trim on hot path; rotate files; defer trim to background thread; confirm empty-block skip (`lifecycle.rs:1972`) — **empty blocks skip `note_seal`** ✓ |
| **Med** | `api/common.rs:263-283` | Every rejected `POST /v1/tx` | `tx_reject_json` — `json!` + `.to_string()` + `format!` message + `tx_id_hex` | Static error templates; numeric codes only; avoid full JSON body on 4xx |
| **Med** | `api/common.rs:348-364` | Direct-seal / roaming RPC paths | `push_tx_flow` — `format!`, duplicate `hex::encode`, `tx_id_hex` per acceptance | Store fixed-size tx hash bytes in flow row; format on export only |
| **Med** | `lifecycle.rs:861-865` | Seal failure / replay (`first_bad_tx_ctx`) | Per-tx `apply_tx_with_ctx`; `err.to_string()` on first bad tx | Return `&'static str` / error code from `TxError`; avoid full Display string unless logging |
| **Med** | `transport/peer_session/mod.rs:690-691`, `:838-840` | Each cluster propose/attest | `hex::encode` for hashes/signatures into `String` fields before JSON | Fixed-size byte arrays on wire with hex only at debug boundary |
| **Med** | `lifecycle.rs:1490-1491` | Every seal poll (~100/s at 10ms) | `block_timing::try_flush_once` — may serialize `pending.json` (`to_string_pretty`) under lock | Flush on seal event only; coalesce pending writes; skip flush when queue empty (partially done) |
| **Med** | `api/handlers_status.rs:216-237`, `:243-303` | `/v1/status` polling (soak dashboards) | Many `.to_string()` on lease enums + full `Json(StatusOut)` serialize (`types.rs` `u128` → decimal strings) | Lean status DTO for hot poll; numeric fields as integers; split “heavy diagnostics” endpoint |
| **Med** | `api/common.rs:426-463` | `/v1/account/*` | Multiple `u128::to_string()` per account view | Serde decimal-as-string only at JSON boundary once; reuse buffer in handler |
| **Low** | `lifecycle.rs:871-877` | Sealed blocks, debug logger | `log_tx_debug` — hex per tx/account | Already debug path via custom logger; ensure default off in ramp |
| **Low** | `lifecycle.rs:1948-1949` | Every 10th block | `info!("sealed height={}")` | Fine; keep |
| **Low** | `lifecycle.rs:2045-2048` | Every 100 blocks | `seal_cadence_drift` formatted `info!` | Fine cadence |
| **Low** | `pipeline/worker.rs:434` | Worker precheck errors | `err.to_string()` for `TxRejectReason` | Map `TxError` → compact enum for worker reply |
| **Low** | `perfmon.rs` | Instrumented paths | No JSON — atomics only | No action |

## 3. Focus-area answers

### 1. RPC handler

- **Deserialize:** yes — `Json<SignedTx>` on every `POST /v1/tx` (`handlers_tx.rs:30`).
- **Re-serialize response:** pipeline path returns `204 No Content` — no response JSON on success (`:241`). Errors return `(StatusCode, String)` plain text body, often JSON **strings** built manually (`tx_reject_json`).
- **Tight loop:** ramp driver hammers `/v1/tx`; double validation (handler + worker) is the main redundant cost.

### 2. Seal loop

- No JSON in core seal path except `block_timing` profile + JSONL (non-empty blocks only after `e9b3f7e` guard at `lifecycle.rs:1972`).
- **Heavy strings:** per-tx `info!` in `log_tx_commit_delta` (`:895-904`); `seal_pt` checkpoint map growth every poll (`:1771-1808`); `summary_log_line` every `SUMMARY_BLOCK_INTERVAL` blocks (`:2001-2002`).

### 3. Tx validation pipeline

- Worker: sync `validate_tx_shape` — crypto, not JSON (`worker.rs:341-343`).
- `first_bad_tx_ctx`: sim `apply_tx_with_ctx` per tx + `err.to_string()` on failure (`lifecycle.rs:861-865`) — allocation on **error** path only, but replay can scan many txs under eviction storms.

### 4. P2P codec

- **JSON**, not binary — `PeerWireMsg` serde JSON (`wire.rs:74-186`, encode/decode `:193-235`).
- **Per block (cluster):** propose + attest round-trip per attester; propose may embed multiple full `Block` values in `tail_blocks` — dominant JSON cost under load.

### 5. Logging

- `log_tx_commit_delta` — **not** behind level guard; runs at INFO for every tx in every sealed block.
- Many `info!`/`warn!` in seal skip paths use `format!`/`Display` — lower frequency than per-tx commit logs.
- `tracing` structured fields still evaluate `%hex::encode(...)` when branch taken (`handlers_tx.rs:53-67` on Import).

### 6. block_timing / empty blocks

- `note_seal` gated by `!txs.is_empty()` (`lifecycle.rs:1972-1998`) — **no JSONL row / no `profile_json` serialize on empty seal** ✓.
- `note_t0` / `note_gate_ok` still enqueue on all slots; `try_flush_once` every poll may still touch pending JSON (`lifecycle.rs:1490-1491`, `block_timing.rs:867-868` pretty JSON).

### 7. Buffer reuse opportunities

| pattern | where | idea |
|---------|-------|------|
| `Vec<u8>` payload | `wire.rs:223` | Per-connection read buffer reused |
| `Vec` framing | `wire.rs:195-197` | `BytesMut` / pool |
| `ProfileTime` maps | `block_timing.rs:135-145` | Static keys, clear per seal turn |
| `tail_blocks` clone | `mod.rs:696-707` | Arc<Block> or header-only wire |
| JSONL trim | `block_timing.rs:937-956` | Ring file / segment rotation |

## 4. Requirements fit

Analysis deliverable matches ticket: prioritized High/Med/Low table with file:line and fix directions. No code edits in this slice.

### Wire JSON / u128

Wire JSON / u128: **in scope for analysis** — P2P `PeerWireMsg` and RPC `StatusOut`/`AcctOut` use JSON with `u128` as decimal strings (`api/types.rs:430-434`). Confirmed wire-safe pattern on API; peer `AccountViews` / `CrossShardFacts` on JSON wire remain a known perf + correctness surface (see prior `wire_decode_failed` reviews).

## 5. Concurrency / parallelism

Hotspot work is mostly **per-thread alloc** (seal loop task, worker threads, peer session tasks) — no new races from JSON itself. Shared `block_timing` file lock + `try_flush_once` on seal poll can contend across tasks writing pending JSON. P2P encode on peer session task while seal holds chain lock is independent but competes for CPU — aligns with flamegraph “string under load” symptom.

## 6. Recommended fix order (for coding slices)

1. **P2P propose payload diet** — drop inline `Block` from steady-state `ClusterPropose` (largest JSON blob).
2. **Disable or downgrade `log_tx_commit_delta`** at INFO during ramp/production default.
3. **Remove duplicate `validate_tx_shape`** on worker pipeline path.
4. **Seal profile checkpoints** — stop per-poll `String` map inserts; serialize profile only when timing enabled + non-empty seal.
5. **block_timing I/O** — async trim / file rotation; avoid `read_to_string` on every append.
6. **Wire codec v2** — binary frames (longer-term).

## 7. Verdict

**Approve** (analysis complete) — flamegraph correlation plausible; highest ROI is cluster wire JSON with embedded blocks, per-tx INFO logging, and duplicate tx validation. Empty-block JSONL suppression from `e9b3f7e` is confirmed in tree.

## 8. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260629-flamegraph-json-hotspots-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 32000, "confidence": "medium" }`