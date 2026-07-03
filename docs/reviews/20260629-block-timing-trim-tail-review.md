# Review: block_timing tail-read trim (e8e8397)

- date: 2026-06-29
- ticket: `20260629-block-timing-trim-tail-review`
- coding_ticket: `20260629-block-timing-trim-tail`
- commit: `e8e8397`

## 1. Scope recap

Review commit `e8e8397` — `trim_jsonl_tail` in `crates/pwmd/src/block_timing.rs`:

| area | change |
|------|--------|
| `JSONL_TAIL_BYTES` | `4096` constant (`:13`) |
| `trim_jsonl_tail` | Metadata length; tail read when `file_len > 4096` (`:940-952`) |
| `read_tail_jsonl` | Seek `max(0, len-4096)`, align to line boundaries (`:963-1000`) |
| `read_full_jsonl` | Extracted fallback (`:954-961`) |
| `trim_raw_jsonl` | Shared trim logic (`:1002-1016`) |

Called from `append_jsonl` after every non-empty seal row (`:937`).

## 2. Requirements fit

| Focus area | Verdict | Evidence |
|------------|---------|----------|
| Seek `max(0, file_len - 4096)` | **PASS** | `saturating_sub` (`:964`); no underflow |
| Tail line-boundary alignment | **PASS** | Skip partial first line when `start > 0` (`:985-990`); drop incomplete last line (`:992-998`) |
| Small-file fallback | **PASS** | `file_len <= JSONL_TAIL_BYTES` → `read_full_jsonl` (`:942-943`) |
| Seek/read error fallback | **PASS** | `read_tail_jsonl` `Err` → full read (`:946-948`) |
| Trim correctness (large files) | **FAIL** | Tail slice line count ≠ file line count — see §3 |
| Future-removal NOTE | **PASS** | Comment at `:945` |

## 3. Trim correctness — blocker

`trim_raw_jsonl` decides whether to rewrite the file using **only** the `raw` string passed in:

```1002:1006:crates/pwmd/src/block_timing.rs
fn trim_raw_jsonl(path: &Path, raw: &str, max_rows: usize) -> Result<(), String> {
    let lines: Vec<&str> = raw.lines().collect();
    if lines.len() <= max_rows {
        return Ok(());
    }
```

For `file_len > 4096`, `raw` is **at most ~4KB of complete lines** from the file tail (`read_tail_jsonl`), not the full JSONL.

**Failure mode:** CY lab JSONL with `max_rows = 1500` (production constant in `append_jsonl`). Typical `RowOut` lines are hundreds of bytes to a few KB. A file with **>1500 rows** and size **>4KB** (always true well before row cap) yields a tail containing only **a handful of lines** (often ≪ 1500).

Then `lines.len() <= max_rows` → function returns `Ok(())` **without trimming**, even though the file may have thousands of rows. The **1500-row cap invariant is broken** for all large files.

**Secondary hazard:** If a dense tail ever contained `> max_rows` lines (only plausible with `max_rows` ≪ 1500 or tiny lines), `fs::write` would persist **only the last `max_rows` lines of the tail chunk**, discarding the rest of the file — data loss.

**Test gap:** `jsonl_tail_keeps_latest` (`:1150-1172`) seeds 15 short lines (~hundreds of bytes total) — always exercises **full-read path**, not tail-read.

### Suggested fix direction

Keep perf win without breaking semantics — pick one:

1. **Two-phase decision:** Use tail read only to detect “definitely under cap” (e.g. `file_len <= max_rows * MIN_LINE_BYTES`); otherwise newline-count scan or full read before trim.
2. **Streaming line count:** `BufReader` seek-to-start, increment newline count without `read_to_string` whole file.
3. **Rotate / segment files** instead of in-place trim (aligns with NOTE about future JSONL removal).

## 4. `read_tail_jsonl` mechanics (non-blockers)

| check | result |
|-------|--------|
| `saturating_sub` | OK for `file_len < 4096` → `start=0`, no partial-line skip |
| No `\n` in tail when `start > 0` | `Err` → full-read fallback (`:946-948`) |
| `from_utf8_lossy` | Acceptable for JSONL; nit if non-UTF8 ever written |
| Panics | None — errors are `Result<String, String>` |

## 5. Style and module shape

- `use std::io::{Read, Seek, SeekFrom, Write}` — appropriate.
- Helpers split cleanly: `read_full_jsonl`, `read_tail_jsonl`, `trim_raw_jsonl`.
- No new dependencies.

### Wire JSON / u128

Wire JSON / u128: not applicable (local JSONL disk format only).

## 6. Safety

- No panics on I/O failure paths.
- Incorrect trim skip is a **resource / observability** issue (unbounded JSONL growth), not consensus.

## 7. Tests

| test | covers tail path? |
|------|-------------------|
| `jsonl_tail_keeps_latest` | **No** — file < 4KB |
| Large-file trim invariant | **Missing** |

`cargo test -p pwmd block_timing`: **UNVERIFIED**.

## 8. Concurrency / parallelism

`append_jsonl` runs under `block_timing` file lock from seal flush path. Tail-read does not introduce new shared-state races; separate open/read after append is same as before. No concurrency blocker — correctness issue is sequential logic.

## 9. Verdict

**Request changes**

**BLOCKER-1:** Tail-only line count must not gate trim for `max_rows=1500` when `file_len > 4096` — cap enforcement silently stops.

**TEST-1:** Add test: seed JSONL >4KB with >`max_rows` lines (short rows), assert trim reduces to `max_rows`.

**NIT-1:** Log or metric when falling back from tail to full read (debug visibility).

NOTE comment at `:945` is present as requested.

## 10. Participation

- `agent`: `pwm-review`
- `result`: `FAIL`
- `artifacts`: `docs/reviews/20260629-block-timing-trim-tail-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 20000, "confidence": "high" }`