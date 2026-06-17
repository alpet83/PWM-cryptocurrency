# Testing: `20260619-pwmd-trust-load-fastpath-proposer-validation`

**Date:** 2026-06-17  
**Agent:** pwm-testing  
**Overall:** **PASS**

## Environment

| Item | Value |
|------|-------|
| Host | Windows (Git Bash) |
| Repo | `P:/opt/docker/PWM-cryptocurrency` |
| `CARGO_TARGET_DIR` | `F:/pwm-test/PWM-cryptocurrency` |
| Preflight | `bash tools/dev/preflight_target_debug.sh` → **ok** (3274 MiB / threshold 4096 MiB, `removed: no`) |
| Live CY @124k+ | **not required** (operator gate deferred per ticket) |

## Scope files (coding + nit-fix slice)

- `crates/pwmd/src/snapshot/io.rs`
- `crates/pwmd/src/snapshot/incremental.rs`
- `docs/guide-node-storage-and-snapshot.md`

## Commands and results

### Preflight

```bash
bash tools/dev/preflight_target_debug.sh
# pwm-testing preflight: target/debug 3274MiB (threshold 4096MiB)
```

### Targeted acceptance

`cargo test` accepts only **one** name filter; ticket filters `trust_prod trust_load snapshot incremental io` are covered by the parent-module filter `snapshot` (submodules `io`, `incremental`, and `tests::snapshot_*`).

```bash
export CARGO_TARGET_DIR=F:/pwm-test/PWM-cryptocurrency
cargo test -p pwmd snapshot -- --nocapture
```

| Filter | Tests run | Passed | Failed | Ignored | Wall time |
|--------|-----------|--------|--------|---------|-----------|
| `snapshot` | 69 | 69 | 0 | 0 | ~63s (lib) |

**Trust fastpath tests (new / touched):**

| Test | Result | Notes |
|------|--------|-------|
| `snapshot::io::tests::trust_prod_no_bnd_set` | **PASS** | No epoch boundary in tail → schedule from checkpoint state |
| `snapshot::io::tests::trust_prod_tail_bnd_tx_ok` | **PASS** | Boundary `bnd_h < tip_h` with post-boundary txs (review nit #1 fix) |
| `snapshot::incremental::tests::trust_load_skips_old_replay` | **PASS** | Trust load end-to-end; see `validate_ms` below |

### `validate_ms` — `trust_load_skips_old_replay`

Captured from `--nocapture` stderr:

```
trust_load_skips_old_replay validate_ms=8054
```

| Field | Value |
|-------|-------|
| Fixture | `N=1105` seals, tip-aligned summary, tampered `prod_idx` at block height 2 |
| Profile | `test` (unoptimized + debuginfo) |
| `validate_ms` | **8054** (~8.1 s) |
| `used_full_verify` | `false` (trust path) |
| Tail blocks loaded | `TAIL_BLOCK_CAP` (1000) |

**Scale note (AC#6, documented — no hard assert in test):**

- Observed: **8.1 s** validate for **1.1k** tip on debug build.
- Naive linear extrapolation to 10k tip on same profile: ~**73 s** — above the 60 s operator SLO if interpreted naively by block count.
- **Intent of fastpath:** validate cost is **O(tail + optional boundary segment)**, not O(tip). At production tip 125k+ with `TAIL_BLOCK_CAP=1000`, expected validate work is comparable to ~1k-block tail (not 125k genesis replay). Pre-fix evidence in ticket: `validate_ms≈1_130_281` @ `tip_h=61_008` (~19 min).
- Release / owner CY remeasure @124k+ remains operator gate; fixture timing is regression signal only.

## Acceptance criteria (ticket)

| AC | Check | Result |
|----|-------|--------|
| Trust proposer path: no `1..tip_h` genesis loop | `trust_prod_*`, `trust_load_skips_old_replay` | **PASS** |
| No boundary in tail → prod_idx without replay | `trust_prod_no_bnd_set` | **PASS** |
| Boundary in tail → sequential replay from B | `trust_prod_tail_bnd_tx_ok` | **PASS** |
| Sequential epoch read / no per-height JSONL hot path | code + green snapshot suite | **PASS** |
| `stage=trust_validate` progress logs | present in `io.rs` (no log-assert test) | **PASS** (compile + trust tests) |
| validate_ms regression / documented benchmark | `trust_load_skips_old_replay` eprint **8054** | **PASS** (documented; soft gate per review) |
| Full verify (`--snapshot-verify-chain`) unchanged | `v4_replay_det_gate_ok`, `snap_*` roaming tests | **PASS** |
| `cargo test -p pwmd snapshot incremental io` green | 69 tests via `snapshot` filter | **PASS** |
| Operator guide trust-load SLO | `docs/guide-node-storage-and-snapshot.md` | **PASS** (coding; not re-tested here) |

## Testing changes (this session)

None — verification only; coding + boundary nit-fix landed before testing.

## Open risks

| Risk | Severity | Notes |
|------|----------|-------|
| AC#6 has no `assert!(validate_ms < …)` | low | By design (review PASS_WITH_NITS); owner CY @124k is live gate |
| `epoch_trust_respects_tail_cap` ~60s+ on debug | low | Pre-existing slow test; passed within wall budget |
| Debug `validate_ms` not representative of release SLO | info | Document scale above |

## Product bugs found

None.

## Checklist

`docs/MVP-checklist.md` — no rows flipped (trust-load perf slice; not a §3–§6 closure item).

## Compiler warnings (pre-existing, non-blocking)

`pwmd` lib: `dead_code` (`block_timing`, `lifecycle`) — unchanged by this slice.
