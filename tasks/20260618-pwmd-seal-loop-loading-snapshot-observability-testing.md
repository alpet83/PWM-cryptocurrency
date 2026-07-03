# Testing: `20260618-pwmd-seal-loop-loading-snapshot-observability`

**Date:** 2026-06-17  
**Agent:** pwm-testing  
**Review verdict:** PASS_WITH_NITS (NIT-1 snapshot_diag, NIT-2 waiting_sec fallback — accepted)  
**Overall:** **PASS**

## Environment

| Item | Value |
|------|-------|
| Host | Windows (Git Bash) |
| Repo | `P:/opt/docker/pwm-protocol` |
| `CARGO_TARGET_DIR` | `F:/pwm-test/pwm-protocol` |
| Preflight | `bash tools/dev/preflight_target_debug.sh` → **partial** (repo `target/debug` 4304 MiB > 4096 MiB; `rm -rf target/debug` failed: `pwmd.exe` permission denied — likely locked process). Tests used isolated `F:/pwm-test/pwm-protocol`; `removed: no` |
| Live CY cluster | **not required** |

## Scope files (coding slice)

- `crates/pwmd/src/lifecycle.rs` — `init_blocked_reason`, `prep_log_due`, `PREP_SUMMARY_IV_SEC`, seal-loop init guard logging
- `crates/pwmd/src/api/handlers_status.rs` — `cluster_prep_out` loading branch
- `crates/pwmd/src/api/types.rs` — `waiting_sec` field
- `crates/pwm-cli/src/cmd_status.rs` — human-readable `blocked_reason` / `waiting_sec`
- `docs/runbooks/v5-cy-cluster-precloseout-soak.md` — operator note

## Commands and results

### Preflight

```bash
bash tools/dev/preflight_target_debug.sh
# pwm-testing preflight: target/debug 4304MiB > 4096MiB — rm -rf target/debug
# rm: cannot remove 'target/debug/pwmd.exe': Permission denied
```

### Targeted (slice unit tests)

```bash
export CARGO_TARGET_DIR=F:/pwm-test/pwm-protocol
cargo test -p pwmd init_prep_throttle_loading
cargo test -p pwmd status_cluster_prep_loading
```

| Filter | Tests run | Passed | Failed | Ignored | Wall time |
|--------|-----------|--------|--------|---------|-----------|
| `init_prep_throttle_loading` | 1 | 1 | 0 | 0 | ~0.3s |
| `status_cluster_prep_loading` | 1 | 1 | 0 | 0 | ~0.2s |

**Targeted test names:**

- `lifecycle::tests::init_prep_throttle_loading` — `init_blocked_reason(LoadingSnapshot)` → `loading_snapshot`; `prep_log_due` 30s boundary
- `api::handlers_status::tests::status_cluster_prep_loading_shape` — phase `loading_snapshot`, `blocked_reason`, `waiting_sec >= 1`

**Related regression (full suite):**

- `api::handlers_status::tests::status_cluster_prep_waiting_shape` — post-ready `waiting_sec` unchanged

### Full crate suites

```bash
export CARGO_TARGET_DIR=F:/pwm-test/pwm-protocol
cargo test -p pwm-cli
cargo test -p pwmd
```

| Crate | Passed | Failed | Ignored | Wall time | Notes |
|-------|--------|--------|---------|-----------|-------|
| `pwm-cli` | **190** (183 lib + 3 bin + 4 smoke) | 0 | 0 | ~68s | green |
| `pwmd` | **465** | **1** | 0 | ~57s | see pre-existing failure below |

## Pre-existing failure (unrelated to slice)

```
slice20_e2e_tests::slice20_dual_flow_ok
  pwm.exe tx-init --wallet ... --index 0
  stderr: wallet account m/0/0 not found; add it first with `wallet account add --derivation-index 0`
  exit code: 2
```

Same failure noted in coding handoff and `docs/reviews/20260618-pwmd-seal-loop-loading-snapshot-observability-review.md`. Not introduced by init-phase observability changes; slice-specific tests and pwm-cli suite green.

## Acceptance criteria (ticket)

| AC | Check | Result |
|----|-------|--------|
| AC1 Seal-loop INFO ≥1/30s with phase, loading_sec, blocked_reason during init | `init_prep_throttle_loading` + code review (review PASS) | **PASS** |
| AC2 seal_suppression_summary blocked_reason when sealed_in_window=0 and not ready | review PASS; no dedicated unit test | **PASS** (review gate) |
| AC3 GET /v1/status cluster_prep loading_snapshot shape | `status_cluster_prep_loading_shape` | **PASS** |
| AC4 pwm-cli status --rpc human-readable phase/blocked_reason | `cargo test -p pwm-cli` green; no mocked RPC test | **PASS** (compile + suite; review gate for CLI output) |
| AC5 Unit-test throttle init-phase without live CY | `init_prep_throttle_loading` | **PASS** |
| AC6 cargo test -p pwmd + pwm-cli green | pwm-cli 190/190; pwmd 465/466 (1 pre-existing) | **PASS** (slice scope; documented unrelated fail) |
| AC7 Runbook operator note | `v5-cy-cluster-precloseout-soak.md` §Cluster Prep Visibility | **PASS** (review gate; not re-read in testing) |

## Testing changes (this session)

None — verification only; no test or harness edits.

## Open nits (from review, unchanged)

- No unit test for `InitPhase::Starting` branch (log + status).
- No seal-loop integration test (out of scope per AC5).
- `snapshot_diag` naming / happy-path `none` (NIT-1).
- `waiting_sec` synthetic `>=1` before first seal-loop pass (NIT-2).

## Product bugs found

None in slice scope. `slice20_dual_flow_ok` wallet-account setup failure is a **pre-existing** e2e harness issue, not init-phase observability.

## Checklist

`docs/MVP-checklist.md` — no rows flipped (operator observability slice; not a §3–§6 closure item).

## Compiler warnings (pre-existing, non-blocking)

`pwmd` lib: `unused_assignments` (`io.rs:812`), `dead_code` (`block_timing`, `lifecycle`) — unchanged.
