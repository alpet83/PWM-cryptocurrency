# Review: V2-8 Slice 3 — header-first live sync + block apply baseline

**Ticket:** `tasks/20260508-v2-sprint8-slice3-header-block-sync.json`  
**Code baseline:** commit `c316e7309fb76393cd2b28f16d0d3ce09a4137e1` (`feat(pwmd): add header-first live sync with block fetch/apply baseline`)  
**Normative anchor:** `docs/rfc/15-same-shard-sync-v1.md` (Slices 0–3, esp. §6.2 live messages, §7 fork-choice, §8 limits, §10 observability)  
**Testing artifact:** `docs/reviews/20260508-v2-8-slice3-testing.md` — при проверке через индекс git отсутствовал как отслеживаемый файл; локально обнаружена неиндексированная копия с результатами по `c316e73` (cargo check, фильтрованные тесты pwmd, naming script). Для полной трассируемости стоит **добавить файл в репозиторий** отдельным коммитом оркестратора.

---

## 1. Scope recap

Slice 3 RFC acceptance (§11): live chain sync — tip signaling, header-first alignment, bounded block fetch; fork-choice v1 tuple applied consistently; convergence under bounded lag on same-shard network.

Observed deliverables in `c316e73`: new `sync_live` module (tip announce send/receive, headers request/response, blocks request/response, bounded inflight and pending queues, rollback-safe batch apply), `SyncPeerState` / `SyncLiveState` in `handshake_state.rs`, transport counters for sync tip/headers/blocks/apply/fork conflicts, integration of live sync into inbound and seed steady loops, wire coverage for `SyncTipAnnounce`, unit/async tests for header break, successful apply, failed apply with tip unchanged, shard mismatch drop.

Explicitly **not** implemented here (aligned with Slice 4): epoch catch-up messages and mode transitions — appropriate scope boundary.

---

## 2. Requirements fit

**Strengths**

- **Header-first then bodies:** tip triggers header request from `local_h + 1`; header batch checked for continuity against local tip `prev_hash` chain; hashes queued for block fetch; responses validated against expected queue — matches intended live-sync shape.
- **Apply safety:** `apply_blk_batch` snapshots chain state and blocks, restores on first failure — satisfies rollback-safe import for this baseline.
- **Validity aligned with §7.2 path:** `apply_blk` enforces next height, `prev_hash`, expected proposer index, signature, tx root, state transition and `state_root` — consistent with “validity before preference” mindset.
- **Gating:** Same-shard + `full_v1` via existing `route_sync_stub`; legacy / profile mismatch drops counted — consistent with Slice 2 negotiation.
- **Anti-DoS baseline:** Local caps (`SYNC_HDR_REQ_CAP`, `SYNC_BLK_REQ_CAP`, inflight caps, pending cap, peer map cap) sit below RFC §8 maxima; malformed limits get `SyncNack` on serving paths where implemented.

**Gaps / partial coverage**

- **Fork-choice tuple (§7.3):** Handler matches only `head_height` / `head_hash` on `SyncTipAnnounce`; **`finalized_height` is not read or compared**. Outbound `send_sync_tip` sets `finalized_height` equal to `head_h`, which does not exercise tuple semantics and may misrepresent the wire field if real finalized lag is introduced later. There is **no cross-peer aggregation** that picks a winning tip among competing announces using `(finalized_height, head_height, head_hash)`; each session advances independently when following one peer’s chain. For a minimal single-peer linear sync baseline this may be acceptable, but it is **not** a full literal match to “fork-choice v1 tuple applied consistently” across competing peers.
- **Silent stalls:** Some paths return success without visible nack (e.g. header batch when expected start height mismatch after clearing inflight counter — logged fork metric in one case; `exp_h != local_h + 1` returns quietly). Worth monitoring operationally; not necessarily wrong for v1 baseline.

---

## 3. Style and module shape

- New `sync_live.rs` carries a focused `//!` banner; logic is concentrated in one module rather than inflating `mod.rs` unchecked — reasonable for this slice.
- **`python scripts/check_rust_fn_name_segments.py crates/pwmd/src/transport`:** no violations (prod ≤4 segments, tests ≤5).

---

## 4. Safety

- Trust boundaries respected at stub level: shard mismatch drops; sync applies only after cryptographic/state checks on blocks.
- Panic surface not audited exhaustively; heavy use of `map_err` / early returns in apply path — consistent with surrounding transport style.
- Queue caps reduce unbounded memory growth from adversarial header spam; dropping oldest pending under cap is a deterministic trade-off (possible starvation under abuse — acceptable for stated baseline).

---

## 5. Tests

- Present: wire decode for tip announce; `hdr_batch_break_drop`; `blk_fetch_apply_ok`; `blk_bad_reject_safe`; `sync_shard_drop_noop`; legacy profile drop for headers.
- Missing vs ideal: multi-peer conflicting tips, tuple ordering, partial header batches spanning tip reorg, and integration test proving lag convergence under multiple peers — understandable deferral but leaves RFC §7.3 compliance partly narrative-only.

---

## 6. Verdict

**approve with nits** — baseline matches Slice 3 functional intent (tip → headers → blocks → safe apply) without Slice 4 catch-up creep; main nit is incomplete **§7.3 tuple** handling and unused **`finalized_height`** on ingest. Отдельный nit: отчёт pwm-testing есть локально, но пока не закоммичен — слабее трассируемость в истории git до добавления файла.

---

## 7. Participation / token estimate

```
agent: pwm-review
result: PASS_WITH_NITS
artifacts: docs/reviews/20260508-v2-8-slice3-review.md
token_usage: { "source": "estimate", "input": null, "output": null, "total": 12000, "confidence": "low" }
```

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260508-v2-8-slice3-review.md'
git add 'tasks/20260508-v2-sprint8-slice3-header-block-sync.json'
git commit -m 'docs(v2-8): slice3 header-first sync review + ticket close'
```
