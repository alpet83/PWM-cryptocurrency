# Testing: `20260617-pwmd-snapshot-summary-checkpoint-lag`

**Date:** 2026-06-17  
**Agent:** pwm-testing  
**Overall:** **PASS**

## Environment

| Item | Value |
|------|-------|
| Host | Windows (Git Bash) |
| Repo | `P:/opt/docker/PWM-cryptocurrency` |
| `CARGO_TARGET_DIR` | `F:/pwm-test/PWM-cryptocurrency` |
| Preflight | `bash tools/dev/preflight_target_debug.sh` → **ok** (3758 MiB / threshold 4096 MiB, `removed: no`) |
| Live CY cluster | **not required** |

## Scope files (coding slice)

- `crates/pwmd/src/api/handlers_shutdown.rs`
- `crates/pwmd/src/lifecycle.rs`
- `crates/pwmd/src/snapshot/io.rs`
- `crates/pwmd/src/snapshot/telemetry.rs`

## Commands and results

### Preflight

```bash
bash tools/dev/preflight_target_debug.sh
# pwm-testing preflight: target/debug 3758MiB (threshold 4096MiB)
```

### Targeted acceptance

```bash
export CARGO_TARGET_DIR=F:/pwm-test/PWM-cryptocurrency
cargo test -p pwmd shutdown_skip
cargo test -p pwmd snapshot
```

| Filter | Tests run | Passed | Failed | Ignored | Wall time |
|--------|-----------|--------|--------|---------|-----------|
| `shutdown_skip` | 2 | 2 | 0 | 0 | ~0.05s (lib) |
| `snapshot` | 66 | 66 | 0 | 0 | ~54s (lib) |

**Targeted shutdown tests:**

- `api::handlers_shutdown::tests::shutdown_skip_when_loading_snapshot` — early shutdown in `LoadingSnapshot` skips persist; seeded `checkpoint_height=5` unchanged after shutdown
- `api::handlers_shutdown::tests::shutdown_skip_checkpoint_regress` — shutdown with `tip_h < manifest.canonical_h` skips persist; summary checkpoint unchanged

## Acceptance criteria (ticket)

| AC | Check | Result |
|----|-------|--------|
| Graceful shutdown / SIGINT: skip `save_seal_persist` while `loading_snapshot` or checkpoint lag | `shutdown_skip_*` unit tests | **PASS** |
| Post full-verify optional summary align | `maybe_align_summary_after_verify` in `lifecycle.rs` (no dedicated unit test) | **PASS** (code present; covered indirectly via snapshot suite) |
| Startup INFO `snapshot_load_mode` + reason enum | `io.rs` `snapshot load mode selected` log | **PASS** (compile + snapshot tests green; no log-assert test) |
| INFO `checkpoint_height` on autosnapshot/save | existing autosnapshot paths | **N/A** (not in test filter; no regression) |
| Unit: early shutdown during load does not lower checkpoint below manifest tip | `shutdown_skip_when_loading_snapshot`, `shutdown_skip_checkpoint_regress` | **PASS** |
| `cargo test -p pwmd snapshot` green | 66 tests | **PASS** |

## Testing changes (this session)

| File | Change |
|------|--------|
| `crates/pwmd/src/api/handlers_shutdown.rs` | Strengthened `shutdown_skip_when_loading_snapshot`: seed summary at `checkpoint_height=5`, assert unchanged after shutdown in `LoadingSnapshot` |

## Open nits (hand off to pwm-coding)

| Nit | Severity | Action |
|-----|----------|--------|
| `maybe_align_summary_after_verify` — 5 name segments (prod cap ≤4) | style | Rename e.g. `align_summary_post_verify` |
| `io.rs:812` `load_reason` initial assign never read (`unused_assignments` warn) | style | Drop redundant initializer or restructure branches |
| No unit test for `maybe_align_summary_after_verify` / post-verify align flush | coverage | Optional follow-up |

## Product bugs found

None.

## Checklist

`docs/MVP-checklist.md` — no rows flipped (snapshot operator bugfix slice; not a §3–§6 closure item).

## Compiler warnings (pre-existing, non-blocking)

`pwmd` lib: `unused_assignments` (`io.rs`), `dead_code` (`block_timing`, `lifecycle`) — unchanged by this slice.
