# Review: lean ClusterPropose default + `--cluster-propose-full-blocks` (b1176ec)

- date: 2026-06-29
- ticket: `20260629-p2p-propose-tail-flag-review`
- coding_ticket: `20260629-p2p-propose-tail-flag`
- commit: `b1176ec`
- prior analysis: `docs/reviews/20260629-flamegraph-json-hotspots-review.md`

## 1. Scope recap

Review commit `b1176ec` — P2P propose JSON diet:

| area | change |
|------|--------|
| `main.rs` | `--cluster-propose-full-blocks` CLI (`default_value_t = false`, `SetTrue`) |
| `config.rs` | `ClusterCfg.full_blocks: bool` (default `false`) |
| `lifecycle.rs` | Startup `info!(full_blocks=..., "cluster propose mode")` |
| `transport/peer_session/mod.rs` | `mk_cluster_prop` branches on `full_blocks` |

## 2. Requirements fit

| Focus area | Verdict | Evidence |
|------------|---------|----------|
| CLI flag default false | **PASS** | `main.rs:333-335`; wired to `ClusterCfg.full_blocks` (`:672`) |
| Lean `tail_blocks` | **PASS** | `full_blocks == false` → `Vec::new()` (`mod.rs:708-709`); no `Block` clone |
| Full-blocks path preserved | **PASS** | `full_blocks == true` → prior tail depth + `block: Some(blk.clone())` (`:692-707`) |
| Attester empty tail | **PASS** | `filter_map` + `if !tail_batch.is_empty()` (`:960-997`); no panic |
| Startup log | **PASS** | `lifecycle.rs:2694-2697` when cluster enabled |
| Quorum binding fields | **PASS** | `vote_object`, `candidate_hash`, `height` in lean propose (`:690-718`); attest uses same (`:827-845`, `:1074-1097`) |
| Tests | **PASS** with nit | Production/wire tests use `tail_blocks: Vec::new()`; no `full_blocks` toggle test |

## 3. Propose path analysis

### `mk_cluster_prop` (`mod.rs:665-720`)

**Lean default (`full_blocks = false`):**

```708:709:crates/pwmd/src/transport/peer_session/mod.rs
    } else {
        Vec::new()
```

- Empty `tail_blocks` — **not** header-only rows with `block: None` (ticket allowed either; implementation chose empty vec).
- Wire JSON still carries `height`, `round`, `vote_object`, `candidate_hash` — sufficient for RFC16 attest binding (`cluster_sig_msg` at `:535-543`).
- `remote_tip_h` only consulted inside `full_blocks` branch — lean path ignores it (OK).

**Full-blocks flag (`full_blocks = true`):**

- Restores gap-based `tail_depth` and `block: Some(blk.clone())` — prior behavior.

**Residual work on every propose (nit):** `g.chain.st.clone()` for `pick_prod_idx` (`:684`) still runs in both modes — not introduced here, but propose path still allocates state copy.

### Attester receive (`mod.rs:958-1027`)

When `local_tip + 1 < height`:

1. Build `tail_batch` from `msg.tail_blocks` with `block: Some` only.
2. Lean propose → empty batch → **skips** `apply_cluster_tail_blocks`.
3. Continues to `record_cluster_prop` + `mk_cluster_attest` — **no assert on tail len**.

`apply_cluster_tail_blocks` handles empty slice (`sync_live.rs:1325-1326` → `Ok(0)`).

**Sync fallback:** Tail apply was a **catch-up shortcut**, not the only sync path. Attesters still sync via heartbeat / `sync_live` (existing CY behavior). Proposer seal gate uses `count_sync_ready_attesters` / cluster gate — unchanged by this flag.

### Quorum correctness

| field | lean propose | used in attest |
|-------|--------------|----------------|
| `height` | ✓ next seal target | `cluster_sig_msg`, round key |
| `vote_object` | `vo1:{height}:{tip_hash}` | must match on attest RX |
| `candidate_hash` | proposer tip hash hex | must match on attest RX |
| `tail_blocks` | empty | **not** part of sig domain |

Attester signs proposal binding without requiring embedded blocks — **pre-existing protocol design**. Lean default removes optional inline catch-up; operators who need propose-time block push use `--cluster-propose-full-blocks`.

**Soak note:** Scenarios where attester relied on tail for fast catch-up may see more `cluster_attest_waiting_sync` until sync wire catches up — observability, not quorum invalidation, when gate waits for attest quorum.

## 4. Style and module shape

- Config field documented (`config.rs:61-62`).
- CLI help text clear (`main.rs:333`).
- No new dependencies.

### Wire JSON / u128

Wire JSON / u128: lean propose reduces JSON size; wire fields remain strings for hashes (`candidate_hash`). No new `u128` on wire. **Perf win aligns with hotspot review.**

## 5. Safety

- No panics on empty `tail_blocks`.
- Default behavior change: attesters must already handle empty tail (tests do).

## 6. Tests

| coverage | status |
|----------|--------|
| `transport/tests/production.rs` | Multiple proposes with `tail_blocks: Vec::new()` |
| `wire_decode.rs` | `cluster_propose` without tail field → empty vec |
| `mk_cluster_prop` with `full_blocks` true/false | **missing** |
| E2E cluster soak with lean default | **recommended** (operator) |

`cargo test`: **UNVERIFIED**.

## 7. Concurrency / parallelism

`full_blocks` is immutable `ClusterCfg` after startup — no threading issue. Lean propose reduces JSON encode contention on proposer peer sessions (less allocation per heartbeat/propose). No new shared mutable state.

## 8. Verdict

**Approve with nits**

Prioritized nits:

1. **DOC-1 (low):** Document that lean mode sends **empty** `tail_blocks`, not hash-only headers; full blocks require explicit flag.
2. **TEST-1 (low):** Unit test `mk_cluster_prop` — `full_blocks=false` → empty tail + binding fields set; `true` → non-empty with `block: Some`.
3. **SOAK-1 (low):** CY soak with default lean — watch `cluster_attest_waiting_sync` / seal slip vs prior full-tail baseline.
4. **PERF-1 (low, follow-up):** Avoid `st.clone()` in `mk_cluster_prop` when only tip hash/height needed.

No quorum or panic blockers identified.

## 9. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260629-p2p-propose-tail-flag-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 24000, "confidence": "medium" }`