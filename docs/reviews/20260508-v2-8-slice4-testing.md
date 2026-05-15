# V2-8 Slice 4 — pwm-testing report (epoch catch-up fallback)

**Commit:** `4df23d53a431e02ade502201aaeebc7926aefd06`  
**Ticket:** `tasks/20260508-v2-sprint8-slice4-epoch-catchup.json`  
**Date:** 2026-05-08  
**Agent:** pwm-testing  

## Summary

Epoch catch-up fallback on the slice implementation commit is **build-clean** and **covered** by unit tests for happy-path chunk apply with `on_cup_done`, corrupted chunk tail/hash mismatch (metrics + no tip advance), and profile gate (`can_cup == false` → drop counter only). Wire JSON decoding for `sync_catchup_chunk` is exercised. Code review of `on_cup_chunk` shows validation branches that call `cup_chunk_fail` instead of panicking on bad bounds/order/link/apply. Function-name segment policy on listed transport paths is **clean**.

## Checks performed

### 1. `cargo check -p pwmd`

- **Result:** PASS (finished successfully).

### 2. Targeted tests — catch-up flow + wire decode

| Focus | Command (filter) | Result |
|--------|------------------|--------|
| Catch-up happy path, bad chunk safety, profile mismatch drop | `cargo test -p pwmd cup_` | PASS — 4 tests (`cup_missing_range_ok`, `cup_bad_chunk_safe`, `cup_profile_mismatch_noop`, plus `decode_sync_cup_chunk_ok` matched by substring `cup_`) |
| Wire decode `sync_catchup_chunk` (explicit) | `cargo test -p pwmd decode_sync_cup_chunk_ok` | PASS |

**Details:**

- `cup_missing_range_ok`: after tip align + two nacks (stall window), one valid `SyncCatchupChunkWire` and `on_cup_done` → `sync_cup_{start,chunk,done}_total == 1`, `tip_h() == 3`.
- `cup_bad_chunk_safe`: mismatched `last_hash` vs header row → `sync_cup_fail_total >= 1`, chain tip unchanged (`tip_h() == 0`).
- `cup_profile_mismatch_noop`: `route_sync_stub` with `can_cup: false` on `SyncCatchupReq` → `sync_cup_drop_total == 1`, no chain advance.

### 3. Panic / bad-input safety (code + tests)

- **`on_cup_chunk`** (`sync_live.rs`): empty/over-cap/`headers.len() != blocks.len` → `chunk_bounds`; inactive session → early return; epoch/index/prev-hash mismatch → `chunk_order`; per-row height/hash/prev link → `chunk_link`; missing block → `chunk_empty`; body hash vs header → `chunk_hash`; height outside cup range → `chunk_range`; `last_hash` tail mismatch → `chunk_tail`; `apply_blk_batch` error → `chunk_apply`. No `unwrap`/`expect` on peer-controlled fields in this path; failures funnel through `cup_chunk_fail` with backoff/clear.
- **Integration with router:** `can_cup == false` increments `sync_cup_drop_total` and returns without calling server-side catch-up handlers.

### 4. Naming script (`scripts/check_rust_fn_name_segments.py`)

Run on Slice 4 artifact list (prod ≤4 segments, test ≤5):

- `crates/pwmd/src/transport/handshake_state.rs`
- `crates/pwmd/src/transport/metrics.rs`
- `crates/pwmd/src/transport/peer_session/inbound.rs`
- `crates/pwmd/src/transport/peer_session/mod.rs`
- `crates/pwmd/src/transport/peer_session/seed/session/initial_exchange.rs`
- `crates/pwmd/src/transport/peer_session/seed/session/steady_session.rs`
- `crates/pwmd/src/transport/peer_session/sync_live.rs`
- `crates/pwmd/src/transport/peer_session/wire.rs`
- `crates/pwmd/src/transport/tests/harness.rs`
- `crates/pwmd/src/transport/tests/wire_decode.rs`

**Result:** PASS — `violations: []` for every file.

## Gaps / follow-ups (not blockers for this slice)

- **Filter quirk:** `cargo test -p pwmd cup_` matches `decode_sync_cup_chunk_ok` because the filter is substring-based (`cup` in `catchup`). Use explicit filters if you need only `peer_session::tests::cup_*`.
- **Multi-node e2e:** coverage remains harness-level (`App` + Tokio test streams), not full mesh steady-session over live TCP for catch-up handoff.

## Handoff block (orchestrator)

```yaml
agent: pwm-testing
result: PASS
artifacts:
  - docs/reviews/20260508-v2-8-slice4-testing.md
commands:
  - name: cargo check -p pwmd
    result: PASS
  - name: cargo test -p pwmd cup_
    result: PASS
  - name: cargo test -p pwmd decode_sync_cup_chunk_ok
    result: PASS
  - name: python scripts/check_rust_fn_name_segments.py (transport slice list)
    result: PASS
cleanup: n/a (no daemons started)
```
