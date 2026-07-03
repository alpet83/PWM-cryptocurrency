# Security scope: wire (SignedTx) + RPC handler layer

- **date:** 2026-07-02
- **ticket:** `20260702-wire-rpc-security-scope-review`
- **commit:** `8fbc22681a2c54eee87b4b077748a6026d6f76b8`
- **agent:** `pwm-review` (`pwm_review`)
- **purpose:** Structured attack-surface scope for Fable 5 deep analysis of JSON tx deserialization and `/v1/*` RPC admission. **Not** a pass/fail code verdict — deliverable is this scope document.
- **scope IN:** `SignedTx` / `TxBody` JSON wire, `ser_json_u128`, `/v1/tx` admission, worker precheck pipeline, import relay, body limits.
- **scope OUT:** Conservation transfer path ([`20260702-conservation-security-scope.md`](20260702-conservation-security-scope.md)), TUI, ClickHouse snapshot.

---

## 1. Executive summary

PWM accepts `SignedTx` JSON on `POST /v1/tx` with a **256 KB global `DefaultBodyLimit`** on the API router. Amount fields use `pwm-core` `ser_json_u128` (decimal string or `u64` JSON number only). Structural validation (`validate_tx_shape`) runs on **all** worker-queued txs and on the direct-seal path for Export/Import/ClaimIPv4Batch; however **async worker admission returns HTTP 204 before seal**, so clients can be acked while seal later evicts the tx. Import relay forwards the same JSON blob to a peer’s `/v1/tx`, which re-runs the full handler. Highest-value Fable 5 targets: **partial `U128Visitor` coverage**, **TOCTOU on import provenance prefilter**, **post-ack seal failure window**, **`SignedTx` fields not covered by `signing_message`**, and **`/v1/accounts` read-lock duration**.

---

## 2. Attack surface map

| Area | Severity estimate | Notes |
|------|-------------------|-------|
| `ser_json_u128` / `U128Visitor` | **Medium** | Only `visit_str` / `visit_u64`; negative/float/large JSON numbers likely fail at deserialize; long decimal strings unbounded parse (`ser_json_u128.rs:48–50`). `pwmd::wire_serde::de_u128_compat` is stricter but **not** used for `SignedTx`. |
| `validate_tx_shape` coverage | **Low–Medium** | Worker path calls `validate_tx_shape` (`worker.rs:342`); direct-seal path calls it for Export/Import/ClaimIPv4Batch (`handlers_tx.rs:125`). `init_v4` on non-Init rejected (`tx.rs:647`). Gap: **unsigned envelope fields** (`import_fee`, `import_provenance`, `burn_purpose` on wrong body) not always rejected. |
| `V1_TX_BODY_LIMIT` (256 KB) | **Low** | Applied to **entire** router via `.layer(DefaultBodyLimit::max(...))` (`router.rs:74`) — all POST bodies capped; GET endpoints unaffected. |
| `/v1/accounts` read-lock DoS | **Medium** | Holds `inner.read()` across per-account loop **and** `await foreign_home_lookup_state` (`handlers_account.rs:36–51`, `common.rs:490+`) — lock contention / slow peer handshake extends critical section. |
| `relay_import` | **Medium** | Forwards full `SignedTx` to peer `POST /v1/tx` (`relay.rs:558`); target re-validates independently. `select_target` polls `/v1/status` then posts — topology race window. Source shard marks roaming imported even on relay success without local seal (`relay.rs:627–631`). |
| Import provenance TOCTOU | **Medium–High** | `enforce_import_provenance_prefilter` checks `imported_set` under read lock (`tx_policy.rs:98`); seal marks imported later under write lock. Concurrent duplicate `export_id` submissions may both pass prefilter. |
| `TxBody` / `SignedTx` strictness | **Low–Medium** | `SignedTx` has no `deny_unknown_fields`; extra top-level keys **ignored** by serde. `TxBody` custom deserializer: unknown variant fields ignored per variant struct defaults. Forward-compat hazard: ignored fields stored nowhere but clients may rely on them later. |
| Unsigned envelope malleability | **Medium** | `import_fee` / `import_provenance` included in `signing_message` **only** for `Import` (`tx.rs:500–518`); on `Transfer`/`Policy` they are not signed — wire tampering without invalidating signature if verifier only checks sig. |
| `computed_account_id` | **Low** | `blake3(signer_pk ‖ derivation_index_le)` (`hd.rs:9–13`); recomputed each call, no cache. Collision resistance = blake3 preimage resistance; domain enforced separately (`tx.rs:633–634`). |
| Precheck vs seal parity | **Medium** | Worker: `validate_tx_shape` + `precheck_apply_with_ctx` (`worker.rs:339–416`). Seal: `apply_prechecked_tx` if `validated_at_height == tip_before`, else full `apply_tx_with_ctx` (`chain.rs:188–195`). **204 returned before seal** (`handlers_tx.rs:261–264`); seal failure evicts tx silently (`lifecycle.rs:2073–2129`). |

---

## 3. Detailed findings per focus area

### 3.1 `ser_json_u128` / `U128Visitor`

**Observed behavior** (`crates/pwm-core/src/ser_json_u128.rs`)

- Serialize: decimal string (`serialize_str`).
- Deserialize via `deserialize_any(U128Visitor)` with handlers: `visit_str`, `visit_string`, `visit_u64` only.
- No `visit_i64`, `visit_f64`, `visit_u128`, `visit_i128`.

**Per input type (expected serde_json behavior)**

| JSON input | Expected outcome | Fable 5 should confirm |
|------------|------------------|------------------------|
| Negative integer (`-1`) | `visit_i64` → **unimplemented** → deserialize error | Yes — reject, no partial state |
| Float (`1.5`) | `visit_f64` → **unimplemented** → error | Yes |
| Integer `> u64::MAX` as JSON number | serde_json typically **out of range** before visitor; if passed to `visit_u64`, truncate/wrap N/A (visitor not reached) | Confirm serde_json version behavior |
| String `"007"` | `parse::<u128>()` → `7` (leading zeros accepted) | Semantic ambiguity only |
| `""` | Parse error: `"invalid decimal string for u128"` | Safe reject |
| Very long numeric string | Unbounded `parse` — **CPU DoS** within 256 KB body budget | Bound string length? |
| `"2e30"` | Parse fails (not valid `u128` decimal) | Safe reject |
| Decimal string > u128::MAX | Parse error | Safe reject |

**Contrast:** `crates/pwmd/src/wire_serde.rs` `de_u128_compat` adds `visit_i64` (non-negative), hex prefix support — used in `api/types.rs` and ledger wire, **not** in `SignedTx` amount fields.

**Open questions**

- Should RPC `SignedTx` adopt `de_u128_compat` for parity with peer ledger types?
- Is there a normative RFC requiring decimal-string-only for tx amounts?

---

### 3.2 `validate_tx_shape` coverage gap

**Two admission paths in `handlers_tx.rs`**

1. **Direct seal** (Export, Import, ClaimIPv4Batch): `validate_tx_shape` at line 125, then immediate `chain.seal`.
2. **Worker queue** (Transfer, Policy, Init, Stake, etc.): `run_worker_precheck` → worker `precheck_client` → **`validate_tx_shape`** at `worker.rs:342`.

**No bypass for Transfer/Policy reaching `apply_tx` without `validate_tx_shape`** on the worker path. Direct-seal types always call it explicitly.

**`apply_prechecked_tx` / `validate_shape_no_sig`:** Used at seal when `PreValidated` entry matches tip (`chain.rs:190–191`); skips **Ed25519 re-verify** only (`state.rs:386–390`), not shape rules.

**Residual gaps**

| Condition | `validate_tx_shape` | Notes |
|-----------|---------------------|-------|
| `init_v4` on Transfer/Policy | **Rejected** (`tx.rs:647`) | Safe |
| `target_account != computed_account_id` on Policy | **Rejected** (`tx.rs:671–672`) | Safe |
| `import_fee` on Transfer | **Not rejected** | Not in Transfer `signing_message` — malleable post-sign |
| `import_provenance` on non-Import | **Not rejected** | Ignored at apply for non-Import bodies (verify) |
| `burn_purpose` on non-BurnMark | **Not rejected** | Not in signing for non-BurnMark |

Fable 5 should trace whether malleable envelope fields affect fee, policy, or cross-shard bookkeeping.

---

### 3.3 `V1_TX_BODY_LIMIT` and endpoint limits

**Observed**

- `V1_TX_BODY_LIMIT = 256 * 1024` (`api/types.rs:16`).
- `router.rs:74`: `.layer(DefaultBodyLimit::max(V1_TX_BODY_LIMIT))` on the **root Router** — applies to **all routes** on this router, not only `/v1/tx`.
- GET handlers (`/v1/accounts`, `/v1/account/:id`, `/v1/head`, …) have negligible request bodies; limit is effectively relevant for POST endpoints (`/v1/tx`, roaming, offchain batch, peer hello, lab seal, etc.).

**`/v1/accounts` DoS**

- Iterates **all** `g.chain.st.accounts` under `inner.read()` (`handlers_account.rs:36–62`).
- For foreign-home accounts, `foreign_home_lookup_state(...).await` runs **inside the loop while read lock is held** — may call `handshake_read_traced` (`common.rs:501`).
- **Needs analysis:** many accounts × slow handshake → extended read lock blocks writers (seal path uses `write()`).

**Open questions**

- Per-route body limits vs global cap?
- Pagination / rate limit on `/v1/accounts`?

---

### 3.4 `relay_import` path

**Observed** (`relay.rs:539–631`)

1. `target_hi_for_import(tx)` derives shard from import body.
2. `select_target` scans HTTP seeds, GET `/v1/status`, checks `network_id` / genesis / `domain_hi` match (`relay.rs:204–279`).
3. `POST {target.base}/v1/tx` with **same** `SignedTx` JSON (`relay.rs:558`).
4. On HTTP success, source records relay flow and `roaming_pool.mark_import_by_export` under write lock — **no local chain seal**.

**What appears safe**

- Target node runs full `v1_tx` handler including provenance prefilter and (for local import) direct seal.
- Relay source treats non-success as `BAD_GATEWAY` / `SERVICE_UNAVAILABLE`.

**Needs deeper analysis**

- **Prefilter asymmetry:** Source shard runs `enforce_import_provenance_prefilter` before relay (`handlers_tx.rs:77–80`); foreign shard runs again — can crafted tx pass source but fail target (or vice versa)?
- **`select_target` race:** Status fetched at T0, POST at T1 — peer may have changed role/domain; wrong-shard delivery?
- **Trust model:** Relay uses configured seeds + status self-report — malicious seed advertisement?

---

### 3.5 `enforce_import_provenance_prefilter` TOCTOU

**Observed** (`tx_policy.rs:74–157`)

Under **read lock** in `v1_tx`:

1. Reject if `st.imported_set.contains(export_id)` (`:98–99`).
2. Validate `import_provenance` vs registry / cross_shard facts.
3. Later, direct-seal path calls `chain.seal` → `apply_tx` → `imported_set.insert`.

**Race window**

- Two concurrent `POST /v1/tx` with same `export_id`: both may observe `imported_set` empty → both pass prefilter → first seal wins, second should fail at `apply_tx` (`DuplicateImport`). Second returns **500 on seal path** (`handlers_tx.rs:147–152`) not CONFLICT prefilter.

**Needs deeper analysis**

- Worker-queue imports: are they always direct-seal? Import uses direct-seal branch (`handlers_tx.rs:101`) — yes.
- Relay + local concurrent import of same export_id on different nodes — cross-node duplicate semantics.
- Is CONFLICT vs 500 distinction exploitable for griefing?

---

### 3.6 `TxBody` deserialization strictness

**Observed**

- `SignedTx`: derive `Deserialize`, **no** `deny_unknown_fields` (`tx.rs:364–388`).
- `TxBody`: custom `Deserialize` via internal `RawTxBody` enum (`tx.rs:220–343`); variant structs lack `deny_unknown_fields` — **unknown fields in a variant object are silently ignored** (serde default).
- Retired `claim_mark` variant maps to explicit error (`tx.rs:339–341`).

**PolicyAction / InitV4Extension:** derived Deserialize without `deny_unknown_fields`.

**Forward-compat hazard:** Clients sending extra fields today are ignored; future code might start reading them — document versioning policy.

**`import_fee` on Transfer/Policy in JSON:** Deserializes into `SignedTx.import_fee`; `validate_tx_shape` does not clear/reject for non-Import; field is **not** in Transfer/Policy signing payload — **signature malleability** relative to envelope.

---

### 3.7 `computed_account_id()` determinism

**Observed** (`tx.rs:394–396`, `hd.rs:9–13`)

```text
account_id = blake3_32(signer_pk[32] || derivation_index.to_le_bytes()[4])
```

- Recomputed on every call; no caching.
- `validate_tx_shape` checks `domain_of_account_id(aid) == tx.domain_code` (`tx.rs:633–634`).
- Account flags (conservation, cosign) read from `aid` bytes, not wire `Init.flags`.

**What appears safe**

- Different `(signer_pk, derivation_index)` → different hash input (blake3).
- Second preimage on account_id would break blake3 collision resistance.

**Needs deeper analysis**

- Can two valid Ed25519 pubkeys + indices collide on account_id? (Cryptographic birthday bound.)
- Mismatch attack: correct sig for `signer_pk` but `domain_code` inconsistent with derived id — caught by `DomainMismatch`.

---

### 3.8 Worker precheck vs seal parity

**Precheck pipeline** (`handlers_tx.rs:269–302`, `worker.rs:339–416`)

1. `validate_tx_shape` (full sig check).
2. Optional **hot path** for simple Transfer (`precheck_hot` / `check_hot_transfer`) — nonce, balance, initialized flags.
3. Else `precheck_full`: `evaluate_policy` + `precheck_apply_with_ctx` (clone state, dry-run `apply_tx_with_ctx` at `tip+1`).

**On success:** HTTP **204 No Content** returned; `ValidatedTx` queued.

**Seal** (`lifecycle.rs:1884–1912`, `chain.rs:188–195`)

- Drains `ValidatedTx` as `SealEntry::PreValidated { at_height: validated_at_height }`.
- If `validated_at_height == tip_before`: `apply_prechecked_tx` (skip policy re-eval + skip sig re-verify).
- Else: `apply_tx_with_ctx` (full path); `inc_stale_validated` metric.

**Seal failure** (`lifecycle.rs:2073–2129`)

- Atomic block apply rolls back; failing tx hash added to `evicted_hashes`.
- **No HTTP callback** to original submitter — client already received 204.

**Invariants precheck enforces**

- Shape + signature + domain + body-specific schema.
- Policy decision Allow (or Init exception).
- Dry-run apply success at next height.

**Open questions / hazards**

- **Ack-without-apply window:** By design async; Fable 5 quantify user/fund impact.
- **Stale precheck:** State changes between precheck and seal (another block sealed, nonce consumed) → apply fails at seal, tx evicted — silent from client POV.
- **Hot path vs full path parity:** Hot path skips `precheck_apply_with_ctx` — when is hot path taken and can it admit txs full path would reject?

---

### Concurrency / parallelism

- **Worker pool:** `validate_tx_shape` + state snapshot reads (`WorkerCtx.reads.snapshot.load()`); validated queue between worker and seal loop.
- **TOCTOU:** Import provenance prefilter vs seal (`read` then later `write`).
- **`/v1/accounts`:** `read()` lock held across `await` in loop — blocks seal `write()` path.
- **Relay:** Concurrent relay + local seal on source/target shards.
- **Test gaps:** No stress tests for duplicate concurrent import prefilter; no harness for post-204 seal eviction notification.

---

## 4. Recommended prompt for Fable 5 agent

---

**Task:** Security analysis of PWM `SignedTx` JSON wire and `/v1/tx` RPC admission at commit `8fbc226`. Focus: deserialization hazards, admission/seal parity, import relay/trust, RPC DoS. **Out of scope:** conservation queue, TUI, ClickHouse.

**Threat model:** Remote HTTP client submitting crafted JSON txs; malicious or misconfigured relay seed; concurrent duplicate import submitters; read-heavy RPC against single node.

**Invariants to prove or refute**

1. No JSON encoding of `u128` fields can panic the node or yield truncated/wrapped amounts reaching `apply_tx`.
2. No `SignedTx` can reach `apply_tx` without equivalent checks to `validate_tx_shape` (modulo documented `validate_shape_no_sig` fast path at seal).
3. Duplicate `export_id` cannot credit twice on one shard.
4. Malleable unsigned envelope fields cannot change economic outcome without invalidating signature.
5. Post-204 eviction cannot create double-spend or permanent fund loss (only liveness/UX harm).

**Files and line ranges**

| File | Lines | Focus |
|------|-------|-------|
| `crates/pwm-core/src/ser_json_u128.rs` | 1–60 | `U128Visitor` coverage |
| `crates/pwmd/src/wire_serde.rs` | 1–72 | Compare `de_u128_compat` |
| `crates/pwm-core/src/tx.rs` | 158–344, 364–713 | `TxBody` deserialize, `SignedTx`, `signing_message`, `validate_tx_shape` |
| `crates/pwm-core/src/hd.rs` | 9–18 | `account_id_from_parts` |
| `crates/pwmd/src/api/handlers_tx.rs` | 47–303 | `/v1/tx` branches, precheck vs direct seal |
| `crates/pwmd/src/api/router.rs` | 31–76 | `DefaultBodyLimit` scope |
| `crates/pwmd/src/api/types.rs` | 16 | `V1_TX_BODY_LIMIT` |
| `crates/pwmd/src/api/handlers_account.rs` | 31–63 | `/v1/accounts` lock scope |
| `crates/pwmd/src/api/common.rs` | 490–520 | `foreign_home_lookup_state` |
| `crates/pwmd/src/pipeline/worker.rs` | 315–417 | `precheck_client`, hot path |
| `crates/pwmd/src/tx_policy.rs` | 74–157 | import provenance prefilter |
| `crates/pwmd/src/relay.rs` | 204–279, 539–631 | `select_target`, `relay_import` |
| `crates/pwm-core/src/chain.rs` | 169–199 | `seal_entries`, PreValidated apply |
| `crates/pwmd/src/lifecycle.rs` | 1884–2130 | seal drain, failure eviction |

**Attack scenarios**

- **A:** JSON fuzz `amount`/`fee`/`import_fee` — negative, float, huge number, 200KB decimal string, scientific notation, hex string (if accepted).
- **B:** Transfer with `init_v4`, `import_fee`, `import_provenance` set — sig validity + apply behavior.
- **C:** Two parallel imports same `export_id` — prefilter vs seal outcome codes.
- **D:** Submit Transfer, receive 204, advance chain state before seal — confirm eviction not double-apply.
- **E:** `relay_import` to wrong shard (topology race) — fund safety on source vs target.
- **F:** Flood `/v1/accounts` while submitting txs — seal latency / lock contention.
- **G:** Policy tx with `target_account` ≠ sender but valid sig for sender — must reject at shape.

**Deliverable:** Ranked findings with proof sketches; separate **consensus-critical** vs **node-liveness/ops** buckets.

---

## 5. Verdict

**Approve with nits** — scope document complete for Fable 5 handoff. Wire/RPC layer has deliberate async admission (204-before-seal), asymmetric u128 deserializers, and import prefilter TOCTOU worth deep analysis; no obvious consensus bypass found in static review of `validate_tx_shape` routing.

### Nits (scope-quality)

1. Align `SignedTx` amount deserialization with `wire_serde::de_u128_compat` or document why not.
2. Reject unsigned envelope fields (`import_fee`, `import_provenance`) on non-matching `TxBody` variants at `validate_tx_shape`.
3. Add bounded-length guard on u128 decimal string parse.
4. Refactor `/v1/accounts` to avoid `read().await` handshake work inside account loop.

---

## 6. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260702-wire-rpc-security-scope.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 35000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260702-wire-rpc-security-scope.md'
git commit -m 'docs(v7-8): wire and RPC security scope for Fable 5 (8fbc226)'
```