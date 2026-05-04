# Sprint 14 Slice 16 — coding report

## Scope implemented

- Added explicit operator finalize endpoint for manual relay contract:
  - `POST /v1/roaming-intents/:id/finalize`
- Improved idempotent/operator-facing finalize responses:
  - `status`, `changed`, and deterministic `message`
- Extended tx/intent observability in `/v1/flow/recent` with lifecycle events:
  - `accepted:*`, `applied:*`, `exported:*`, `imported:*`, `sealed:*`, `roaming_status:*`, `finalized:*`
- Unified persistence error surfacing for updated paths:
  - `500` on snapshot save failure after finalize
  - `500` on snapshot save failure when status lookup triggers TTL-expire mutation

## Operator sequence (manual handoff, deterministic)

1. `POST /v1/roaming-intents` with signed `EXPORT` (home shard).
   - Expected: `status=exported`
2. Relay provenance to target shard (operator action outside `pwmd` API).
3. `POST /v1/roaming-intents/:id/finalize` (home shard).
   - First finalize: `status=relayed`, `changed=true`
   - Retry finalize: `status=relayed`, `changed=false`, stable message
4. Submit `IMPORT` on target shard.
   - Intent eventually becomes `imported` (via import path update).
5. Track lifecycle and diagnostics:
   - `GET /v1/roaming-intents/:id`
   - `GET /v1/flow/recent`

## Files changed

- `crates/pwmd/src/api.rs`
- `crates/pwmd/src/roaming.rs`
- `crates/pwmd/src/lib.rs` (tests)
- `crates/pwmd/Cargo.toml` (`pwmd` marker bump `0.1.9 -> 0.1.10`)
- `docs/pwmd.md`
- `docs/reviews/sprint-14-slice16-coding.md`

## Open risks

- Finalize currently marks operator handoff (`relayed`) on source shard only; it does not cryptographically confirm target-shard import completion by itself.
- `/v1/flow/recent` is in-memory bounded trace and not durable audit storage.
