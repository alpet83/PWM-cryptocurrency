# V7-S3 fixes review — 85dbcae + eec711d

- date: 2026-06-27
- ticket: `20260627-v7s3-fixes-review`
- commits: `85dbcae`, `eec711d`
- prior review: `docs/reviews/20260627-v7s3-combined-review.md` (medium nits addressed)

## 1. Scope recap

Two follow-up commits fixing V7-S3 regressions identified in combined review:

| commit | fix |
|--------|-----|
| `85dbcae` | BlockWriter fail-fast after first append error; `ExportProvenance.to` → `hex32` |
| `eec711d` | `sync_epoch_to_tip` before `BlockWriter` init; `cleanup_empty_gap_file`; `epoch_gap_mid_start` test |

## 2. Requirements fit

| Focus area | Verdict | Evidence |
|------------|---------|----------|
| Writer fail-fast after append error | **PASS** | `block_writer.rs:113-125` — skip `Append` when `pending_error` set; warn per skipped block |
| Flush/shutdown retain error state | **PASS** | `128-131` — `pending_error.clone()` (not `take()`); repeated flush/shutdown still report failure |
| `ExportProvenance.to` hex + legacy | **PASS** | `state.rs:79-84` — `#[serde(with = "hex32")]`; test `export_prov_json_hex` (`2538-2559`) — hex round-trip + legacy array |
| `sync_epoch_to_tip` before writer reinit | **PASS** | `lifecycle.rs:2453-2460` — sync under read lock, then `BlockWriter::new` |
| Bootstrap resume sync guard | **PASS** | `bootstrap.rs:314-316` — only when manifest exists (genesis cold-start skipped) |
| Epoch gap regression test | **PASS** | `incremental.rs:622-645` — `epoch_gap_mid_start`: sync 29 blocks, writer appends 30 |
| `writer_stops_after_error` test | **PASS** | `block_writer.rs:211-238` — enqueue [1,3,2], flush/shutdown fail, disk has only height 1 |
| `cargo test` | **UNVERIFIED** | Shell unavailable in review session |

## 3. Commit analysis

### 85dbcae — writer fail-fast + provenance hex

**BlockWriter** closes combined-review nit #1 (silent append after first disk error). After first `append_block_for_epoch` failure, `pending_error` is set permanently for the writer lifetime; subsequent appends are skipped with `tracing::warn` (`height`, `error`). `Flush`/`Shutdown` propagate the stored error via `clone()`, so `periodic_snap_save` flush-before-autosnap and shutdown path both observe failure.

**Caveat (by design):** `enqueue()` still returns `Ok(())` for blocks skipped in the writer thread — seal path only learns on `flush()`. Acceptable given existing flush barrier before autosnap (`lifecycle.rs:986-989`).

**ExportProvenance** closes combined-review nit #3. `to` serializes as 64-char hex; deserialize accepts hex string and legacy byte array via `hex32` visitor.

### eec711d — epoch gap before BlockWriter

**Root cause:** `BlockWriter` starts with in-memory chain ahead of epoch JSONL on disk. First enqueued block may be height N while epoch file expects `first_h` or `prev_h` continuity → append gap error.

**Fix paths:**

1. **Lifecycle reinit** (`lifecycle.rs:2453-2456`) — `sync_epoch_to_tip` drains missing heights from `chain.blocks` tail before spawning new writer.
2. **Bootstrap resume** (`bootstrap.rs:314-316`) — same sync when snapshot loaded and manifest already exists.
3. **`cleanup_empty_gap_file`** (`incremental.rs:100-114`) — removes zero-byte epoch file left from interrupted append; called on gap detection (`32-36`).

## 4. Style and module shape

- Changes are minimal and localized; no new long production identifiers.
- `cleanup_empty_gap_file` is 4-word fn name — within policy.

### Wire JSON / u128

**Scope:** yes — `ExportProvenance` embedded in `SignedTx.import_provenance` JSON.

| field | assessment |
|-------|------------|
| `ExportProvenance.to` | **OK** — `hex32` on human-readable JSON |
| `ExportProvenance.amount` | **OK** — existing `ser_json_u128` unchanged |

## 5. Safety

1. **Fail-fast prevents silent epoch corruption** — better than pre-85dbcae continue-after-error behavior.
2. **`recover_append` fallback unchanged** — `enqueue_sealed_block` still sync-writes on channel failure (`lifecycle.rs:1001-1009`).
3. **`cleanup_empty_gap_file`** only deletes `meta.len() == 0` files — non-empty gap files still fail loudly.

## 6. Tests

| test | covers |
|------|--------|
| `writer_stops_after_error` | fail-fast skip + flush/shutdown error |
| `export_prov_json_hex` | provenance hex + legacy |
| `epoch_gap_mid_start` | sync + writer mid-epoch start |

**Gaps:** no test for `cleanup_empty_gap_file` (empty vs non-empty); no test bootstrap resume path with manifest guard.

## 7. Concurrency / parallelism

**Components:** BlockWriter OS thread; `sync_epoch_to_tip` under `inner.read()` on lifecycle reinit; bootstrap sync before `RwLock` wrap.

| Hazard | Assessment |
|--------|------------|
| Read lock held during `sync_epoch_to_tip` I/O | **Medium nit** — `lifecycle.rs:2454-2455`; blocks writers during restart if many tail blocks missing; bounded by `TAIL_BLOCK_CAP` in practice |
| Fail-fast skip + concurrent enqueue | **OK** — FIFO thread serializes; skipped blocks not written |
| Blocking `mpsc::send` on full queue | **Residual** — not changed in these commits (pre-existing) |

## 8. Findings (prioritized)

### Medium

1. **`inner.read()` during disk sync on reinit** — `lifecycle.rs:2454-2455` holds async read lock across potentially many `append_block_for_epoch` calls. Prefer clone missing blocks, drop lock, then sync (deferred optimization).

### Low

2. **`BlockWriter` still no `Drop` shutdown** — pre-existing; shutdown path in `handlers_shutdown` covers graceful exit.

3. **`cleanup_empty_gap_file` untested** — only exercised indirectly; no assertion for non-empty gap file left in place.

4. **Enqueue succeeds while writer in failed state** — callers must flush to detect; documented by fail-fast design.

## 9. Verdict

**Approve with nits** — both commits correctly fix real V7-S3 regressions (writer silent corruption + epoch gap on writer start). Tests cover the critical paths. Medium lock-scope nit on reinit is non-blocking at current scale.

## 10. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260627-v7s3-fixes-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 32000, "confidence": "medium" }`