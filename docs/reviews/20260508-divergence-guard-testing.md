# Divergence guard hotfix — pwm-testing report

**Date:** 2026-05-08  
**Ticket:** `tasks/20260508-consensus-divergence-guard.json`  
**Scope:** validate pwm-coding hotfix for same-height tip hash mismatch (disconnect, per-peer backoff ~60s, metrics/reason labels, no action when heights differ).

## Verdict: **PASS** (merge-ready for stated transport surface)

Automated coverage aligns with policy: unit tests in `pwmd` `peer_session` exercise disconnect + backoff marker on equal-height hash mismatch and confirm the divergence counter is **not** incremented when peer height differs.

*Note:* the command rows below capture the **first** pwm-testing sweep (two `tip_divergence_*` tests). The current tree adds settled-anchor + inbound-cooldown cases (four `tip_divergence_*` tests total); reruns and microfix commits are summarized in § **Microfix validation** at the end of this file.

## Commands executed

| Step | Command | Result |
|------|---------|--------|
| Preflight | `powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/dev/preflight_target_debug.ps1` | PASS (~216 MiB under 4096 MiB threshold) |
| Targeted tests | `cargo test -p pwmd tip_divergence -- --nocapture` | **2 passed** |
| Transport peer_session | `cargo test -p pwmd peer_session::tests -- --nocapture` | **17 passed** |
| Format | `cargo fmt -p pwmd -- --check` | PASS |
| Snapshot bench harness | `cargo bench -p pwmd --bench snapshot_load --no-run` | PASS (compile only) |

## Key test outputs (abridged)

- `tip_divergence_disconnect_marks_backoff` — asserts `PeerCloseReason::SyncTipDivergence`, reconnect/backoff bookkeeping, and metric `sync_tip_divergence_disconnect_total` increment where applicable.
- `tip_divergence_height_skip` — asserts **no** divergence disconnect path when heights differ (counter unchanged).

Full logs were not archived; reruns use the commands above.

## Gaps / risks

- No dedicated integration test with live TCP peers in this pass; behavior is covered at session/route layer via existing harness.
- Operator-visible log lines and Prometheus scraping were not exercised here—only code paths + unit assertions.

## CQDS / MCP

- Consulted MCP **`cq_help`** for `cq_process_ctl` prior to execution planning; tests were run locally via `cargo` from repo root (`P:\opt\docker\pwm-protocol`) after successful preflight.

---

## Microfix validation (commits `39b258a`, `9328415`)

**Date:** 2026-05-08 (pwm-testing re-run)  
**Coding commit:** `39b258a` (inbound cooldown symmetry, optional `finalized_hash` on `SyncTipAnnounce`, `on_tip` settled-anchor path). Ticket-only record: `9328415`.

### Verdict: **PASS**

| Focus | Automated signal |
|-------|------------------|
| **(1) Settled anchor at equal height** | `tip_divergence_prefers_settled_anchor`: chain sealed to height ≥2; peer sends **mismatched** `head_hash` but **matching** `finalized_hash` at `finalized_height = tip_h - 1` → `SyncRouteOutcome::Continue`, `sync_tip_divergence_disconnect_total == 0`. |
| **(2) Fallback when anchor absent** | `tip_divergence_disconnect_marks_backoff`: `finalized_hash: None`, equal height, tip hash mismatch → `PeerCloseReason::SyncTipDivergence`, metric increment, seed cooldown ≥59s. |
| **(3) Inbound cooldown symmetry** | `tip_divergence_inbound_seed_cooldown`: `seed_key` argument `None`; `seed_peers` entry keyed by seed id with `last_node_id` matching peer → disconnect still applies **≥59s** cooldown on that seed bucket (maps node→seed for `sync_tip_divergence`). |
| **(4) Height lag / legacy assumptions** | `tip_divergence_height_skip`: peer **ahead** by one → `Continue`, divergence counter **unchanged** (no same-height false trigger). `live_reconnect_sync_no_deadlock` still passes in `peer_session::tests` (`on_tip` with `finalized` args via `None` lag path). |

### Commands executed

| Step | Command | Result |
|------|---------|--------|
| Preflight | `powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/dev/preflight_target_debug.ps1` | PASS (~226 MiB under 4096 MiB threshold) |
| Divergence suite | `cargo test -p pwmd tip_divergence -- --nocapture` | **4 passed** |
| `peer_session` module | `cargo test -p pwmd peer_session::tests -- --nocapture` | **19 passed** |
| Wire decode | `cargo test -p pwmd wire_decode -- --nocapture` | **7 passed** (incl. `decode_sync_tip_ok` with optional `finalized_hash`) |
| Format | `cargo fmt -p pwmd -- --check` | PASS |
| Snapshot bench harness | `cargo bench -p pwmd --bench snapshot_load --no-run` | PASS (compile only) |

### CQDS / MCP (microfix pass)

- **`cq_help`** with `tool_ref=cq_process_ctl` consulted before run planning; execution used host `cargo` from repo root after preflight.
