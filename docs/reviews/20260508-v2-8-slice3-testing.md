# V2-8 Slice 3 — pwm-testing report (header-first live sync)

**Commit:** `c316e7309fb76393cd2b28f16d0d3ce09a4137e1`  
**Ticket:** `tasks/20260508-v2-sprint8-slice3-header-block-sync.json`  
**Date:** 2026-05-08  
**Agent:** pwm-testing  

## Summary

Baseline for same-shard header-first sync and block fetch/apply is **build-clean** and **covered by unit-level tests** that exercise wire decode for new sync-v1 frames, happy-path block apply, invalid-block rejection without chain corruption, header fork handling, and shard/profile gating. No panics observed in the exercised paths. Function-name segment policy for listed transport paths is **clean**.

## Checks performed

### 1. Preflight (`target/debug` size)

- **Script:** `powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/dev/preflight_target_debug.ps1`
- **Result:** PASS — reported size within default threshold (4096 MiB).
- **Note:** Git Bash path for `preflight_target_debug.sh` was not used; `pwsh` was unavailable on PATH; PowerShell fallback used.

### 2. `cargo check -p pwmd`

- **Result:** PASS (finished successfully, no warnings treated as failures).

### 3. Targeted tests — sync / wire decode

| Focus | Command (filter) | Result |
|--------|------------------|--------|
| Wire JSON decode (incl. `sync_tip_announce`, `sync_headers_req`, negative u128 guard) | `cargo test -p pwmd decode_` | PASS — 7 tests (includes one unrelated `real_xfer_status_decode_bad`; all ok) |
| Header batch fork / break path | `cargo test -p pwmd hdr_batch_break_drop` | PASS |
| Block fetch + apply + tip advance | `cargo test -p pwmd blk_` (matches `blk_fetch_apply_ok` and `blk_bad_reject_safe`, also `snap_replay_uses_blk_ctx`) | PASS |
| Bad block apply rejected; chain rolled back / tip unchanged | (same `blk_` run) | PASS — `sync_apply_fail_total == 1`, `tip_h() == 0` after bad `state_root` |
| Shard mismatch drops tip | `cargo test -p pwmd sync_shard_drop_noop` | PASS |

**Apply safety (from tests):** `blk_bad_reject_safe` asserts metrics and **`tip_h() == 0`** after a deliberately invalid block; `blk_fetch_apply_ok` asserts **`tip_h() == 1`** and `sync_apply_ok_total == 1` for a valid remote block. No `panic!` or `unwrap` failures in these runs.

### 4. Naming script (`scripts/check_rust_fn_name_segments.py`)

Run on slice artifact list (prod ≤4 segments, test ≤5):

- `crates/pwmd/src/transport/peer_session/wire.rs`
- `crates/pwmd/src/transport/peer_session/sync_live.rs`
- `crates/pwmd/src/transport/peer_session/mod.rs`
- `crates/pwmd/src/transport/peer_session/inbound.rs`
- `crates/pwmd/src/transport/peer_session/seed/session/initial_exchange.rs`
- `crates/pwmd/src/transport/peer_session/seed/session/steady_session.rs`
- `crates/pwmd/src/transport/handshake_state.rs`
- `crates/pwmd/src/transport/metrics.rs`
- `crates/pwmd/src/transport/tests/wire_decode.rs`
- `crates/pwmd/src/transport/tests/harness.rs`

**Result:** PASS — `violations: []` for every file.

## Gaps / follow-ups (not blockers for this slice)

- **Multi-peer / network integration:** coverage is harness-level (`route_sync_stub` / `on_*` with in-memory `App`), not multi-node e2e over real sockets for full steady session.
- **`cargo test -p pwmd blk_`:** also runs `snapshot::io::tests::snap_replay_uses_blk_ctx` (name substring); harmless but slightly broader than sync-only.

## Handoff block (orchestrator)

```yaml
agent: pwm-testing
result: PASS
artifacts:
  - docs/reviews/20260508-v2-8-slice3-testing.md
commands:
  - name: preflight_target_debug.ps1
    result: PASS
  - name: cargo check -p pwmd
    result: PASS
  - name: cargo test -p pwmd decode_
    result: PASS
  - name: cargo test -p pwmd hdr_batch_break_drop
    result: PASS
  - name: cargo test -p pwmd blk_
    result: PASS
  - name: cargo test -p pwmd sync_shard_drop_noop
    result: PASS
  - name: python scripts/check_rust_fn_name_segments.py (transport slice list)
    result: PASS
cleanup: n/a (no daemons started)
token_usage: { source: estimate, input: null, output: null, total: ~8000, confidence: low }
```
