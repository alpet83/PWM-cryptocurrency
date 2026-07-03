# V7-S2 Slice 3 fix — dual admission removed (pwm-review)

- date: 2026-06-26
- ticket: `20260626-v7-s2-slice3-fix-dual-admission-review`
- commit: `28d05fe` (branch `mvp-v7`)
- normative: `docs/plans/mvp_v7s2.md` § Slice 3
- prior review: `docs/reviews/20260626-v7-s2-slice3-validated-queue-review.md` (FAIL — dual admission)

## 1. Scope recap

Fix for Slice 3 blocker: remove `tx_ingress` enqueue after worker precheck so HTTP non-roaming txs enter the seal path only via `validated_rx`.

| file | change |
|------|--------|
| `api/handlers_tx.rs` | non-roaming `_ =>` branch: `run_worker_precheck` only; log `accepted: queued via worker`; no `tx_ingress.try_send`, no `push_tx_flow` |
| `tests/http_status.rs` | `v1_tx_xfer_worker_once` — ingress empty, validated queue exactly one tx |
| `tests/http_export.rs` | `v1_tx_accepts_signed_init` — same ingress/validated assertions |

## 2. Requirements fit

| Acceptance criterion | Verdict | Evidence |
|---------------------|---------|----------|
| `tx_ingress.try_send` removed from non-roaming branch | **PASS** | `handlers_tx.rs:220-224` — only `run_worker_precheck`, tip read, `info!`, `Ok(NO_CONTENT)`; repo-wide grep shows no `tx_ingress.sender` / `try_send` producers |
| Log `accepted: queued via worker` replaces old `push_tx_flow` | **PASS** | `handlers_tx.rs:223` — `info!(tx_id = %tx_id_hex(&tx), h = h, "accepted: queued via worker")` |
| Tests: no duplicate seal-batch source for HTTP tx | **PASS** | `v1_tx_xfer_worker_once` (`http_status.rs:748-825`): ingress `try_recv` empty; validated queue single entry, second `try_recv` empty. `v1_tx_accepts_signed_init` (`http_export.rs:49-58`) same for Init |
| `cargo test -p pwmd -p pwm-core` PASS | **UNVERIFIED** | Shell unavailable in review session |

**Prior blocker closed:** worker-accepted HTTP txs no longer appear on both `validated_rx` (→ `SealEntry::PreValidated`) and `tx_ingress` (→ `g.pool` → `SealEntry::Raw`) in the same seal tick.

## 3. Style and module shape

- Non-roaming handler branch reduced to 4 lines after precheck — clear single admission path ✓
- Test names `v1_tx_xfer_worker_once` ≤5 segments (test budget) ✓
- Roaming direct-seal branch still uses `push_tx_flow` — unchanged ✓

Entity segment check not run (shell unavailable).

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

1. **Single admission path** — HTTP success implies worker `ValidatedTx` on channel; lifecycle drains `_validated_rx` first (`lifecycle.rs:1819-1828`). No duplicate nonce in seal batch from HTTP.

2. **Rejected txs** — `v1_tx_underfunded_xfer_mempool` still asserts ingress empty on 422 (`http_status.rs:714-715`); worker rejects before any enqueue.

3. **Residual: worker `try_send` silent drop** — `worker.rs:295` still ignores validated channel full while HTTP returns 204. Pre-existing; not introduced by this fix. Risk: silent accept with no validated entry (not duplicate).

4. **`tx_ingress` drain still in lifecycle** — no current HTTP producer; harmless no-op drain until another ingress source is wired.

## 5. Tests

| Test | Covers |
|------|--------|
| `v1_tx_xfer_worker_once` | funded transfer 204; ingress empty; validated exactly once |
| `v1_tx_accepts_signed_init` | Init 204; ingress empty; validated exactly once |
| `v1_tx_underfunded_xfer_mempool` | reject path; ingress empty |
| `v1_stat_snap_tx_nl` | concurrent status+tx smoke (unchanged; still 204) |

**Gap (non-blocking):** no test runs lifecycle `seal_entries` and asserts block tx count == 1 per HTTP submit (admission-layer assertions suffice for this fix ticket).

## 6. Concurrency / parallelism

**Components:** HTTP awaits worker oneshot, worker OS-thread `validated_tx` send, lifecycle `try_lock` on `_validated_rx` under seal write lock.

| Hazard | Assessment |
|--------|------------|
| Dual-channel duplicate | **Closed** — ingress no longer fed by HTTP non-roaming |
| HTTP holds `inner` read lock only for tip height log | **OK** — brief read after precheck |
| Validated + ingress race | **N/A** for HTTP path post-fix |

**Test gap:** seal-loop integration under concurrent HTTP load (deferred to Slice 4 / soak).

## 7. Findings (prioritized)

### Blocker

None — dual admission blocker from prior review is closed.

### Medium

1. **Worker validated channel full → silent drop** — HTTP 204 without validated entry if `try_send` fails. Recommend propagating backpressure or returning 507 (Slice 4 / follow-up).

### Low

2. **`push_tx_flow` removed from worker path** — less in-memory tx flow trace; `info!` log compensates per acceptance criterion.

3. **Lifecycle `tx_ingress` drain** — dead for current HTTP-only producers; keep until peer/helper ingress lands.

4. **No seal-loop integration test** — admission tests prove single-queue source; end-to-end block composition untested here.

## 8. Verdict

**Approve with nits** — fix is minimal, correct, and well-tested at the admission boundary. Prior Slice 3 blocker (duplicate PreValidated+Raw in seal batch) is resolved.

## 9. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260626-v7-s2-slice3-fix-dual-admission-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 28000, "confidence": "medium" }`