# Sprint 14 Slice 16 — testing report

Date: 2026-04-28  
Repo: `P:/opt/docker/pwm-protocol`  
Scope: validate Slice16 runtime/docs contract for finalize flow, flow trace diagnostics, and persistence failure surfacing.

## Verdict

`PASS` for all 4 requested checks.

## What was validated

1. Finalize endpoint works and is idempotent with clear statuses/messages.
2. `/v1/flow/recent` includes lifecycle events useful for CY->DO diagnosis.
3. Persistence failures in finalize/roaming paths are surfaced as `500` (not silent).
4. `docs/pwmd.md` operator sequence matches runtime behavior.

## Evidence and focused tests

### 1) Finalize endpoint + idempotency

- Existing test:
  - `tests::v1_roaming_intent_finalize_sets_relayed_and_is_idempotent`
  - Confirms first finalize -> `status=relayed`, `changed=true`;
  - Repeated finalize -> `status=relayed`, `changed=false`, stable message containing `already finalized`.
- Existing test:
  - `tests::v1_roaming_intent_finalize_returns_not_found_for_unknown_id`
  - Confirms unknown intent returns `404` with clear text.

Result: `PASS`.

### 2) `/v1/flow/recent` lifecycle diagnostics

- Existing test:
  - `tests::v1_flow_recent_exposes_accepted_and_sealed_events`.
- Added focused test:
  - `tests::v1_flow_recent_includes_roaming_finalize_lifecycle_events`
  - Confirms recent flow contains lifecycle kinds with prefixes:
    - `accepted:`
    - `finalized:`
    - `roaming_status:`

Result: `PASS`.

### 3) Persistence failures surfaced as 500

- Existing test:
  - `tests::v1_roaming_intent_returns_500_when_snapshot_save_fails` (create path).
- Added focused tests:
  - `tests::v1_roaming_intent_finalize_returns_500_when_snapshot_save_fails` (finalize path),
  - `tests::v1_roaming_intent_status_returns_500_when_expire_snapshot_save_fails` (roaming status/expire path).
- All three assert:
  - HTTP status `500 INTERNAL_SERVER_ERROR`,
  - body contains `snapshot save failed`,
  - init phase becomes `ReadyDegraded` with non-empty `snapshot_error`.

Result: `PASS`.

### 4) Operator docs sequence vs runtime

- Checked `docs/pwmd.md` roaming contract section:
  - `POST /v1/roaming-intents/:id/finalize` described as idempotent `queued|exported -> relayed`,
  - repeat finalize described as `200` and `changed=false`,
  - `/v1/flow/recent` documents lifecycle families (`accepted`, `applied`, `exported`, `imported`, `sealed`, `roaming_status`, `finalized`),
  - runtime strictness section documents `500` surfacing for finalize/roaming persistence failures.
- Runtime behavior from tests matches this sequence/contract.

Result: `PASS`.

## Commands run

- `cargo fmt`
- `cargo test -p pwmd tests::v1_roaming_intent_finalize_sets_relayed_and_is_idempotent -- --exact`
- `cargo test -p pwmd tests::v1_roaming_intent_finalize_returns_not_found_for_unknown_id -- --exact`
- `cargo test -p pwmd tests::v1_flow_recent_exposes_accepted_and_sealed_events -- --exact`
- `cargo test -p pwmd tests::v1_flow_recent_includes_roaming_finalize_lifecycle_events -- --exact`
- `cargo test -p pwmd tests::v1_roaming_intent_returns_500_when_snapshot_save_fails -- --exact`
- `cargo test -p pwmd tests::v1_roaming_intent_finalize_returns_500_when_snapshot_save_fails -- --exact`
- `cargo test -p pwmd tests::v1_roaming_intent_status_returns_500_when_expire_snapshot_save_fails -- --exact`

All listed focused tests: `PASS`.

## Files changed

- `crates/pwmd/src/lib.rs` (added 3 focused tests for slice16 validation gaps).
- `docs/reviews/sprint-14-slice16-testing.md` (this report).
