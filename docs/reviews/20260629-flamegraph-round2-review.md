# Analysis: flamegraph round 2 — remaining hotspots after perf fixes (2d9b8d4)

- date: 2026-06-29
- ticket: `20260629-flamegraph-round2-review`
- baseline commit: `2d9b8d4` (branch `mvp-v7`, second ramp context)
- prior analysis: `docs/reviews/20260629-flamegraph-json-hotspots-review.md` (`37c2ae8`)
- scope: `crates/pwmd/src/` — lifecycle, pipeline/worker, api handlers, transport wire
- type: read-only hotspot re-analysis (no code changes)
- profile: `profile.json.gz` at project root — present; terminal unavailable in this session so stacks not re-sampled (ticket notes Jun 28 mtime = structural reference only)

## 1. Round-1 High items — gone vs still present

| Round-1 High | Status at `2d9b8d4` | Notes |
|--------------|---------------------|-------|
| `log_tx_commit_delta` INFO + hex per tx | **GONE** | `tracing::enabled!(DEBUG)` guard + `debug!` (`lifecycle.rs:881-912`) |
| Duplicate `validate_tx_shape` (handler + worker) | **GONE** (pipeline) | Handler pipeline path → `run_worker_precheck` only (`handlers_tx.rs:258-260`); worker `precheck_client` (`worker.rs:341-344`). Direct-seal path still validates in handler (`handlers_tx.rs:124-134`) |
| `seal_pt` checkpoint `String` keys | **GONE** | `BTreeMap<&'static str, _>` (`block_timing.rs:114-145`); lifecycle literals e.g. `"lease_gate_begin"` (`lifecycle.rs:1776-1813`) |
| `ClusterPropose` full `tail_blocks` + `Block` clone | **GONE (default)** | `full_blocks=false` → empty `tail_blocks` (`mod.rs:698-716`, `config.rs:62-80`). Opt-in `--cluster-propose-full-blocks` |
| `trim_jsonl_tail` full-file read | **IMPROVED** | Tail-read 4KB when `file_len > JSONL_TAIL_BYTES`; full read only when tail line count `< max_rows` (`block_timing.rs:940-951`) |
| P2P `serde_json::to_vec` / `from_slice` every frame | **STILL PRESENT** | `wire.rs:193-235` unchanged |
| `seal_pt.json_stats_with_precision` on non-empty seal | **STILL PRESENT** | `lifecycle.rs:1999-2000` + `block_timing.rs:153-192` |
| `block_timing` JSONL `to_string` + `sync_all` + poll `try_flush_once` | **STILL PRESENT (reduced)** | Append path (`block_timing.rs:922-937`); `try_flush_once` every seal poll (`lifecycle.rs:1495-1497`) |

**Net effect:** per-tx INFO logging, duplicate Ed25519 on ingress, propose blob JSON, and per-poll `String` checkpoint keys are off the hot path. Flamegraph “string/JSON under load” should shift toward **steady P2P JSON**, **seal-time state serialization**, and **RPC body parse** — consistent with user observation in Firefox Profiler.

## 2. Prioritized hotspot table (round 2)

| Priority | Location | Hot-path frequency | Description | Suggested fix direction |
|----------|----------|-------------------|-------------|-------------------------|
| **High** | `pipeline/worker.rs:341-344` + `pwm-core/tx.rs:618-631` | Every ramp `POST /v1/tx` (64/block) | Single `validate_tx_shape` → Ed25519 `verify_sig` per accepted tx; instrumented `PERF_ED25519` | Expand `precheck_hot` coverage; optional sig-cache keyed by `tx_hash` for replay/dedup; batch verify API if ramp sends bursts |
| **High** | `pwm-core/chain.rs:181-220`, `state.rs:160-162` | Every sealed block | `seal_entries`: `st.clone()` then per-tx `apply_prechecked_tx` loop; **`digest(st)` = full `bincode::serialize(st)`** dominates seal CPU vs reward/header | Incremental `state_root`; snapshot digest off hot path; avoid duplicate `st` clone (see lifecycle) |
| **High** | `transport/peer_session/wire.rs:193-235` | Every P2P frame | JSON encode/decode + fresh `Vec` alloc per frame | `BytesMut` reuse; slim enum variants; binary/postcard v2 for cluster+heartbeat |
| **High** | `api/handlers_tx.rs:48`, `:269` | Every pipeline `POST /v1/tx` | Axum `Json<SignedTx>` deserialize + `tx.clone()` into worker job | Raw body buffer + single deserialize in worker; or `Arc<SignedTx>` through queue |
| **High** | `transport/peer_session/mod.rs:147-184`, `steady_session.rs:196-209` | Per peer × `heartbeat_interval_ms` (capped to seal_ms, often ~1s) | Heartbeat JSON: tip, lease fields, optional `federation_gossip` (≤32 rows, ~4KB budget, `federation.rs:12-15`) + encode; ClusterPropose/Attest on same loop | Strip lease/gossip from steady heartbeat; binary heartbeat; attest without hex `String` fields (`mod.rs:844-851`) |
| **Med** | `lifecycle.rs:1907`, `:1918` | Every seal attempt (incl. failures) | `st_before = g.chain.st.clone()` for debug delta logging; success path `Arc::new(g.chain.st.clone())` — **full state clone even when `log_tx_commit_delta` is DEBUG-gated** | Clone `st_before` only when `tracing::enabled!(DEBUG)`; share `Arc<State>` snapshot without second clone |
| **Med** | `lifecycle.rs:1776-1813`, `:1495-1497` | Seal poll ~100/s (`SEAL_POLL_INTERVAL_MS=10`) | `seal_pt.checkpoint*` BTreeMap inserts every poll; `try_flush_once` may read/write `pending.json` | Checkpoint only on seal turn start/end; flush on seal event or 1 Hz coalesce |
| **Med** | `lifecycle.rs:1999-2000`, `block_timing.rs:153-192`, `:922-937` | Per non-empty sealed block | `json_stats_with_precision` builds profile JSON `String`; JSONL row `to_string` + `sync_all` + trim | Numeric arrays in JSONL; defer `profile_json` to export; async trim / rotate |
| **Med** | `transport/peer_session/mod.rs:684-691`, `:696-697` | Cluster propose when proposer (≤1× heartbeat) | Epoch boundary `g.chain.st.clone()` for `active_validator_indices`; `hex::encode` tip hash + `format!("vo1:...")` vote | Cache prod idx / vote template; avoid full state clone for index pick |
| **Med** | `pwm-core/chain.rs:187-200` | 64 tx/block | `apply_prechecked_tx` skips Ed25519 (`state.rs:386-390`) but still runs shape + balance/nonce logic per tx | Already optimized vs raw path; profile should show apply << digest |
| **Low** | `pipeline/queue.rs:345-346`, `handlers_tx.rs:268-284` | Per RPC tx | `blocking_recv` worker + `oneshot` RPC wait — scheduling overhead under 64 tx/s | Usually noise; tune worker count / queue depth if profiler shows `tokio` |
| **Low** | `api/handlers_tx.rs:218-227` | Direct-seal path only | `info!` tx commit delta still at INFO for Export/Import | Align with pipeline: DEBUG guard |
| **Low** | `block.rs:52-69`, `chain.rs:231-232` | Per block | `txs_root` blake3 merkle + header `sign` — small vs state digest | No action unless blocks >>64 tx |

## 3. Focus-area answers

### 1. Ed25519 / `validate_tx_shape` (worker precheck)

- **Still the dominant per-tx CPU cost on ingress.** Pipeline path validates once in `precheck_client` (`worker.rs:341-344`); seal uses `SealEntry::PreValidated` → `apply_prechecked_tx` which calls `validate_shape_no_sig` only (`chain.rs:190-191`, `state.rs:386-390`).
- **`precheck_hot` fast-path** exists for simple `Transfer` when both accounts are in the hot index with `flags==0` (`worker.rs:354-367`) — skips `precheck_full` state reads but **still pays Ed25519** before the hot branch.
- **Dedup cache:** not implemented. Ramp driver sending distinct txs → no win; replay/eviction retries could benefit from `(tx_hash → sig_ok)` LRU.
- **Verdict:** Ed25519 remains #1 per accepted tx; removing duplicate handler check (round-1 fix) makes it more visible, not smaller.

### 2. P2P wire JSON after lean propose

- **Lean propose (`full_blocks=false`):** `tail_blocks` empty (`mod.rs:714-716`) — removes largest JSON blob from round-1.
- **What remains (steady load):**
  - **Heartbeat** ~every `min(heartbeat_interval_ms, seal_ms)` per peer (`lifecycle.rs:146-163`, default heartbeat 1500ms capped to seal). Payload: `unix_ms`, tip, lease strings, optional gossip (`wire.rs:86-105`, `mod.rs:147-184`). Gossip capped ~4KB (`federation.rs:12-15`).
  - **HeartbeatAck** — tiny (`wire.rs:106-108`).
  - **ClusterPropose** — lean: `height`, `vote_object`, `candidate_hash`, empty `tail_blocks` (~hundreds of bytes JSON).
  - **ClusterAttest** — `vote_object`, `candidate_hash`, `signature` hex string (`mod.rs:844-851`); one per attester per propose round-trip.
- **Codec tax unchanged:** every message still `serde_json::to_vec` / `from_slice` (`wire.rs:193-235`).
- **Frequency:** with cluster enabled, attester heartbeat capped to seal cadence (e.g. 1000ms @ 3600 bph) — ~1 encode+decode per peer per second, plus propose/attest bursts on seal ahead.

### 3. `chain.seal_entries` internals

Breakdown at `pwm-core/chain.rs:170-242` (pwmd calls at `lifecycle.rs:1911-1912`):

| Phase | Work | Relative cost (static) |
|-------|------|------------------------|
| Setup | Clone entries → `txs`; `st = self.st.clone()` | O(state) alloc |
| Epoch | `roll_epoch_if_needed`, `refund_exp_locks` | periodic |
| Apply loop | `apply_prechecked_tx` × N (64) — no Ed25519 | O(N × account ops) |
| Conservation | `drain_conservation_at_height` | policy-dependent |
| Reward | `compute_block_reward` + `reward_producer_v2` | small |
| Roots | `digest(st)` **bincode whole state**; `txs_root` blake3 | **digest likely dominates** |
| Header | `BlockHdr::sign` — one Ed25519 | tiny vs digest |

Pwmd wrapper adds: `st_before` clone (`lifecycle.rs:1907`), post-seal `st` clone into `Arc` (`:1918`), `json_stats` on non-empty block (`:1999-2000`).

### 4. Tokio runtime overhead

- Seal loop: 10ms deadline poll (`lifecycle.rs:61`, `:1495-1497`) — async sleep + `read().await` on gates.
- Workers: dedicated threads, `blocking_recv` (`worker.rs:307-312`) — avoids executor for precheck; RPC uses `oneshot` + `await` (`handlers_tx.rs:268-284`).
- Peer sessions: steady heartbeat loop (`steady_session.rs`) — concurrent with seal.
- At 64 tx/block and ~1 block/s, **expect secondary** unless profiler shows `tokio::runtime` / `poll` >5% — channel/oneshot cost << crypto + JSON + state digest.

### 5. RPC deserialization

- Handler still `Json<SignedTx>` (`handlers_tx.rs:48`) — full serde parse on every POST.
- With handler `validate_tx_shape` removed on pipeline path, **deserialize + `tx.clone()`** (`:269`) are the next RPC-side bottlenecks before worker queue.
- Success returns `204` — no response serialize (unchanged).

### 6. New hotspots visible after round-1 fixes

1. **`digest(st)` / bincode state serialize** — was masked by per-tx INFO hex logging.
2. **Unconditional `st_before` clone** — debug logging gated but clone is not (`lifecycle.rs:1907`).
3. **P2P steady heartbeat + attest JSON** — propose blob shrink makes these relatively larger in flamegraph.
4. **`seal_pt` per-poll checkpoints** — keys are static but inserts still run ~10/s through gate waits.

### 7. Recommended fix order — top 3 coding sprint

1. **Seal path state cost** — gate `st_before` clone; incremental `state_root` or move `digest` off hot path (`chain.rs:220`, `lifecycle.rs:1907-1918`). Highest block-rate ROI now that per-tx INFO is gone.
2. **P2P wire slimming** — buffer reuse + attest/heartbeat field diet (drop hex strings on wire, shrink gossip on steady path) before full binary codec (`wire.rs`, `mod.rs:147-184`, `:844-851`).
3. **Ingress crypto / parse** — sig-cache or expanded hot-path skip rules (`worker.rs`); optional single-parse `Arc<SignedTx>` from RPC (`handlers_tx.rs`).

(If ramp uses mostly unique txs, prioritize 1–2 over sig-cache.)

## 4. Profile evidence note

`profile.json.gz` exists at repo root (untracked). Shell analysis script `scripts/analyze_profile_hotspots.py` could not be run (terminal I/O error). Round-2 conclusions are **static** at `2d9b8d4`, aligned with round-1 Firefox Profiler symptom (string/JSON stacks). Re-run profiler after sprint 1–3 to confirm `bincode`/`digest` and heartbeat JSON move to top.

## 5. Concurrency

No new races from round-2 hotspots. `block_timing::try_flush_once` file lock + seal poll still contends with append on seal. P2P encode on peer tasks competes with seal-loop CPU — same as round-1.

## 6. Verdict

**Approve** (analysis complete) — round-1 High items largely addressed; remaining top consumers are **Ed25519 ingress**, **state digest on seal**, **P2P JSON codec**, and **RPC deserialize/clone**. Lean cluster propose confirmed default-off for `tail_blocks`.

## 7. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260629-flamegraph-round2-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 38000, "confidence": "medium" }`