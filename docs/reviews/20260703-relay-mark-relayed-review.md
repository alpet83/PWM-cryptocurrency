# Review: relay roaming state transition fix (b1772aa)

- **date:** 2026-07-03
- **ticket:** `20260703-relay-mark-relayed-review`
- **coding_ticket:** `20260703-relay-mark-relayed`
- **commit:** `b1772aa920374bd2de64a7fa7ddb38f42601ef3d`
- **agent:** `pwm-review` (`pwm_review`)
- **scope:** `relay.rs` `relay_import`, `roaming.rs` `mark_relayed_by_export`, `state.rs` `merge_cross_shard_facts`

---

## 1. Scope recap

`relay_import` previously called `mark_import_by_export` after peer HTTP 204, treating remote acceptance as final import on the source shard. Fix: record **Relayed** (in-flight) on 204; promote to **Imported** only when trusted cross-shard facts report `Imported` status.

---

## 2. Focus-area verification

| # | Focus | Verdict | Evidence |
|---|-------|---------|----------|
| 1 | `relay_import` no longer `mark_import_by_export` on bare 204 | **PASS** | After success check (`relay.rs:583–604`), post-delivery block calls `mark_relayed_by_export(export_key)` at `:631`. `mark_import_by_export` absent from `relay.rs`. |
| 2 | Relayed / in-flight state recorded | **PASS** | `IntentStatus::Relayed` enum variant (`roaming.rs:70`). `mark_relayed_by_export` → `set_status(..., Relayed, ...)` (`:278–281`). Flow trace `roaming_status:relayed` pushed (`relay.rs:634–641`). |
| 3 | Cross-shard fact path: relayed → imported | **PASS** | `Inner::merge_cross_shard_facts` (`state.rs:217–232`): when `fact.status == CrossShardStatus::Imported`, calls `roaming_pool.mark_import_by_export(export_id)`. Invoked from trusted peer paths only (`peer_session/mod.rs:483–496`, `trusted` gate). Local direct-seal import still marks imported in `handlers_tx.rs:206` (same-shard, correct). |
| 4 | No permanent stuck state if facts never arrive | **PASS** with nit | `Relayed` is locking (`roaming.rs:92–93`). `expire_by_height` (`:302–323`) transitions non-terminal intents past `expires_at_height` to `Expired` and clears `active_locks` via `set_status`. `/v1/status` exposes `stuck_relayed_without_import` counter (`handlers_status.rs:69–70`, `:86`). **Nit:** no automatic `Failed` on relay HTTP success + missing facts — relies on height TTL. |

---

## 3. State machine trace

```text
Export sealed (source)     → Queued → Exported (mark_exported)
Handoff / relay path       → Relayed (mark_relayed / mark_relayed_by_export)
Peer 204 on relay_import   → Relayed (mark_relayed_by_export)   [was: Imported — fixed]
Trusted cross-shard fact   → Imported (merge_cross_shard_facts → mark_import_by_export)
TTL exceeded               → Expired (expire_by_height on seal / handlers)
```

**Security improvement:** Source shard no longer trusts peer 204 as cryptographic proof of import settlement.

### Wire JSON / u128

Wire JSON / u128: not applicable (roaming state machine only; no wire field changes in this slice).

---

## 4. Safety

| Risk | Assessment |
|------|------------|
| False imported on malicious 204 | **Mitigated** — 204 → Relayed only. |
| Intent never completes | **Bounded** — height TTL → `Expired`, lock released. |
| Untrusted facts promoting import | **Mitigated** — `merge_cross_shard_facts` returns early when `!trusted`. |
| Snapshot durability after relay mark | **Handled** — `relay_import` saves tip summary with rollback on failure (`relay.rs:642–667`). |

---

## 5. Tests

No new unit/integration test asserting `relay_import` sets `Relayed` not `Imported`. Coding ticket ran `cargo check` / `clippy`. `issues-report.md` notes follow-up test need.

**Gap (nit):** HTTP/roaming test covering relay_import → `IntentStatus::Relayed` and fact merge → `Imported`.

---

## 6. Concurrency / parallelism

`relay_import` takes `write()` lock for roaming mark + optional snapshot (`relay.rs:629–648`). Short critical section; no lock held across HTTP POST to peer (POST completes before write). **No new deadlock hazards.**

---

## 7. BLOCKERs

None.

---

## 8. Nits (non-blocking)

1. **NIT-1:** Add test: mock peer 204 → assert intent `relayed`; inject trusted `CrossShardFact` Imported → assert `imported`.
2. **NIT-2:** `mark_relayed_by_export` is silent no-op when `export_id` has no `export_to_intent` mapping — log at debug if observability needed.
3. **NIT-3:** Document operator expectation: `stuck_relayed_without_import` may be non-zero until facts propagate or TTL expires.

---

## 9. Verdict

**Approve** — `relay_import` correctly defers imported finality to trusted cross-shard facts; Relayed state and expiry path prevent indefinite locking. Matches coding ticket acceptance criteria.

---

## 10. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260703-relay-mark-relayed-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 14000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260703-relay-mark-relayed-review.md'
git commit -m 'docs(v7): relay mark-relayed roaming state fix review (b1772aa)'
```