# S2 lease / fencing failover — testing report (`pwm-testing`)

**Date:** 2026-05-09  
**Ticket:** `tasks/20260509-single-sealer-failover-profiles.json`  
**Coding commit validated:** `2e597d7` (`feat(pwmd): add single-sealer lease fencing failover gate`)  
**Task metadata commit:** `cfda7af` (`chore(tasks): record S2 coding delegation commit`)

## Verdict: **PASS**

All targeted checks compile and automated tests exercised below **passed**. Minor gap: JSON `/status`-style lease counters are wired in handlers/types but **not** asserted by a dedicated test (compilation + code review only).

---

## 1) Seal gate in `single_sealer` requires valid lease

**Code:** `lifecycle::run_lease_gate` returns `false` when `step.allow_seal` is false (`single_sealer` only); seal loop skips `chain.seal` when the gate fails.

```228:316:crates/pwmd/src/lifecycle.rs
async fn run_lease_gate(app: &App) -> bool {
    if app.deployment_profile != DeploymentProfile::SingleSealer {
        return true;
    }
    // ...
    step.allow_seal
}

pub fn spawn_seal_loop(app: App) {
    tokio::spawn(async move {
        // ...
            if !run_lease_gate(&app).await {
                // ... continue without seal
                continue;
            }
            maybe_align_mid(&app).await;
            let mut g = app.inner.write().await;
            // ...
            match g.chain.seal(txs) {
```

**Evidence (tests):** pure lease state machine asserts acquire/renew/reject/takeover semantics used by `step_lease` (see §2–§3).

---

## 2) Takeover / loss scenarios (same validator hash, distinct node instances)

| Scenario | Test | Result |
|---------|------|--------|
| B blocked while A holds lease; takeover after TTL+takeover window | `lease::tests::lease_takeover_after_timeout` | **ok** |
| A loses lease after B committed takeover; further A steps yield loss | `lease::tests::old_active_blocked_without_lease` | **ok** |
| Same owner renew | `lease::tests::lease_renew_ok_same_owner` | **ok** |

---

## 3) Old active blocked without lease

Covered by `old_active_blocked_without_lease`: after B takes over at `t=2600`, A at `t=2650` gets `allow_seal == false`, `LeaseEvent::Loss`, `LeaseState::StandbySyncing`.

---

## 4) Status / observability fields compile and surfaced

**Types:** `crates/pwmd/src/api/types.rs` — `RpcStatusReply` adds `lease_state`, `seal_gate_allowed`, `lease_owner_id`, `lease_term`, `lease_expires_at_ms`, `lease_last_tip`, `lease_fence`, `lease_last_reason`, and lease stat counters (`lease_acquire_ok`, …).

**Handler:** `api::handlers_status::v1_status` maps `lease_runtime` + `lease_stats.snapshot()` into the reply.

**Test:** `api::handlers_status::tests::status_exposes_identity_signals` — **PASS** (profile / seal role / identity); does **not** yet assert lease field values (non-blocking for “compiles + wired”).

---

## 5) Transport heartbeat paths — no obvious regression

Extra frames on `PeerWireMsg::Heartbeat` / `Hello` (optional lease fields) remain backward-compatible (`Option`).

| Test | Result |
|------|--------|
| `transport::tests::production::prod_seed_idle_windows_ok` | **ok** |
| `transport::tests::peer_harness::peer_micro_idle_hb_ok` | **ok** |

---

## Commands run

| Command | Outcome |
|---------|---------|
| `powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/dev/preflight_target_debug.ps1` | **PASS** (~226 MB under 4096 MiB threshold) |
| `cargo test -p pwmd lease_renew_ok_same_owner` | **ok** |
| `cargo test -p pwmd lease_takeover_after_timeout` | **ok** |
| `cargo test -p pwmd old_active_blocked_without_lease` | **ok** |
| `cargo test -p pwmd status_exposes_identity_signals` | **ok** |
| `cargo test -p pwmd prod_seed_idle_windows_ok` | **ok** |
| `cargo test -p pwmd peer_micro_idle_hb_ok` | **ok** |
| `cargo check -p pwmd` | **PASS** (from prior run in session; incremental) |

**Note:** Filter `cargo test -p pwmd lease_` only matches tests whose names contain `lease_`; **`old_active_blocked_without_lease` must be invoked by full substring** (`old_active_blocked_without_lease` or `lease::tests`).

**Snapshot benches:** not required for this slice (per ticket); not run.

---

## Risks / follow-ups (one line each)

- In-process lease map is MVP/local-failover semantics; multi-host split-brain assumptions should stay documented in ops runbooks.
- Consider extending `status_exposes_identity_signals` (or a sibling test) to assert non-empty/default lease fields after one `step_lease` equivalent on a dev app snapshot.
