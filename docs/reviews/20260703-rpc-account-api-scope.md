# Security scope: RPC / account / admin API layer (final Fable 5 pass)

- **date:** 2026-07-03
- **ticket:** `20260703-rpc-account-api-scope-review`
- **commit:** `fc16364e31959320eceb5c6bb8324419dd0b6517`
- **agent:** `pwm-review` (`pwm_review`)
- **purpose:** Final pre-publication security scope for account queries, chain/status RPC, admin/operator endpoints, cross-shard/roaming state, and lifecycle notification paths. Complements [`20260702-conservation-security-scope.md`](20260702-conservation-security-scope.md) and [`20260702-wire-rpc-security-scope.md`](20260702-wire-rpc-security-scope.md).
- **scope IN:** All `/v1/*` routes except deep re-analysis of `/v1/tx` admission (covered in wire scope); peer-wire fact merge as it affects roaming finality.
- **scope OUT:** Conservation queue internals, TUI, ClickHouse snapshot, pwm-cli wallet UX.

---

## 1. Executive summary

PWM exposes a **flat, largely unauthenticated HTTP API** on the node listener (CORS permissive on loopback; `PWM_CORS_ORIGINS` required on non-loopback). Global **256 KB `DefaultBodyLimit`** applies to all routes (`router.rs:74`). Account list queries were fixed (`9905187`) to snapshot under read lock before `await`; single-account and several status paths still hold `inner.read()` across async handshake work. **Cross-shard facts are not HTTP-POSTable** — ingestion is local seal, operator backfill, or **trusted peer wire** (`merge_cross_shard_facts(..., trusted=true)`). Roaming uses a height-TTL state machine (`Queued → Exported → Relayed → Imported | Expired | Failed`); relay 204 now marks **Relayed** not **Imported** (`b1772aa`). Highest pre-mainnet risks: **unauthenticated shutdown/bridge-reset/offchain batch**, **information disclosure via `/v1/status` and `/v1/accounts`**, **trusted-peer fact forgery surface**, and **no per-IP rate limits**.

---

## 2. HTTP endpoint inventory (beyond `/v1/tx`)

| Method | Path | Handler | Primary gates |
|--------|------|---------|---------------|
| GET | `/v1/accounts` | `v1_accounts` | `ensure_ready`; snapshot then per-account `foreign_home_lookup_state` |
| GET | `/v1/account/:id` | `v1_account` | `ensure_ready`; **read lock held across `await`** |
| GET | `/v1/head` | `v1_head` | `ensure_ready` |
| GET | `/v1/status` | `v1_status` | partial readiness (no hard fail if not ready) |
| GET | `/v1/flow/recent` | `v1_flow_recent` | `ensure_ready` |
| GET | `/v1/version` | `v1_version` | none observed |
| GET | `/v1/perfmon` | `v1_perfmon` | **none** |
| GET | `/v1/cross-shard/facts` | `v1_cross_shard_facts` | `ensure_ready`; read-only ledger export |
| POST | `/v1/cross-shard/backfill` | `v1_cross_shard_backfill` | `ensure_user_tx_allowed`, `ensure_bridge_federation_ok` |
| POST | `/v1/export-readiness` | `v1_export_readiness` | user tx + bridge + `validate_tx_shape` |
| POST | `/v1/roaming-intents` | `v1_roaming_intent_create` | user tx + bridge + shape |
| GET | `/v1/roaming-intents/:id` | `v1_roaming_intent_status` | `ensure_ready` |
| POST | `/v1/roaming-intents/:id/finalize` | `v1_roaming_intent_finalize` | user tx + bridge |
| POST | `/v1/export-provenance` | `v1_export_handoff_register` | user tx + `ensure_trusted_handoff_source` |
| POST | `/v1/peer/hello` | `v1_peer_hello` | dev profile **or** transport enabled |
| GET | `/v1/dev/peers` | `v1_dev_peers` | dev profile **or** transport |
| GET | `/v1/federation/shards` | `v1_federation_shards` | `ensure_ready` |
| POST | `/v1/offchain/batch` | `v1_off_batch` | **no `ensure_ready`** |
| GET | `/v1/offchain/batch/:id` | `v1_off_batch_get` | none |
| GET | `/v1/offchain/batch/:id/proof/:entry_index` | `v1_off_proof` | none |
| POST | `/v1/bridge-federation/reset` | `v1_bridge_federation_reset` | `ensure_ready` only |
| POST | `/v1/shutdown` | `v1_shutdown` | **no auth** |
| GET/POST/DELETE | `/v1/operator/log/override` | `v1_log_ovr_*` | loopback **or** `Bearer` `op_token` |
| GET/POST | `/v1/lab/seal/*` | lab seal handlers | loopback + proposer/`--lab-seal-api` |

Source: `crates/pwmd/src/api/router.rs:32–76`.

---

## 3. Attack surface map

| Area | Severity | Notes |
|------|----------|-------|
| Unauthenticated admin RPC | **High** | `POST /v1/shutdown`, `POST /v1/bridge-federation/reset` — no bearer/loopback gate (`handlers_shutdown.rs:126`, `handlers_bridge.rs:11`). |
| Information disclosure | **Medium** | `/v1/accounts`, `/v1/status`, `/v1/flow/recent`, `/v1/cross-shard/facts` expose balances, roaming stuck counters, peer topology, export IDs. |
| Account query lock contention | **Low–Medium** | `v1_accounts` fixed (`handlers_account.rs:60–63`); `v1_account` still awaits under read lock (`:102–119`). |
| Cross-shard fact trust | **Medium–High** | HTTP GET facts are read-only; **mutation** via trusted peer wire + `merge_cross_shard_facts(..., true)` (`state.rs:217–232`, `peer_session/mod.rs:483–496`). Untrusted wire facts ignored (`inbound.rs:709`, `trusted=false`). |
| Roaming state machine | **Medium** | Height TTL expiry (`roaming.rs:302–323`); Relayed→Imported only on trusted imported facts or local seal (`b1772aa`). |
| Offchain batch injection | **Medium** | `POST /v1/offchain/batch` accepts entries without tx gates (`handlers_offchain.rs:10–27`); ephemeral in-memory store. |
| Operator log override | **Low–Medium** | Token or loopback (`handlers_operator_log.rs:161–184`); misconfigured `op_token` on public bind. |
| Lab seal RPC | **Low** (dev) | Loopback-only (`handlers_lab_seal.rs:504–527`); can drive cluster seal steps on proposer. |
| Rate limiting / throttling | **High (ops)** | No per-IP or per-route throttle; only body size cap and internal queue bounds. |
| Lifecycle notifications | **Low** | `tx_events` broadcast channel (`lifecycle.rs:1974`) — **no external webhooks**. |
| CORS + public bind | **Medium** | Non-loopback requires `PWM_CORS_ORIGINS` (`lib.rs:101–126`); API itself still open to direct curl. |

---

## 4. Detailed findings per focus area

### 4.1 Endpoint enumeration and auth gaps

**Auth tiers observed**

1. **`ensure_ready`** — node finished init (`common.rs:645–653`).
2. **`ensure_user_tx_allowed`** — ready + not degraded + genesis guard clear (`:656–675`).
3. **`ensure_bridge_federation_ok`** — bridge trust not refused (`:678–688`).
4. **`ensure_trusted_handoff_source`** — handoff signer ∈ trusted live peers (`common.rs:171–218`).
5. **`ensure_lab_seal_ok`** — loopback IP (`handlers_lab_seal.rs:504–527`).
6. **`ensure_op_log_auth`** — loopback or `Bearer` matches `app.op_token` (`handlers_operator_log.rs:161–184`).

**Unauthenticated or weakly gated (Fable 5 priority)**

- `POST /v1/shutdown` — graceful stop + snapshot (`handlers_shutdown.rs:126–132`).
- `POST /v1/bridge-federation/reset` — clears federation refusal latch (`handlers_bridge.rs:11–18`).
- `POST /v1/offchain/batch` — builds Merkle batch anchored to tip (`handlers_offchain.rs:10–27`).
- `GET /v1/perfmon` — perf snapshots, no readiness check (`handlers_perfmon.rs`).
- `GET /v1/status` — extensive cluster/lease/genesis/roaming diagnostics even when not fully ready (`handlers_status.rs:34+`).

**No session identity model** — gates are network position (loopback), optional static bearer, or handshake trust state—not per-caller accounts.

---

### 4.2 Account state queries

**`GET /v1/accounts`** (`handlers_account.rs:55–90`)

- Snapshots `Account`, `PeerAccountView`, `pending_conservation` under short read guard (`:40–63`).
- Foreign accounts: `foreign_home_lookup_state` after lock drop (`:71–78`).
- Exposes `pending_conservation` per account (see conservation scope).

**`GET /v1/account/:id`** (`:93–130`)

- Still holds `g.inner.read().await` through `foreign_home_lookup_state(...).await` (`:102–119`).
- **Residual seal-starvation / contention risk** (nit from `20260703-accounts-lock-snapshot-review`).

**`acct_out_for_runtime`** (`common.rs:419–479`) — splits local vs foreign balance semantics; relies on peer view freshness.

---

### 4.3 Chain / block query endpoints

| Endpoint | Data exposed | Lock pattern |
|----------|--------------|--------------|
| `/v1/head` | `tip_h`, `tip_hash` | brief `read()` (`handlers_status.rs:16–22`) |
| `/v1/status` | phase, genesis guard, roaming stuck metrics, lease, peers | multiple `read()` scopes |
| `/v1/flow/recent` | last 256 flow trace rows (tx ids, export ids, notes) | `read()` clone deque (`:25–31`) |
| `/v1/version` | build metadata | static |

No block body / tx list download RPC on this router — sync is peer wire, not public HTTP bulk export.

---

### 4.4 Cross-shard fact ingestion

**Ingestion paths (no arbitrary HTTP POST)**

| Path | Trust | Effect |
|------|-------|--------|
| Local `record_export` / `record_import` on seal | consensus | Updates `CrossShardLedger` + chain `imported_set` |
| `POST /v1/cross-shard/backfill` | operator-triggered; peer status trust check | `record_handoff` + synthetic `Import` via `v1_tx` (`handlers_backfill.rs:179–236`) |
| Peer wire `CrossShardFacts` (trusted session) | `trusted=true` | `merge_cross_shard_facts` → may `mark_import_by_export` (`state.rs:228–229`) |
| Peer wire `CrossShardFacts` (inbound stub) | `trusted=false` | **No merge** (`inbound.rs:703–709`) |

**`upsert` monotonicity** (`ledger.rs:286–312`): rejects status **downgrade** (`rank` comparison); local origin preserved over peer overlay; `FACT_CAP=4096` with eviction of oldest `last_height`.

**Replay:** Facts keyed by `export_id`; duplicate upserts idempotent if unchanged. Backfill skips `DUPLICATE_IMPORT_ERR_TEXT` (`handlers_backfill.rs:224–227`).

**Open questions:** What fields must a trusted peer prove for `CrossShardStatus::Imported`? Can a compromised trusted peer mark imports never sealed on target?

---

### 4.5 Roaming pool state

**State machine** (`roaming.rs:67–94`)

```text
Queued → Exported → Relayed → Imported (terminal)
                    ↘ Expired / Failed (terminal)
```

- **Locking statuses:** Queued, Exported, Relayed (`is_locking`) — block sender via `active_locks` (`:292–294`).
- **TTL:** `expire_by_height` on seal tick and several handlers (`:302–323`).
- **`mark_relayed_by_export`** on relay 204 (`relay.rs:631`, `b1772aa`).
- **`mark_import_by_export`** on trusted imported facts or local import seal (`state.rs:229`, `handlers_tx.rs:206`).

**Concurrent access:** `RoamingPool` mutated under `inner.write()` in handlers; reads under `read()` in status. No fine-grained lock — whole `Inner` RwLock.

**Finalize path** (`handlers_roaming.rs:262–385`): may call `relay_handoff`; snapshot rollback on persist failure.

---

### 4.6 Lifecycle finality notifications

- **No HTTP webhooks or outbound callbacks** on seal.
- **Internal:** `tokio::sync::broadcast::Sender<TxEvent>` with `Sealed` / `Rejected` (`pipeline/queue.rs:92–100`, `lifecycle.rs:1974`).
- **Flow trace:** `push_flow` / `recent_flow` deque (cap 256) — readable via `/v1/flow/recent`.
- Crafted RPC input cannot directly invoke external URLs; indirect exfil only via logs/metrics if operator scrapes them.

---

### 4.7 Rate limiting and resource exhaustion

| Control | Coverage | Gap |
|---------|----------|-----|
| `DefaultBodyLimit` 256 KB | all POST bodies | GET unbounded response size (accounts list scales with account count) |
| `facts` query `limit` clamp | 1–4096 (`handlers_backfill.rs:52`) | `/v1/accounts` no pagination |
| Worker queue cap | `/v1/tx` path | other POSTs unbounded concurrency |
| `FACT_CAP` / flow cap | ledger / flow deque | offchain batches unbounded count? |
| Per-IP throttle | **none** | network DoS on any open endpoint |

**CORS:** Permissive on loopback; production bind needs explicit origins (`lib.rs:101–126`).

---

### Concurrency / parallelism

- **`inner` RwLock:** Account queries, roaming mutations, seal loop — write-preferring; long read scopes block seal.
- **`handshake_read/write`:** Separate async locks; `foreign_home_lookup_state` and backfill iterate transport state without holding `inner` write.
- **Backfill loop:** Sequential `v1_tx` per fact — can hold system busy; uses validator key from genesis (`pick_backfill_signer`).
- **Test gaps:** No stress test for `/v1/accounts` + seal concurrency post-9905187; no auth negative tests for shutdown.

---

## 5. Numbered invariants (Fable 5)

1. **I1:** No unauthenticated HTTP call may stop the node, reset bridge trust, or mutate consensus chain state without a valid `SignedTx`.
2. **I2:** `CrossShardStatus::Imported` on the source shard MUST NOT advance from peer HTTP 204 alone (post `b1772aa`).
3. **I3:** Untrusted peer wire MUST NOT promote roaming intents or cross-shard facts (`trusted=false` paths).
4. **I4:** Trusted peer imported facts MUST NOT create double-credit on source if target never sealed import.
5. **I5:** Roaming `active_locks` MUST release on terminal states (Imported, Expired, Failed).
6. **I6:** Account query endpoints MUST NOT hold `inner.read()` across `.await` (list fixed; single-account pending).
7. **I7:** No RPC response field may leak validator private keys or `op_token` values.

---

## 6. Recommended Fable 5 prompt

---

**Task:** Final pre-publication security audit of PWM **account/admin RPC layer** at commit `fc16364`. Prior scopes cover conservation transfer and `/v1/tx` wire admission — **do not re-audit those except where this layer intersects** (roaming, cross-shard, account fields).

**Threat model:** Internet-facing node on non-loopback bind; unauthenticated scanner; malicious trusted peer; operator mistake (open shutdown); concurrent load from `/v1/accounts` + seal.

**Files and line ranges**

| File | Lines | Focus |
|------|-------|-------|
| `crates/pwmd/src/api/router.rs` | 31–76 | Route table + body limit |
| `crates/pwmd/src/lib.rs` | 101–126 | CORS policy |
| `crates/pwmd/src/api/common.rs` | 419–479, 645–688, 171–218, 490–520 | Account output, gates, foreign lookup |
| `crates/pwmd/src/api/handlers_account.rs` | 33–130 | List vs single account locks |
| `crates/pwmd/src/api/handlers_status.rs` | 16–32, 34–333 | head/flow/status disclosure |
| `crates/pwmd/src/api/handlers_backfill.rs` | 46–248 | Facts read + operator backfill |
| `crates/pwmd/src/api/handlers_roaming.rs` | 30–450 | Readiness, intents, finalize, handoff |
| `crates/pwmd/src/api/handlers_shutdown.rs` | 126–132 | Shutdown RPC |
| `crates/pwmd/src/api/handlers_bridge.rs` | 11–18 | Federation reset |
| `crates/pwmd/src/api/handlers_offchain.rs` | 10–69 | Offchain batch |
| `crates/pwmd/src/api/handlers_operator_log.rs` | 161–184 | Operator auth |
| `crates/pwmd/src/api/handlers_lab_seal.rs` | 504–527 | Lab gate |
| `crates/pwmd/src/relay.rs` | 627–641 | Relayed not imported |
| `crates/pwmd/src/roaming.rs` | 67–323, 278–287 | State machine |
| `crates/pwmd/src/state.rs` | 217–232 | Fact merge → roaming |
| `crates/pwmd/src/ledger.rs` | 104–312 | Fact upsert semantics |
| `crates/pwmd/src/transport/peer_session/mod.rs` | 483–496 | Trusted merge gate |
| `crates/pwmd/src/transport/peer_session/inbound.rs` | 703–709 | Untrusted wire facts |
| `crates/pwmd/src/lifecycle.rs` | 1872–1978 | Seal + tx_events |

**Attack scenarios**

- **A:** `POST /v1/shutdown` and `POST /v1/bridge-federation/reset` from remote — document impact; recommend mitigations.
- **B:** Flood `GET /v1/accounts` while submitting txs — measure seal latency post-snapshot fix; compare `v1_account`.
- **C:** Trusted peer sends forged `CrossShardFacts` Imported — does source roaming + ledger accept without target seal proof?
- **D:** Relay 204 without target import — intent stays Relayed until TTL; confirm lock release and operator visibility (`stuck_relayed_without_import`).
- **E:** `POST /v1/cross-shard/backfill` with malicious `peer_base` — trust_peer_status bypass attempts.
- **F:** `POST /v1/export-provenance` with valid sig but wrong peer identity — `ensure_trusted_handoff_source` rejection paths.
- **G:** `POST /v1/offchain/batch` huge batch / many batches — memory and CPU bounds.

**Deliverable:** Ranked findings (Critical/High/Medium/Low/Info); separate **pre-mainnet hardening** checklist from consensus-critical issues.

---

## 7. Verdict

**Approve with nits** — scope document complete for final Fable 5 pass. Residual API surface is broad and mostly unauthenticated by design (devnet/MVP); explicit high-severity candidates are shutdown/bridge-reset/offchain batch and missing rate limits.

### Nits

1. Apply account snapshot pattern to `v1_account`.
2. Add auth (loopback/token) to shutdown, bridge-reset, offchain batch before public deployment.
3. Paginate or rate-limit `/v1/accounts` and `/v1/cross-shard/facts`.
4. Document trusted-peer threat model for cross-shard fact promotion.

---

## 8. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260703-rpc-account-api-scope.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 42000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260703-rpc-account-api-scope.md'
git commit -m 'docs(v7): RPC account API security scope for Fable 5 (fc16364)'
```