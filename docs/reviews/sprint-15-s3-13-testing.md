# Sprint 15 S3.13 — Federation table (testing handoff)

## Environment

- Repo: `P:/opt/docker/PWM-cryptocurrency`
- Host: Windows; toolchain via PowerShell `cargo`.
- Date: 2026-05-01

## Commands

| Step | Command | Result | Duration (wall) | Hang watchdog |
|------|---------|--------|-----------------|---------------|
| Format | `cargo fmt --check -p pwmd` | **PASS** | ~1.4s | no |
| Federation unit | `cargo test -p pwmd federation -- --nocapture` | **PASS** (4/4) | ~0.4s | no |
| Full lib | `cargo test -p pwmd --lib -- --nocapture` | **FAIL** (175 passed, **16 failed**) | ~3.4s | no |
| Binary refresh (smoke prerequisite) | `cargo clean -p pwmd` + `cargo build -p pwmd --bin pwmd` | **PASS** | ~12s (clean+build) | no |

## `cargo test -p pwmd federation`

All four tests in `crates/pwmd/src/federation.rs` passed:

- `fallback_shard_key_maps_cluster`
- `merge_height_monotonic_and_seen_max`
- `sweep_drops_expired`
- `view_health_semantics`

## `cargo test -p pwmd --lib` — failure classification (16)

**Verdict:** none of the failures are plausibly caused by the **S3.13 federation table** slice (merge/TTL/`GET /v1/federation/shards`). Federation-focused tests are green. The failures cluster on **export-readiness (HTTP 409)** not being exercised in older tests, plus **unrelated** lifecycle/E2E issues.

| Test | Symptom | Likely cause vs S3.13 |
|------|---------|------------------------|
| `tests::v1_roaming_intent_create_and_get_status` | 409 vs 200 | **Pre-existing / readiness drift** — create posts `Export` without `POST /v1/export-readiness`; API returns `409` + export readiness JSON (`api.rs` `consume_readiness`). |
| `tests::v1_roaming_intent_create_is_idempotent_for_duplicate_export_delivery` | 409 vs 200 | Same **readiness** pattern. |
| `tests::v1_roaming_intent_expires_by_ttl_height` | 409 vs 200 | Same. |
| `tests::v1_roaming_intent_finalize_is_idempotent_for_terminal_statuses` | 409 vs 200 | Same. |
| `tests::v1_roaming_intent_lock_blocks_competing_local_tx` | 409 vs 200 | Same. |
| `tests::v1_roaming_intent_returns_500_when_snapshot_save_fails` | 409 vs 500 | Fails before reaching snapshot failure branch — **readiness** blocks first. |
| `tests::v1_roaming_intent_status_returns_500_when_expire_snapshot_save_fails` | 409 vs 200 | Same **readiness** pattern for happy path. |
| `tests::v1_status_and_tx_do_not_deadlock_with_snapshot_persist` | 400 vs 204 | **Not federation** — concurrent `/v1/status` + `/v1/tx` harness mismatch (likely body/readiness or validation). |
| `tests::v1_tx_accepts_export` | 409 vs 204 | **Export-readiness** — tx path requires readiness preflight. |
| `tests::v1_tx_accepts_import_after_export` | 409 vs 200 | Same chain: export leg blocked by 409. |
| `tests::v1_tx_rejects_duplicate_import_with_conflict` | 409 vs 200 | Import-only leg never reached. |
| `tests::v1_tx_rejects_import_unknown_export_id` | 409 vs 200 | Same. |
| `tests::v1_tx_rejects_invalid_import_with_bad_request` | 409 vs 200 | Same. |
| `tests::v1_tx_two_node_smoke_cy_to_do_with_negative_suite` | 409 vs 204 | Export path **409** — readiness. |
| `slice20_e2e_tests::slice20_two_shard_e2e_flows_contract` | CLI exit 2: recipient not on RPC | **E2E harness / init ordering** — not federation HTTP contract. |
| `lifecycle::tests::seal_writes_snapshot_file_when_data_file_is_configured` | snapshot file missing after waits | **Seal/snapshot timing or persistence** — orthogonal to federation route. |

**For coding (pwm-coding):** if the repo gate is `cargo test -p pwmd --lib`, the actionable work is **update tests** to call `POST /v1/export-readiness` (and assert `409` where readiness is intentionally absent), and separately fix **lifecycle** / **slice20 E2E** expectations — not rollback federation merge.

## Smoke HTTP — `GET http://127.0.0.1:3030/v1/federation/shards`

**First attempt (without rebuild):** `404 Not Found` on `/v1/federation/shards` while `/v1/head` and `/v1/status` returned **200** — consistent with a **stale** `target/debug/pwmd.exe` (source already contained the route in `api.rs`).

**After** `cargo clean -p pwmd` + `cargo build -p pwmd --bin pwmd`:

- **Single node** (`127.0.0.1:3030`, genesis `tmp/genesis-custom.json`, isolated state dir): **200**, JSON includes `generated_at_unix_ms`, `ttl_sec`, `view_health`, `expected_shard_count`, `active_shard_count`, `stale_shard_count`, `rows[]` with row fields per review §B (`shard_id`, `latest_height`, `last_seen_unix_ms`, `ttl_sec`, `expires_at_unix_ms`, `source`, `source_node_id`, `fresh`).
- **Two nodes** (mirrors `node-1.ps1` / `node-2.ps1` ports **3030** / **3031**, peers **3130** / **3131`): **200** on node-1; sample response had `view_health":"complete"`, `active_shard_count":2`, two `rows` (`CY` from `status`, `DO` from `hello`), counters consistent with contract.

**Cleanup:** all spawned `pwmd` processes stopped via `Stop-Process` on known PIDs and `Get-Process pwmd | Stop-Process -Force`; verified no stray `pwmd` left before finishing.

## Overall testing verdict

- **S3.13 federation slice (unit + live route after rebuild):** satisfied for this handoff.
- **`cargo test -p pwmd --lib`:** **FAIL** due to **non-federation** failures (readiness/test drift + lifecycle + slice20 E2E); **not** classified as S3.13 federation regression.

---

```yaml
agent: pwm-testing
result: PARTIAL
artifacts:
  - docs/reviews/sprint-15-s3-13-testing.md
commands:
  - cmd: "cargo fmt --check -p pwmd"
    duration_sec: ~1.4
    pass_fail: PASS
    hang_watchdog: no
  - cmd: "cargo test -p pwmd federation -- --nocapture"
    duration_sec: ~0.4
    pass_fail: PASS
    hang_watchdog: no
  - cmd: "cargo test -p pwmd --lib -- --nocapture"
    duration_sec: ~3.4
    pass_fail: FAIL
    hang_watchdog: no
  - cmd: "cargo clean -p pwmd; cargo build -p pwmd --bin pwmd"
    duration_sec: ~12
    pass_fail: PASS
    hang_watchdog: no
  - cmd: "HTTP GET http://127.0.0.1:3030/v1/federation/shards (pwmd after rebuild; one- and two-node smoke)"
    duration_sec: ~10–11 each run
    pass_fail: PASS
    hang_watchdog: no
cleanup:
  cleaned: yes
  killed: pwmd test processes (explicit Stop-Process + sweep)
  artifact_cleanup: none required (temp state under tmp/smoke-s15-* left on disk; processes gone)
token_usage:
  source: estimate
  input: null
  output: null
  total: 12000
  confidence: low
```
