# Sprint 14 Slice 15 Testing

Date: 2026-04-28
Repository: `P:/opt/docker/pwm-protocol`

## Scope

Validated runtime remediation in `pwmd` for:
1. strict failure surfacing on snapshot-save errors for tx/intent endpoints;
2. autosnapshot checkpoint policy every 100 blocks;
3. diagnostics endpoint `/v1/flow/recent`;
4. explicit roaming relay contract fields (`relay_mode` / `relay_hint`);
5. docs alignment with runtime behavior.

## Evidence

### 1) `POST /v1/tx` and `POST /v1/roaming-intents` fail explicitly on snapshot-save error

Source checks:
- `crates/pwmd/src/api.rs`: shared `persist_snapshot_or_http_err(...)` returns `500` with explicit text when `save_snapshot` fails; used by both `v1_tx` and `v1_roaming_intent_create`.
- `crates/pwmd/src/lib.rs`: tests validate both endpoint paths.

Targeted tests:
- `cargo test -p pwmd tests::v1_tx_returns_500_when_snapshot_save_fails -- --exact` -> PASS (`1 passed; 0 failed`).
- `cargo test -p pwmd tests::v1_roaming_intent_returns_500_when_snapshot_save_fails -- --exact` -> PASS (`1 passed; 0 failed`).

Observed contract:
- HTTP status is `500 INTERNAL_SERVER_ERROR`;
- response body contains `snapshot save failed`;
- runtime state transitions to `ready_degraded` with `snapshot_error` set.

Verdict: **PASS**.

### 2) Autosnapshot every 100 blocks behavior is effective

Source checks:
- `crates/pwmd/src/lifecycle.rs`:
  - `AUTOSNAPSHOT_BLOCK_INTERVAL: u64 = 100`;
  - seal loop computes `periodic_hit = h > 0 && h % AUTOSNAPSHOT_BLOCK_INTERVAL == 0`;
  - emits checkpoint log `autosnapshot checkpoint hit interval=100 height=<h>` on each 100th block.

Targeted test:
- `cargo test -p pwmd lifecycle::tests::autosnapshot_interval_hits_every_100_blocks -- --exact` -> PASS (`1 passed; 0 failed`).

Verdict: **PASS** (policy and trigger condition validated).

### 3) `/v1/flow/recent` exposes useful diagnostics events

Source checks:
- `crates/pwmd/src/api.rs`: `v1_flow_recent` returns bounded in-memory recent flow rows.
- flow rows are pushed for `accepted:*`, `sealed:*`, and `roaming_status:*`.

Targeted test:
- `cargo test -p pwmd tests::v1_flow_recent_exposes_accepted_and_sealed_events -- --exact` -> PASS (`1 passed; 0 failed`).

Observed contract:
- endpoint returns `200 OK` with non-empty `rows`;
- rows include `accepted:*` events after tx submission.

Verdict: **PASS**.

### 4) Roaming status includes explicit `relay_mode` / `relay_hint`

Source checks:
- `crates/pwmd/src/api.rs`: `IntentStatusOut` includes `relay_mode` and `relay_hint` with explicit manual handoff guidance.
- same relay contract is also present in `/v1/status` as `roaming_relay_mode` / `roaming_relay_hint`.

Targeted test:
- `cargo test -p pwmd tests::v1_roaming_intent_create_and_get_status -- --exact` -> PASS (`1 passed; 0 failed`).

Observed contract:
- status response includes `relay_mode = manual_handoff_required`;
- test now also asserts `relay_hint` contains explicit operator guidance text.

Verdict: **PASS**.

### 5) Docs are aligned with new runtime behavior

Checked docs:
- `docs/pwmd.md` documents:
  - strict `500` on snapshot-save failures for tx/intent;
  - `ready_degraded` + `snapshot_error` visibility;
  - `/v1/flow/recent` diagnostics purpose;
  - explicit relay fields/hints;
  - autosnapshot checkpoint interval (`100`).
- `docs/tester-guide-devnet-smoke.md` includes operator checks for:
  - relay fields in roaming intent status;
  - `/v1/flow/recent` diagnostics;
  - expected `500` persistence-fail behavior for tx/intent.
- `docs/reviews/sprint-14-slice15-coding.md` scope statement matches implementation.

Verdict: **PASS**.

## Final verdict

Slice 15 runtime remediation validation status: **PASS**.

All requested behaviors are confirmed by focused tests plus source/doc consistency checks:
- strict failure surfacing is active for both tx and roaming-intent paths;
- autosnapshot checkpoint trigger is locked to each 100th block;
- `/v1/flow/recent` provides actionable recent runtime events;
- roaming/status relay contract is explicit (`relay_mode`, `relay_hint`);
- operator/docs narratives are aligned with implemented runtime semantics.
