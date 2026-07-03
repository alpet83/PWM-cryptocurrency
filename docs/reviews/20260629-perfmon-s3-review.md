# Review: perfmon S3 — GET /v1/perfmon RPC (26dec63)

- date: 2026-06-29
- ticket: `20260629-perfmon-s3-review`
- coding_ticket: `20260629-perfmon-s3`
- commit: `26dec63`

## 1. Scope recap

Review commit `26dec63` — HTTP read path for S1/S2 perf counters:

| area | change |
|------|--------|
| `api/handlers_perfmon.rs` | NEW — `get_perfmon()` |
| `api/mod.rs` | `mod handlers_perfmon;` |
| `api/router.rs` | `.route("/v1/perfmon", get(get_perfmon))` |
| `perfmon.rs` | unchanged `PerfSnapshot` + `REGISTRY` (S1/S2) |

## 2. Requirements fit

| Focus area | Verdict | Evidence |
|------------|---------|----------|
| Route registration | **PASS** | `router.rs:32` — exact `/v1/perfmon`; no param route shadowing |
| Handler iteration | **PASS** | `handlers_perfmon.rs:9-12` — `REGISTRY.iter().map(snapshot).collect()` |
| `PerfSnapshot` serialization | **PASS** | Owned `u64` fields (`perfmon.rs:19-24`); `snapshot()` loads `Relaxed` (`:54-56`) |
| Module wiring | **PASS** | `mod.rs:11`; import in `router.rs:18` |
| Panic / error paths | **PASS** | Infallible static slice + atomic loads; no `unwrap` in handler |
| Auth / rate limit | **PASS** with nit | Open like `/v1/version` / `/v1/status` — lab OK |
| JSON response shape | **PASS** with nit | Array of objects; includes `fail` + `avg_ns_per_call` (see §3) |

## 3. Handler and response contract

```7:13:crates/pwmd/src/api/handlers_perfmon.rs
pub(super) async fn get_perfmon() -> Json<Vec<PerfSnapshot>> {
    Json(
        perfmon::REGISTRY
            .iter()
            .map(|entity| entity.snapshot())
            .collect(),
    )
}
```

- **No `State<App>`** — correct: counters are process-global statics; avoids lock on read path.
- **No `ensure_ready`** — counters exist from process start (may be zero pre-traffic); acceptable for observability (unlike `/v1/head` which needs chain state).
- **Deterministic order** — `REGISTRY` static array order (`perfmon.rs:98-103`): `ed25519_verify`, `state_apply`, `chain_seal`, `pool_drain`.

**JSON shape** (serde field names):

```json
[
  {
    "name": "ed25519_verify",
    "calls": 0,
    "success": 0,
    "fail": 0,
    "wall_ns": 0,
    "avg_ns_per_call": 0
  }
]
```

Ticket sketch omitted `fail` and `avg_ns_per_call` — both are useful derived fields from `snapshot()`; not a blocker.

**Snapshot consistency:** concurrent `finish` on entities may produce briefly skewed `fail` vs `calls-success` — acceptable for lab metrics (S1 review rationale).

## 4. Route table / regression

- New route prepended at `router.rs:32` — axum matches literal paths; no collision with `/v1/account/:id`, `/v1/tx`, etc.
- Existing routes unchanged; `DefaultBodyLimit` / CORS layers apply uniformly (`:67-68`).
- **No regression** observed on route wiring.

## 5. Style and module shape

- Handler file has `//!` banner (`handlers_perfmon.rs:1`).
- **Naming nit:** handler is `get_perfmon` while peers use `v1_*` (`v1_version`, `v1_status`) — cosmetic only.
- `PerfSnapshot` remains `pub(crate)` in `perfmon` — fine for in-crate JSON response; no public type leak beyond HTTP body.

### Wire JSON / u128

Wire JSON / u128: not applicable (local observability JSON only; all counter fields are `u64`).

## 6. Safety

- No panics in handler path.
- No trust-boundary parsing (GET, no body).
- **Observability exposure:** unauthenticated read of hot-path timings — acceptable for CY lab; production should document or gate (loopback / operator token pattern like `handlers_operator_log.rs`).

## 7. Tests

- No HTTP integration test for `GET /v1/perfmon` in `crates/pwmd/tests` or `src/tests`.
- Existing `perfmon` unit tests cover `snapshot()` math only.

`cargo test` / smoke `curl`: **UNVERIFIED** (shell unavailable).

## 8. Concurrency / parallelism

Handler reads four static `PerfEntity` atomics without locking. Safe concurrent with S2 instrumentation (`worker` threads + seal loop). No new shared mutex surfaces on the read path. Array build allocates `Vec` per request — fine for low-frequency polling.

## 9. Verdict

**Approve with nits**

Prioritized nits:

1. **API-1 (low):** Rename `get_perfmon` → `v1_perfmon` for router naming consistency.
2. **SEC-1 (low):** Document open access in runbook; consider loopback-only or bearer gate before production exposure.
3. **TEST-1 (low):** Add smoke test — `GET /v1/perfmon` returns 4 rows with expected `name` values.
4. **DOC-1 (low):** Note full JSON schema (`fail`, `avg_ns_per_call`) in `docs/FEATURES.md` or operator docs when perfmon section lands.

No blockers.

## 10. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260629-perfmon-s3-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 18000, "confidence": "medium" }`