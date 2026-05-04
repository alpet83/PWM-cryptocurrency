# Sprint 14 Slice 16 — remediation testing report

Date: 2026-04-28  
Repo: `P:/opt/docker/PWM-cryptocurrency`

## Verdict

`PASS` for all requested remediation checks.

## Scope checked

1. relayed taxonomy in `flow/recent` is consistent.
2. finalize retry semantics after snapshot `500` are deterministic.
3. terminal status finalize behavior is stable.
4. docs route inventory and RFC mention finalize/flow recent contract.

## Evidence

### 1) `flow/recent` taxonomy consistency (`relayed`)

- Focused test passed:
  - `tests::v1_flow_recent_includes_roaming_finalize_lifecycle_events`
- Runtime implementation confirms finalize emits:
  - `kind = finalized:roaming_intent`,
  - transition event `kind = roaming_status:relayed` when state changes.

Result: `PASS`.

### 2) Deterministic finalize retry after snapshot `500`

- Focused test passed:
  - `tests::v1_roaming_intent_finalize_returns_500_when_snapshot_save_fails`
- The scenario validates:
  - first finalize with forced snapshot failure returns `500`,
  - retry finalize returns `200`,
  - payload is deterministic/idempotent (`status=relayed`, `changed=false`, stable `already finalized` message).

Result: `PASS`.

### 3) Stable finalize behavior for terminal statuses

- Focused test passed:
  - `tests::v1_roaming_intent_finalize_is_idempotent_for_terminal_statuses`
- Terminal statuses verified as stable/idempotent:
  - `imported`,
  - `expired`,
  - `failed`.

Result: `PASS`.

### 4) Docs route inventory + RFC contract

- `docs/pwmd.md` includes both routes:
  - `POST /v1/roaming-intents/:id/finalize`,
  - `GET /v1/flow/recent`.
- `docs/pwmd.md` contract text states:
  - finalize is idempotent (`queued|exported -> relayed`, repeat -> `200` + `changed=false`),
  - flow recent includes lifecycle families (`accepted:*`, `applied:*`, `exported:*`, `imported:*`, `sealed:*`, `roaming_status:*`, `finalized:*`).
- `docs/rfc/9-crossdomain-roaming.md` explicitly mentions:
  - finalize endpoint semantics and idempotency,
  - flow/recent lifecycle families and finalize->relayed trace behavior.

Result: `PASS`.

## Focused commands run

- `cargo test -p pwmd tests::v1_flow_recent_includes_roaming_finalize_lifecycle_events -- --exact`
- `cargo test -p pwmd tests::v1_roaming_intent_finalize_returns_500_when_snapshot_save_fails -- --exact`
- `cargo test -p pwmd tests::v1_roaming_intent_finalize_is_idempotent_for_terminal_statuses -- --exact`
- `cargo test -p pwmd tests::v1_roaming_intent_finalize_sets_relayed_and_is_idempotent -- --exact`

All commands: `PASS`.
