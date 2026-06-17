# Testing: `20260616-pwmd-cluster-prep-observability`

**Date:** 2026-06-17  
**Agent:** pwm-testing  
**Review verdict:** PASS_WITH_NITS (low coverage nits only — not blockers)  
**Overall:** **PASS**

## Environment

| Item | Value |
|------|-------|
| Host | Windows (Git Bash) |
| Repo | `P:/opt/docker/PWM-cryptocurrency` |
| `CARGO_TARGET_DIR` | `F:/pwm-test/PWM-cryptocurrency` |
| Preflight | `bash tools/dev/preflight_target_debug.sh` → **ok** (3299 MiB / threshold 4096 MiB, no cleanup) |
| Live CY cluster | **not required** |

## Commands and results

### Preflight

```bash
bash tools/dev/preflight_target_debug.sh
# pwm-testing preflight: target/debug 3299MiB (threshold 4096MiB)
```

### Targeted (pwmd, cluster prep observability)

```bash
export CARGO_TARGET_DIR=F:/pwm-test/PWM-cryptocurrency
cargo test -p pwmd cluster_prep
cargo test -p pwmd sync_stall_tick
```

| Filter | Tests run | Passed | Failed | Ignored |
|--------|-----------|--------|--------|---------|
| `cluster_prep` | 1 | 1 | 0 | 0 |
| `sync_stall_tick` | 1 | 1 | 0 | 0 |

**Targeted test names:**

- `api::handlers_status::tests::status_cluster_prep_waiting_shape` — JSON shape `waiting_attester`, `blocked_reason`, `waiting_since_ms`
- `transport::peer_session::sync_live::tests::sync_stall_tick_10s` — 10s stall log cadence

**Related (full suite only, not name-filtered):**

- `api::handlers_status::tests::status_exposes_identity_signals` — `cluster_prep.phase=ready` regression

### Full crate suites

```bash
cargo test -p pwmd
cargo test -p pwm-cli
```

| Crate | Binaries / harness | Passed | Failed | Ignored | Wall time |
|-------|-------------------|--------|--------|---------|-----------|
| `pwmd` | lib (462) + bins/integration (7) | **469** | 0 | 0 | ~54s lib + integration |
| `pwm-cli` | lib (180) + bins (3) + `cli_smoke` (4) | **187** | 0 | 0 | ~64s lib |

**Totals:** 656 automated tests green; 0 failed; 0 ignored.

## Acceptance criteria (testing slice)

| AC | Check | Result |
|----|-------|--------|
| `cargo test -p pwmd` green | Full suite | **PASS** (469) |
| `cargo test -p pwm-cli` green | Full suite | **PASS** (187) |
| `CARGO_TARGET_DIR=F:/pwm-test/PWM-cryptocurrency` | Isolated target on Windows | **PASS** |
| Preflight `tools/dev/preflight_target_debug.sh` | Size guard | **PASS** |
| Targeted `cluster_prep` / `sync_stall_tick` | pwmd unit tests | **PASS** (2/2) |
| No live CY | No cluster soak run | **N/A** (by design) |

## Open nits (from review, unchanged)

- No unit test for 30s `cluster_prep_summary` throttle in `lifecycle.rs` (`prep_summary_at`).
- No mocked HTTP test for `pwm status` / `cmd_status::run_status`.

## Product bugs found

None.

## Checklist

`docs/MVP-checklist.md` — no rows flipped (slice is operator observability; not a checklist §3–§6 closure).
