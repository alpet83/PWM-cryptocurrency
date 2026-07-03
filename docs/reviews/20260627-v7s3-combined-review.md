# V7-S3 combined review — commit 394d554 (hex + async writer + eviction hardening)

- date: 2026-06-27
- ticket: `20260627-v7s3-combined-review`
- commit: `394d554`
- coding_tickets: `20260627-v7s3-fs-perf-hex-async-append`, `20260627-v7s3-eviction-fix-dedup-skipset`
- normative: `docs/tickets/v7-s3-fs-perf-hex-async-append.md`, `docs/reviews/20260627-eviction-loop-investigation.md` §5

## 1. Scope recap

Single commit merges:

1. **FS perf** — `sig64`/`opt_hex32` hex JSON, `tx.rs`/`block.rs` annotations, true append in `incremental.rs`, dedicated `BlockWriter` thread.
2. **Eviction hardening** — `dedup_seal_entries`, `evicted_hashes` skip-set, `next_seal_time_ms` poll-tick bump on seal `Err`.

| file | change |
|------|--------|
| `pwm-core/src/ser_bin.rs` | `sig64` hex + legacy `visit_seq`; new `opt_hex32` |
| `pwm-core/src/tx.rs` | binary field `#[serde(with=...)]`; round-trip tests |
| `pwmd/src/snapshot/incremental.rs` | O(1) tail `read_last_block_height`, append-only epoch write |
| `pwmd/src/block_writer.rs` | NEW — FIFO sync channel + writer thread |
| `pwmd/src/lifecycle.rs` | dedup/skip-set/deadline bump; `enqueue_sealed_block` |
| `pwmd/src/bootstrap.rs`, `state.rs`, `handlers_shutdown.rs` | writer integration |

## 2. Requirements fit

| Focus area | Verdict | Evidence |
|------------|---------|----------|
| `sig64` hex + legacy compat | **PASS** | `ser_bin.rs:138-211` — human-readable hex serialize; `deserialize_any` + `visit_str`/`visit_seq`; tests `sig64_json_hex_roundtrip`, `sig64_legacy_array_works` |
| `opt_hex32` None/Some/legacy | **PASS** | `ser_bin.rs:81-136` — `visit_none`, `visit_some`→`Hex32Visitor`; tests `opt_hex32_json_roundtrip`, `opt_hex32_legacy_array_works` |
| `tx.rs` binary annotations | **PASS** | `signer_pk`, `signature`, all `to`, `batch_root`, `registry_sig`, `export_id`, `target_account`, `beneficiary`, `activation_target`, `company_metadata_commitment`, `rescue_address` annotated (`tx.rs:76-389`). **Note:** `Stake` has no `beneficiary` (ticket typo); beneficiary is `BurnMark` only |
| `block_writer` shutdown / full channel | **PASS** | `shutdown()` sends `Shutdown`, drops sender, awaits reply + `join` (`block_writer.rs:62-82`); idempotent test `shutdown_is_idempotent`. Full channel: `try_send` → blocking `send` (`98-105`) |
| `try_send` fallback sync write | **PASS** | `enqueue_sealed_block` (`lifecycle.rs:997-1011`) → `recover_append` sync path on enqueue failure |
| O(1) append / tail read | **PASS** | `append_block_for_epoch` (`incremental.rs:21-97`) — no full-file rewrite; `read_last_block_height` tail window 128KiB (`99-144`); tests `append_continuity`, `tail_window_bounds` |
| `dedup_seal_entries` before seal | **PASS** | `lifecycle.rs:1888-1894` — `skip_evicted_entries` then `dedup_seal_entries` then `seal_entries` |
| `evicted_hashes` reset on height | **PASS** | Clear on tip change before assembly (`1853-1856`) and on `Ok(seal)` (`1907-1908`); insert on eviction (`2046`) |
| Deadline bump on eviction `Err` | **PASS** | `2068-2071` (and ctx-fail `2037-2040`) — `next_seal_time_ms = now + SEAL_POLL_INTERVAL_MS`; success path still uses grid `scheduled_next` (`1902`) |
| Tests listed in ticket | **PASS** | `tx_json_hex_round_trip`, `tx_json_legacy_arr_compat` (`tx.rs:1197-1231`); `dedup_seal_entries_removes_dup`, `eviction_skip_set` (`lifecycle.rs:2789-2814`); `mpool_undo_bad_seal` unchanged (`mempool.rs:98-115`) |
| `cargo test -p pwm-core/pwmd` | **UNVERIFIED** | Shell unavailable in review session |

## 3. Style and module shape

- New modules follow existing patterns; production fn names within ≤4-word policy (`dedup_seal_entries`, `read_last_block_height`, `enqueue_sealed_block`).
- `block_writer.rs` has minimal `//!` banner — acceptable for new 204-line module.
- Entity segment script not run (shell unavailable).

### Wire JSON / u128

**Scope:** yes — epoch JSONL blocks and HTTP/RPC `SignedTx` JSON are peer- and client-facing.

| area | assessment |
|------|------------|
| `u128` amounts/fees | **OK** — `ser_json_u128` on `Transfer`, `Export`, `Policy`, etc. |
| binary fields on `SignedTx`/`BlockHdr` | **OK** — `hex32`/`sig64`/`opt_hex32` |
| **gap** | `ExportProvenance.to` (`state.rs:79-84`) embedded in `SignedTx.import_provenance` — still serializes as JSON byte array (no `hex32`). Pre-existing shape; not annotated in this slice |

## 4. Safety

1. **Evicted tx discard** — skip-set consumes validated-queue entries and drops them from batch; bad nonce txs leave pipeline intentionally (matches investigation §5).

2. **Writer error coalescing** — `writer_loop` keeps first `pending_error` but continues appending after failure (`block_writer.rs:109-118`). Later blocks may be lost silently until flush surfaces first error. **Medium nit** — consider fail-fast or per-block error propagation.

3. **Blocking `mpsc::send` from async seal task** — when queue (cap 200) is full, `enqueue` blocks the Tokio worker thread (`block_writer.rs:101-103`). Acceptable backpressure but worth monitoring under burst seal.

4. **Writer disabled on spawn failure** — `bootstrap.rs:29-38` logs warn and sets `None`; seal still works via `recover_append` fallback.

5. **No `Drop` on `BlockWriter`** — relies on `handlers_shutdown.rs:38-44` flush+shutdown; process kill may orphan thread. Low nit.

## 5. Tests

| test | covers |
|------|--------|
| `sig64_json_hex_roundtrip`, `sig64_legacy_array_works` | ser_bin |
| `opt_hex32_json_roundtrip`, `opt_hex32_legacy_array_works` | ser_bin |
| `tx_json_hex_round_trip`, `tx_json_legacy_arr_compat` | SignedTx wire |
| `hdr_json_hex_str`, `hdr_json_legacy_arr` | BlockHdr (`block.rs`) |
| `append_continuity`, `rejects_duplicate_gap`, `tail_window_bounds`, `legacy_arrays_read` | incremental O(1) |
| `preserves_append_order`, `flushes_pending_blocks`, `shutdown_is_idempotent` | BlockWriter |
| `dedup_seal_entries_removes_dup`, `eviction_skip_set` | lifecycle helpers |

**Gaps:** no integration test seal-loop eviction + skip-set under concurrent validated enqueue; no test for `enqueue`→`recover_append` fallback path; no test asserting deadline bump breaks microcycle count.

## 6. Concurrency / parallelism

**Components:** `BlockWriter` OS thread + `SyncSender`; seal async task `enqueue`/`flush`; `inner.write()` during seal; `evicted_hashes` on seal task stack only.

| Hazard | Assessment |
|--------|------------|
| Writer vs seal ordering | **Safe** — single FIFO thread; `flush` barrier before autosnap (`lifecycle.rs:986-989`) |
| `Arc<Mutex<WriterState>>` in `enqueue`/`shutdown` | **OK** — short-held; no await under lock |
| Blocking send in async context | **Medium** — queue-full blocks Tokio worker |
| `evicted_hashes` | **Single-task** — no cross-thread sharing ✓ |
| Skip-set + validated drain | **Correct discard** — `try_recv` removes evicted txs from channel without requeue |

## 7. Findings (prioritized)

### Medium

1. **Writer continues after append error** — `block_writer.rs:113-118` may leave epoch file inconsistent while queue drains; only first error returned on flush.

2. **Blocking backpressure on full writer queue** — may stall seal loop under >200 pending blocks or slow disk.

3. **`ExportProvenance.to` still byte-array JSON** — nested in `SignedTx.import_provenance`; inconsistent with hex migration goal for tx wire.

### Low

4. **`first_bad_tx_ctx` still Raw-only simulation** — pre-existing; eviction index may diverge for PreValidated-heavy batches.

5. **Eviction deadline bump shifts grid by one poll tick** — self-heals on next `Ok(seal)` via `scheduled_next`; acceptable.

6. **`BlockWriter` lacks `Drop` shutdown** — shutdown path covered; abrupt exit nit.

## 8. Verdict

**Approve with nits** — commit satisfies both tickets: hex encoding with legacy compat, true append + async writer with sync recovery, and eviction hardening per investigation §5. Tests cover the requested unit cases. Medium nits: writer error handling after first failure, async blocking on full queue, and `ExportProvenance` hex gap.

## 9. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260627-v7s3-combined-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 48000, "confidence": "medium" }`