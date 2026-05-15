# Testing report: V2-8 Slice 5 (observability, chaos-oriented tests, runbook)

**Commit:** `9029fb0` — `feat(pwmd): add sync v1 observability hardening and chaos runbook`  
**Ticket:** `tasks/20260508-v2-sprint8-slice5-observability-chaos-docs.json`  
**Agent:** pwm-testing  
**Date:** 2026-05-08  

## 1. Required commands

| Check | Result |
|--------|--------|
| `cargo check -p pwmd` | PASS |
| `python scripts/check_rust_fn_name_segments.py crates/pwmd/src/transport/` | PASS (no violations) |

**Preflight `target/debug`:** not run (scoped `cargo check` / `cargo test -p pwmd` only; no full build stress).

## 2. Targeted tests (coding handoff)

### 2.1 `transport::tests::production` (`crates/pwmd/src/transport/tests/production.rs`)

Filtered: `cargo test -p pwmd prod_`

| Test | Result |
|------|--------|
| `prod_ib_sock_idle_ok` | PASS |
| `prod_seed_idle_windows_ok` | PASS |
| `prod_close_lvl_err_ok` | PASS |
| `prod_bad_sync_frame_counted` | PASS |

These cover idle TCP / seed paths, nested close diagnostics, and corrupt sync JSON → `decode_failed` on snapshot counters.

### 2.2 `transport::peer_session::tests`

Filtered: `cargo test -p pwmd peer_session::tests`

| Result | Detail |
|--------|--------|
| **14 PASS / 1 FAIL** | **`tx_batch_profile_drop` FAIL** |

**Failure:** assertion expects `sync_tx_drop_reason_total["shard_mismatch"] == Some(1)`, but routing for legacy / non–`full_v1` peers uses `profile_mismatch` (`route_sync_stub`, branch `if !full_v1 \|\| !same_shard`). The scenario name matches **profile** drop; the expected reason key appears **stale** after reason-code normalization documented in the ticket.

**Recommendation (pwm-coding, trivial):** change the expected map key from `shard_mismatch` to `profile_mismatch` in `tx_batch_profile_drop`, or split tests so shard-vs-profile reasons are asserted separately.

Other slice-relevant peers in this module (catch-up fallback, malformed paths, egress non-blackhole, reconnect) **passed** in this run, including:

- `live_reconnect_sync_no_deadlock`
- `storm_egress_not_blackhole`
- `sync_shard_drop_noop`
- `cup_nack_resets_state` (fallback toward live hdr flow)

## 3. Docs / RFC coherence

- **`docs/runbook-same-shard-sync-v1.md`** aligns with **`docs/rfc/15-same-shard-sync-v1.md` §10 (observability)** and §13 hooks: checklist references transport snapshot keys, `profile_mismatch` vs `shard_mismatch`, gossip drop reasons, storm-guard suppression vs egress counters, corrupt-frame `decode_failed`, and troubleshooting tied to RFC-style alerting (suppress without egress).
- RFC paragraph added in `9029fb0` explicitly allows `pwmd` transport snapshot keys as a **semantic mapping** to the §10 metric list; the runbook performs that mapping in §3.

**Minor terminology drift (acceptable):** RFC §13 describes `mempool_egress_relay_total{peer_class}` with wording `same_segment`; implementation/logs use **`same_shard_peer`** as a concrete class label. Operators should treat this as the same operational bucket unless a future doc normalizes naming.

## 4. Naming policy (transport paths)

Segment policy script was run over `crates/pwmd/src/transport/` including `tests/production.rs` — **no JSON violations**.

## 5. Verdict for merge / review gate

- **Docs + observability wiring:** review can proceed from a docs/RFC/runbook angle.
- **Automated regression:** **`peer_session::tests`** is **not fully green** until `tx_batch_profile_drop` expectations are corrected; slice-5 chaos/production tests targeted by the coding report **are green**.

---

**pwm-review:** please confirm ticket notes and whether pwm-coding should land the one-line test expectation fix as a fast follow (no product behavior change).

