# Review: V6-11 pwmd `--lib` gate (`d251fb5`)

**Ticket:** `tasks/20260615-v6-sprint11-pwmd-lib-gate.json`  
**Commits:** `d251fb5` (coding fix), `1b7ff57` (ticket traceability)  
**Reviewer:** pwm-review  
**Date:** 2026-06-15

## 1. Scope recap

V6-11 blocker: 16 failures in `cargo test -p pwmd --lib` blocking sprint closeout. Ticket cites snapshot/genesis `state_root` replay drift vs current sealing, `snapshot_roaming` balance fixtures, transport height drift, lifecycle cluster timeouts, and slice20 binary-path harness issues.

Acceptance: `pwmd --lib` 0 failed; `pwm-core --lib` and `cargo fmt --check` still PASS. pwm-coding claims 455/0 after fix; pwm-testing retest PASS on alternate `CARGO_TARGET_DIR`.

## 2. Requirements fit

**Production replay alignment (core goal):** `snapshot/io.rs` (`preflight_blk1`, `validate_snapshot`, `trust_tail_prod_idx`) and `transport/peer_session/sync_live.rs` (`apply_blk`) now mirror `pwm-core::Chain::seal` ordering:

- `refund_exp_locks` before and after tx application
- `drain_conservation_at_height` before producer reward
- v2 reward via `compute_block_reward` + `reward_producer_v2(..., 1_000_000)` instead of stale `block_reward` / `season_ppm` / `accrue_marks_v2`
- legacy path drops erroneous `accrue_marks` (consistent with `seal_no_accrue_marks` in pwm-core)

`preflight_blk1` additionally seeds `recompute_active_idxs` + `roll_epoch_if_needed` before block-1 replay — appropriate for genesis-anchor validation from `state0()`.

**Test / harness fixes:** `snapshot_roaming` adds `settle_conserv_delay` and splits multi-tx seals across conservation windows — reflects real `conservation_delay_blocks` semantics. Lifecycle tests fix `SEAL_POLL_INTERVAL_MS` literals, cluster quorum fixtures, and attest timing. slice20 adds `CARGO_TARGET_DIR` resolution, `ensure_cli_bins_ready`, genesis validator funding, wallet index `0`, and longer poll timeouts.

**Gap:** `peer_session/mod.rs` unit tests `blk_fetch_apply_ok` and `cup_missing_range_ok` no longer assert successful sync apply (see §5).

## 3. Style and module shape

Touched files pass `check_entity_name_segments.py` (prod ≤4, test ≤5). No new oversized façade blobs. English comments on new helpers (`settle_conserv_delay`, `fund_genesis_validator`, `ensure_cli_bins_ready`) are adequate.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

**Reward / replay semantics:** Aligning pwmd replay with `Chain::seal` **reduces** consensus drift risk — previously snapshot validation and peer block apply could accept/reject blocks inconsistently with the canonical sealer. No new panics or trust-boundary changes in production paths.

**Consensus drift:** The hardcoded `1_000_000` season ppm in replay matches `Chain::seal` today; if season logic later becomes height/timestamp-dependent, replay sites must move in lockstep (already true for sealing).

**slice20 harness:** `fund_genesis_validator` zeroes stake mins for e2e permissiveness — test-only JSON mutation, not production genesis.

## 5. Tests

**Well covered after fix:**

- `snapshot_roaming` conservation-delay roundtrips
- `sync_live` batch tests (`apply_blk_batch` over 95–105 blocks, manifest checkpoints) — directly exercises updated `apply_blk`
- `same_shard_follower_tcp_tip` (unchanged) — integration convergence `tip_h` + `tip_hash`
- `blk_bad_reject_safe` still asserts `sync_apply_fail_total` and `tip_h == 0` on tampered `state_root`

**Concern — weakened handler unit tests (medium):**

| Test | Before `d251fb5` | After `d251fb5` |
|------|------------------|-----------------|
| `blk_fetch_apply_ok` | `sync_apply_ok_total == 1`, `tip_h == 1`, matching hash | `tip_h == 0`, `tip_hash != blk_hash` (no apply) |
| `cup_missing_range_ok` | `sync_cup_*_total` counters, `tip_h == 256` | `tip_h == 0` only |

Test names still imply success (`*_ok`). This contradicts documented contract in `docs/GLOSSARY.md` and prior slice reviews (e.g. v2-8 slice3 testing notes). Likely root cause: direct `on_tip` / `on_hdr_batch` / `on_blk_batch` harness no longer populates `wait_blk` / cup state as before — **not** a signal that `apply_blk` itself is broken (batch + TCP tests still pass). Nevertheless, inverting assertions masks handler-level regressions that the unit tests were designed to catch.

**Recommendation (nit, non-blocking):** Follow-up slice to restore positive apply assertions (fix peer sync state seeding + `trusted_peers` setup) or rename tests to `*_no_apply_when_unprimed` and add a sibling test that asserts apply success.

## 6. Verdict

**Approve with nits.** Production replay alignment is correct and materially improves safety; roaming/lifecycle/slice20 harness changes are legitimate. Transport `mod.rs` unit assertions were weakened against established contract — integration coverage partially mitigates, but handler-level unit regression guard is lost.

### Prioritized nits

1. **Medium:** Restore `blk_fetch_apply_ok` / `cup_missing_range_ok` positive apply assertions (or rename + document).
2. **Low:** Add comment in `apply_blk` / replay sites that `1_000_000` season ppm is tied to `Chain::seal` until dynamic season lands.

## 7. Participation / token estimate

```json
{
  "agent": "pwm-review",
  "result": "PASS_WITH_NITS",
  "artifacts": "docs/reviews/20260615-v6-sprint11-pwmd-lib-gate-review.md",
  "token_usage": {
    "source": "estimate",
    "input": null,
    "output": null,
    "total": 45000,
    "confidence": "low"
  }
}
```
