# Sprint 15 S3.12.8 — Live testing: one-window foreign balance + federation table

**Agent:** pwm-testing  
**Date:** 2026-04-30 (local wall clock, TZ Europe/Moscow for HTTP poll timestamps)  
**Environment:** Windows 10, repo `P:\opt\docker\pwm-protocol`, ports **3030/3031** HTTP and **3130/3131** transport (defaults from `node-1.ps1` / `node-2.ps1` — no port conflict; alternate ports **not** required).

## Commands (planned vs executed)

| Step | Command | Duration (approx) | Result | Hang watchdog |
|------|---------|-------------------|--------|-----------------|
| Preflight | `Get-Process pwmd` / stop if any | &lt;1s | PASS | no |
| Start CY | `powershell -NoProfile -ExecutionPolicy Bypass -File .\node-1.ps1` | spawned ~0.12s | PASS | no |
| Start DO | `powershell -NoProfile -ExecutionPolicy Bypass -File .\node-2.ps1` | spawned ~0.12s | PASS | no |
| Wait ready | Loop `Invoke-RestMethod http://127.0.0.1:{3030,3031}/v1/status` until `ready==true` | **49s** | PASS | no (120s cap) |
| Trusted stability | 6× `GET /v1/status` on CY (3030) | ~3s | PASS | no |
| Foreign CY→DO | 8× `GET /v1/account/<DO-domain funding acct>` on CY | ~3.5s | PASS | no |
| Foreign DO→CY | 5× `GET /v1/account/<CY-domain funding acct>` on DO | ~2s | PASS | no |
| Federation probe | `GET /v1/federation/shards` on 3030 and 3031 | &lt;1s | **404** (see verdict) | no |
| Dev peers sample | `GET /v1/dev/peers` on CY | &lt;1s | PASS | no |
| Cleanup | `Get-Process pwmd \| Stop-Process -Force` | ~1s | PASS | no |
| Artifacts | Remove `target\debug\incremental` if present | &lt;1s | PASS (dir empty or cleared) | no |

**Note:** Node wrappers were started via `Start-Process` with stdout redirected to `tmp/s3-12-8-node1.log` / `tmp/s3-12-8-node2.log` (same underlying commands as the root scripts). Build artifact `target\debug\pwmd.exe` was already present; first **ready** still waited on **snapshot load** (~56s wall from process start to ready on CY log).

## Live evidence (timestamps / key lines)

### CY node log (`tmp/s3-12-8-node1.log`)

- `[20:15:12.962]` `pwmd listen http://127.0.0.1:3030` … `shard=CY` … `domain-hi-0x2c` … `test-node-CY`
- `[20:15:20.490]` `peer hello accepted node_id=local-node-DO` … `domain_hi=0x32 class=foreign`
- `[20:16:09.172]` `pwmd startup phase: ready (snapshot loaded)`
- `[20:16:10.898]` … `peer account views merged count=1 source=local-node-DO` (repeats during observation window — trusted stream active)

### HTTP — trusted session / relay (CY `GET /v1/status`, 6 polls)

```
poll1 t=2026-04-30T23:16:13.3725007+03:00 trusted_total=1 relay_health=ok live=1 trusted_relay=1
… (polls 2–5 identical fields) …
poll6 t=2026-04-30T23:16:15.4910527+03:00 trusted_total=1 relay_health=ok live=1 trusted_relay=1
```

### HTTP — foreign account lookups (dynamic)

**CY (3030) → home on DO:** account `32ecaa3884011f2c21bf09b05e835ec1df5545bebb2c6c478dcacfb70e7fc1c5` (genesis funding row, domain prefix `0x32…`)

- All 8 samples: `home_lookup_status=ok`, `authoritative_home_balance=1000000`, `authoritative_home_initialized=True`, `local_view_only=True` (foreign on CY).
- **No** flip to `unavailable` during the burst.

**DO (3031) → home on CY:** account `2cfb1e1d7001d108b39e05b194f2d1b126931bbfef38506e34297a5474ddae5e`

- All 5 samples: `home_lookup_status=ok`, `authoritative_home_balance=400000`, `authoritative_home_initialized=True`, `local_view_only=True`.

### HTTP — `GET /v1/dev/peers` (CY)

- `peers_count=1`, peer `local-node-DO`, `domain_hi=50` (0x32), `class=foreign`, `status=connected`.

## Verdict: one-window foreign balance / init visibility

**PASS**

- Trusted one-window path is **stable** over rapid status polls (`peer_relay_health=ok`, `peer_session_trusted_total=1`, live peer present).
- **CY→DO** and **DO→CY** foreign `GET /v1/account/:id` return **`home_lookup_status=ok`** with non-null authoritative balance and init flag; **no regression to `unavailable`** in-session.
- Logs show continuous **`peer account views merged`** from `local-node-DO`, consistent with authoritative fields staying fresh.

## Verdict: federation table on running main nodes

**MISSING** (not **partial** — no JSON contract on wire)

- **`GET http://127.0.0.1:3030/v1/federation/shards`** → **404**
- **`GET http://127.0.0.1:3031/v1/federation/shards`** → **404**
- **Code cross-check:** `crates/pwmd/src/api.rs` `router()` registers `/v1/status`, `/v1/head`, `/v1/accounts`, …, `/v1/dev/peers` — **no** `/v1/federation/shards` route. TTL / `view_health` / `generated_at_unix_ms` **cannot** be exercised on these binaries.

**Gap list (for S3.13 / coding slice):**

1. Implement `GET /v1/federation/shards` (or agreed alias) on `pwmd` and register it on the production router.
2. Acceptance tests: response shape (`ttl_sec`, `view_health`, row list), eviction/freshness semantics per design (`docs/reviews/sprint-15-s3-11-federation-and-reconnect-review.md` § API).
3. Optional: operator doc in `docs/pwmd.md` route list (currently matches “no federation route” reality).

## Blocker classification + recommended next slice

| Item | Classification |
|------|----------------|
| One-window foreign balance/init on node scripts | **Non-blocker** — behavior matches intent post–S3.12.7. |
| Federation HTTP table | **Product gap** (not a test flake): **blocked on implementation** for any “federation API readiness” gate — track under **S3.13** (or dedicated federation slice), not S3.12.8 remediation. |

**Recommended next slice:** coding / S3.13 — wire `GET /v1/federation/shards` + tests; then re-run this live script against `node-1.ps1` / `node-2.ps1` for TTL/`view_health` evidence.

## Cleanup report

- **pwmd:** stopped via `Stop-Process -Force` on all `pwmd` after checks; post-check `Get-Process pwmd,pwm-tui` showed **none**.
- **pwm-tui:** not started during this run.
- **Logs:** `tmp/s3-12-8-node1.log`, `tmp/s3-12-8-node2.log` (+ optional `.err` siblings if created) left for review; not part of git commit per task.
- **Build:** scoped removal of `target/debug/incremental` attempted; negligible or empty on this host.

---

_End of S3.12.8 testing report._
