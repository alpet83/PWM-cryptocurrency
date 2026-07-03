# MVP V4 integrated devnet gate smoke (2026-05-17)

Ticket: `tasks/20260517-v4-sprint6-closeout.json`.

**Current verdict: PASS** — after the pwmd JSON contract fix, the retest gate is green; see **Addendum** at the end of this file.

**Initial run history:** first pass was **PARTIAL**: V4 policy / CLI / core matrix was green, but **`pwmd --lib`** reported 2 failed `transport_peer` tests (JSON field `Null` vs expected value). The failure is preserved below as historical context.

## Environment

- Host: Windows; `powershell.exe` (Core `pwsh` not on PATH).
- **`CARGO_TARGET_DIR=F:\pwm-test\pwm-protocol`** (created; builds off project volume).

## Preflight

- `powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/dev/preflight_target_debug.ps1`
- Repo `target/debug` sum **~216 MiB**, below 4096 MiB threshold (**removed: no**).

## Commands and results

| Command | Result |
|---------|--------|
| `cargo check --workspace` | **PASS** |
| `cargo test -p pwm-core --lib` | **PASS** (134 passed, 1 ignored `snap_keep_imp_replay_guard`) |
| `cargo test -p pwm-core policy_` | **PASS** (20 tests) |
| `cargo test -p pwm-cli tx_pol_` | **PASS** (4 tests) |
| `cargo test -p pwm-cli tx_init_v4_cli_parse` | **PASS** |
| `cargo test -p pwm-cli --test cli_smoke help_tx_core_cmds` | **PASS** |
| `cargo test -p pwm-cli` (full) | **PASS** (165 lib + 4 integration) |
| `cargo bench -p pwmd --bench snapshot_load --no-run` | **PASS** (compiled_only) |
| `cargo test -p pwmd --lib` | **FAIL** — 365 passed, **2 failed** (see below) |
| `cargo fmt --all -- --check` | **PASS** |

### `pwmd` failures (regression gap)

- `tests::transport_peer::v1_dev_peers_xfer_snap` — assertion `left == right` failed: `left: Null`, `right: 1` (`transport_peer.rs:605`).
- `tests::transport_peer::v1_status_gen_guard_diag` — `left: Null`, `right: "peer-genesis-status"` (`transport_peer.rs:205`).

Likely JSON shape / status payload drift (genesis guard field missing or serialized as null). **Not** exercised by the narrow V4 policy CLI filters; blocks a clean “full pwmd lib” gate.

## Not run (scope / cost)

- Full `cargo test --workspace` (partially covered via `pwm-core --lib`, full `pwm-cli`, failed `pwmd --lib`).
- `pwm-tui` / manual TUI banners (machine check limited to **`cargo check -p pwm-tui`** via workspace check).
- Long-lived devnet / daemon soak.
- **`cargo test --test`** integration binaries under `pwmd` beyond `--lib`.

## V4 checklist mapping (automated signals)

- Extended INIT V4 / account fields: **`init_v4_ext_sets_account`**, **`tx::init_v4_signing_json`**, **`tx_init_v4_cli_parse`** — exercised in pwm-core/pwm-cli runs.
- Policy set / activate / deactivate: **`policy_*`**, **`tx_pol_*`**, **`tx_policy_set_cli_parse`** — pass.
- Emergency activation / finalization / redirect: tests such as **`policy_emerg_act_ok_finalizes`**, **`policy_route_deny_no_mut`**, **`policy_fin_blocks_old_ops`** — pass inside pwm-core `--lib`.
- Structured rejects: **`reject_wire::tests::*`** — pass inside pwm-core `--lib**.

## Recommendation

Fix or relax expectations for the two **`transport_peer`** JSON assertions after confirming intended HTTP/status wire shape for V4 genesis guard, then rerun `cargo test -p pwmd --lib`.

---

## Addendum: retest after pwmd JSON contract fix (same day)

**Verdict: PASS** — same host and `CARGO_TARGET_DIR` as above.

| Command | Result |
|---------|--------|
| `cargo fmt --all -- --check` | **PASS** |
| `cargo check --workspace` | **PASS** |
| `cargo test -p pwmd --lib` | **PASS** (367 passed, 0 failed) |
| `cargo test -p pwm-core --lib` | **PASS** (134 passed, 1 ignored) |
| `cargo test -p pwm-cli` | **PASS** (165 lib + 4 integration) |
| `cargo test -p pwm-core policy_` | **PASS** (20 tests) |
| `cargo test -p pwm-cli tx_pol_` | **PASS** (4 tests) |
| `cargo bench -p pwmd --bench snapshot_load --no-run` | **PASS** |

Prior `transport_peer` JSON `Null` vs expected regressions are cleared by the wire/contract fix; no commit from this retest.
