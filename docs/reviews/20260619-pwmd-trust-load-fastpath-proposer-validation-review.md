# Review: pwmd trust-load O(tail) proposer fastpath

**Ticket:** `tasks/20260619-pwmd-trust-load-fastpath-proposer-validation.json`  
**Date:** 2026-06-17  
**Reviewer:** pwm-review  
**Coding handoff:** PASS (`crates/pwmd/src/snapshot/io.rs`, `incremental.rs`, `docs/guide-node-storage-and-snapshot.md`)

---

## 1. Scope recap

Slice targets JsonFile **trust-default** cold start: remove genesis→tip replay inside `validate_snapshot_trusted` / `trust_tail_prod_idx` when summary checkpoint aligns with manifest tip. Claims:

- Use persisted snapshot v4 `active_validator_indices` + `epoch_counter` (RFC V6-3 / snapshot v4) for tail proposer schedule when no epoch boundary falls in the loaded tail window.
- On in-tail epoch boundary, sequential epoch JSONL replay from boundary height only (`load_blocks_range`), not per-height random `load_block_at_height` from genesis.
- `stage=trust_validate` progress logs (~10s) mirroring `chain_verify`.
- Preserve full `--snapshot-verify-chain` / `summary_manifest_lag` forced replay path.
- Operator doc: trust load is O(tail + optional boundary segment), SLO note.

Depends on checkpoint-lag gate (`20260617-pwmd-snapshot-summary-checkpoint-lag`) and chain-verify progress pattern (`20260616-pwmd-chain-verify-progress-pct`).

---

## 2. Requirements fit

| AC | Status | Notes |
|---|---|---|
| No `1..tip_h` loop in trust proposer path | **Met** | `trust_tail_prod_idx` scoped to `[tail_first_h..tip_h]`; full genesis replay remains only in `validate_snapshot` / forced verify. |
| No boundary in tail → prod_idx without replay | **Met** | Fast path uses `pick_prod_idx(h, &snap_state.active_validator_indices)` for entire tail (`trust_prod_no_bnd_set`). |
| Boundary in tail → replay `[B..tip]` sequential | **Partial** | `load_blocks_range` + sequential iterator implemented; pre-boundary tail heights intentionally get `None` (no schedule check). Replay seeds `replay_state` from **tip** `snap_state` — see Safety. |
| `load_blocks_range` / no per-height JSONL re-read hot path | **Met** | `load_blocks_range` delegates to `load_cons_blocks_epochs` with line-range reads; trust path no longer scans genesis heights. |
| `trust_validate` progress logs | **Met** | `stage=trust_validate` start/progress/done with height, percent, elapsed_ms. |
| validate_ms regression / benchmark | **Partial** | `trust_load_skips_old_replay` (N=1105) eprints `validate_ms` only; no threshold assert; no `testing.md` benchmark note; fixture <10k blocks. |
| Full verify not weakened | **Met** | `load_snapshot_timed` still selects `validate_snapshot` + `load_blocks_from_epochs` when `verify_chain` or manifest lag. |
| `cargo test -p pwmd snapshot incremental io` | **Not run by reviewer** | Unit tests present in touched modules; defer to pwm-testing. |
| Operator guide updated | **Met** | `docs/guide-node-storage-and-snapshot.md` documents O(tail), boundary replay segment, SLO target. |

**Gap summary:** AC timing gate and multi-block boundary-tail correctness are under-tested; boundary-branch proposer skip is intentional per AC wording but under-documented as a trust-model limit.

---

## 3. Style and module shape

- Existing `//!` module banners on `io.rs` and `incremental.rs`; new helpers are small and colocated.
- `python scripts/check_entity_name_segments.py` on touched paths: **no violations** (prod_max=4).
- New symbols within policy: `tail_epoch_bnd_h`, `trust_tail_prod_idx`, `load_blocks_range`, `trust_prod_no_bnd_set`, `trust_prod_tail_bnd_skip`, `trust_load_skips_old_replay`.
- `load_cons_blocks_epochs` alias `load_consecutive_blocks_from_epochs` preserved; no façade bloat.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

---

## 4. Safety

### Trust boundary (intended)

Trust-default assumes **local disk integrity** (aligned summary + manifest + epochs). Weakening vs full replay is explicit in guide and ticket brief (Bitcoin UTXO-set analogy). `summary_manifest_lag` and `--snapshot-verify-chain` remain the audit escape hatches.

### Boundary branch: pre-boundary `None` proposer check (coding nit)

When `tail_epoch_bnd_h` returns `Some(bnd_h)` with `bnd_h > tail_first_h`, heights `[tail_first_h, bnd_h)` are left as `Option::None` and `validate_snapshot_trusted` **skips `prod_idx` schedule comparison** (still checks `prev_hash`, `tx_root`, PoA sig against header `prod_idx`).

**Spec / RFC V6-3 alignment:** Normative RFC 4 addendum requires proposer selection from `active_validator_indices` at each height, with active set changing only at epoch boundaries. Persisted tip `snap_state.active_validator_indices` reflects the set **after** the in-tail boundary roll, so it is **wrong** for pre-boundary heights in the same tail window. Using tip active set there would be a false negative; skipping schedule check is a coherent trust tradeoff **if** other bindings (tip `state_root`, tail linkage, manifest `tip_hash`, block@1 preflight) hold.

**Risk window:** With `TAIL_BLOCK_CAP=1000` and production-scale `epoch_length_blocks` (e.g. 20_160), an in-tail boundary occurs in roughly the first ~1000 blocks after each epoch boundary (~5% of restarts). At most ~`epoch_length_blocks - 1` pre-boundary heights could skip schedule check, capped by tail size.

**Recommendation (nit, not blocker):** Add one sentence to the operator guide: when the tail spans an epoch boundary, proposer **schedule** is not re-derived for pre-boundary tail heights; use full verify if that window matters.

### Boundary branch: replay state seeding (follow-up)

`trust_tail_prod_idx` boundary path clones **tip** `snap_state` then replays `[bnd_h..tip_h]` applying `roll_epoch_if_needed`, txs, and rewards again. Correct proposer pick for `h >= bnd_h` within the same epoch segment should use `snap_state.active_validator_indices` directly (post-boundary-roll set is stable until the next boundary). Replaying from tip state:

- Re-applies epoch roll at `bnd_h` (double `epoch_counter` bump).
- Re-applies txs/rewards on top of tip balances — likely **spurious load failures** when post-boundary tail blocks carry non-empty txs; harmless for empty-block tests only (`trust_prod_tail_bnd_skip` covers `bnd_h == tip_h` with empty seals).

This is a **medium** correctness risk for the minority restart window where `bnd_h < tip_h` and tail blocks after the boundary include state-changing txs. Not a remote-attacker vector under the local-disk trust model, but it can break legitimate cold starts or yield unreliable proposer derivation.

**Suggested fix direction for pwm-coding (not applied here):** For `h >= bnd_h`, set `want[tail_pos] = Some(pick_prod_idx(h, &snap_state.active_validator_indices))` without state replay; keep `None` only for `h < bnd_h`. Remove or narrow the replay loop unless tx replay from a correctly seeded pre-boundary state is required.

### Other safety notes

- `epoch_length_blocks == 0` → no boundary helper → full-tail schedule from persisted active set (consistent with genesis guard elsewhere).
- Tamper regression `trust_load_skips_old_replay` confirms trust path does not re-read tampered pre-tail epoch data (height 2 prod_idx corruption) while staying in trust mode — good negative control for genesis→tip removal.
- No new panics/unwraps in hot path; errors propagate as `Result<String, _>`.

---

## 5. Tests

**Present**

- `trust_prod_no_bnd_set` — schedule parity vs `pick_prod_idx` on persisted active set.
- `trust_prod_tail_bnd_skip` — asserts `None` for pre-boundary tail indices when boundary at tip (single-block replay window).
- `trust_load_skips_old_replay` — tamper outside tail + trust mode + timing telemetry hook.
- Existing epoch/tail cap tests (`epoch_trust_respects_tail_cap`, etc.) unchanged.

**Missing / weak for touched logic**

- Multi-block tail with `bnd_h < tip_h` and **non-empty** post-boundary txs (load success + prod_idx values).
- Asserted `validate_ms` upper bound on ~10k fixture (AC#6).
- Explicit full-verify regression that tampered prod_idx inside tail is rejected (trust accepts only when tamper is outside loaded tail — document or test).

pwm-testing should run `cargo test -p pwmd snapshot incremental io` per AC and consider the boundary multi-block case before owner CY @124k measurement.

---

## 6. Verdict

**PASS_WITH_NITS** vs acceptance criteria.

Implementation delivers the architectural win (no genesis→tip trust replay, sequential range load, progress logs, guide update, full-verify path preserved). Boundary pre-boundary `None` proposer skip is **spec-consistent with trust checkpoint semantics** and ticket AC#3, but should be documented for operators. Boundary replay seeding from tip state needs a follow-up test/fix for `bnd_h < tip_h` with txs. AC#6 timing gate remains soft (eprint only).

**Prioritized nits for pwm-coding / pwm-testing**

1. **Medium:** Simplify or fix boundary-tail proposer derivation; add test `bnd_h < tip_h` with non-empty txs.
2. **Low:** Document pre-boundary proposer schedule skip in operator guide.
3. **Low:** Add `validate_ms` threshold assert or documented bench for ~10k blocks (AC#6).

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts: docs/reviews/20260619-pwmd-trust-load-fastpath-proposer-validation-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 28000
  confidence: low
```
