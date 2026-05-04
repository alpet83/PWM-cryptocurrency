# Sprint 14 — Slice 15 coding report

## Scope delivered

- Persistence strictness: snapshot-save errors are no longer warn-only for `POST /v1/tx` and `POST /v1/roaming-intents`; API returns `500` with explicit failure text.
- Runtime persistence visibility: `pwmd` now logs resolved snapshot path on startup and promotes runtime to `ready_degraded` on background seal snapshot failures (with `snapshot_error` visible in `/v1/status`).
- Autosnapshot policy: added block-based checkpoint guarantee every `100` blocks via `AUTOSNAPSHOT_BLOCK_INTERVAL`.
- Cross-shard observability: roaming status now exposes explicit relay contract fields (`relay_mode`, `relay_hint`) to avoid ambiguous "sent but vanished" state when auto-relay is not implemented.
- History/trace visibility: added `GET /v1/flow/recent` (bounded in-memory trace of recent accepted/sealed/roaming-status events).

## Files changed

- `crates/pwmd/src/api.rs`
- `crates/pwmd/src/lifecycle.rs`
- `crates/pwmd/src/state.rs`
- `crates/pwmd/src/bootstrap.rs`
- `crates/pwmd/src/lib.rs` (tests)
- `docs/pwmd.md`
- `docs/tester-guide-devnet-smoke.md`

## Notes

- Kept implementation additive and focused in `pwmd`; no architecture rewrite.
- Existing save points remain; periodic block checkpoint is now explicit and documented.
