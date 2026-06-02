# Review: V5 marks mechanics vs TUI + proposer log correlation

**Date:** 2026-05-30  
**Agent:** orchestrator (mechanics supplement to `20260530-v5-tui-marks-operator-journey-review`)  
**Trigger:** Owner reports stake workaround shows no marks change; asks whether proposer logs show autoclaiming.

---

## 1. Executive summary

| Question | Answer |
|----------|--------|
| Is there autoclaiming in proposer logs? | **No.** Seals do not accrue marks; only account-mutating txs **touch** marks. Logs never print `marks`/`stored_marks` on commit. |
| Is stake broken on-chain? | **No** for the observed CY session — two `kind=stake` commits landed; balance/nonce deltas look consistent. |
| Why does TUI show no change after `S`? | **Expected for short waits:** lazy marks need `floor((head - marks_last_block) / blocks_per_hour) >= 1` with **whole PWM staked** (`staked_pwm_raw / 1_000_000 >= 1`). Default `blocks_per_hour = 3600` → **~3600 blocks (~1 nominal hour)** before **+1** effective mark per 1 PWM staked. CY seal cadence ~15 s / 10 blocks ⇒ **~90 wall-clock minutes** to first +1. |
| Why “touch should change marks” feels wrong? | **Touch materializes** lazy delta into `stored_marks` and resets `marks_last_block` to inclusion height. On **first stake from zero**, touch runs **before** `staked_pwm_raw` increases → **zero matured hours at old stake** (test `stake_autoclaim_zero_matured`). After touch, **stored** stays flat until another hour of blocks **or** another touch tx. |
| TUI bug vs doc gap? | **Both:** stale “Claim” copy (queued coding ticket) **and** detail pane shows **`stored_marks` only**, while the Marks **column** uses **effective** lazy marks — operator can watch the wrong field. |

**Verdict:** Mechanics match RFC 0012 / V5 lazy model; **observability and copy** fail the operator. Not a consensus/autoclaim defect.

---

## 2. Proposer log evidence (CY, 2026-05-30)

Source: operator terminal capture around heights **26350–26620**.

### 2.1 Stake commits (no marks in delta)

```
[10:49:55.910] #INFO: tx commit delta: kind=stake ... bal:4900000000000->4899000000000 nonce:3->4
[10:50:22.185] #INFO: tx commit delta: kind=stake ... bal:4899000000000->4800000000000 nonce:4->5
```

`lifecycle.rs` / `handlers_tx.rs` log format is fixed:

```text
tx commit delta: kind={} tx_id={} sender={} bal:{}->{} nonce:{}->{}
```

There is **no** marks/staked field in this line — absence of “autoclaim” lines is **by design**, not evidence that marks were skipped silently.

### 2.2 Seal path (no per-block mark accrual)

Between stakes, logs are only `sealed height=…`, `cluster_gate_pending_summary`, `seal_lease_renewed`, periodic `autosnapshot` / `seal_cadence_drift`. No `accrue_marks`, `touch`, or `marks` keywords.

Chain tests document V5: **`seal` does not call `accrue_marks`** (`chain.rs::seal_no_accrue_marks`). Legacy `accrue_marks` remains only in **snapshot replay / HTTP import** paths, not live seal.

### 2.3 Timing vs first visible mark

| Event | Approx height | Δ blocks from 1st stake (~26440) |
|-------|---------------|-----------------------------------|
| 1st stake | ~26430–26440 | 0 |
| 2nd stake | ~26440 | 0 (touch resets cursor again) |
| Log end | ~26620 | **~180** |

`delta_hours = 180 / 3600 = 0` → **`compute_lazy_marks` returns `stored_marks` unchanged** (no effective bump).

At observed cadence (~1.5 s/block), **3600 blocks ≈ 90 minutes** before the Marks column can show **+1** per 1 PWM staked (with `marks_per_hour = 1`).

---

## 3. On-chain mechanics (normative)

### 3.1 Lazy generation

`compute_lazy_marks` (`crates/pwm-core/src/marks.rs`):

- Requires `current_height > marks_last_block`.
- `delta_hours = (current_height - marks_last_block) / blocks_per_hour`.
- `whole_pwm_staked = staked_pwm_raw / PWM_RAW_SCALE` (integer division — **sub-1 PWM stake ⇒ 0**).
- `generated = whole_pwm_staked * marks_per_hour * min(delta_hours, satur_hours)`.

### 3.2 Touch on Stake / Unstake

```rust
// state.rs — Stake arm (order matters)
touch_acct_mrks(&mut a, inclusion_height, gen_cfg);  // uses stake *before* += amount
a.staked_pwm_raw += amount;
```

`touch_acct_mrks` sets `stored_marks = compute_lazy_marks(...)` then `marks_last_block = inclusion_height`.

Implications:

1. **First stake from zero:** touch sees `whole_pwm_staked == 0` → no new stored marks; cursor jumps to inclusion height.
2. **Second stake soon after:** if `head - marks_last_block < blocks_per_hour`, touch again adds **0** stored marks.
3. **“Autoclaim” in tests** (`stake_autoclaim_zero_matured`) means **zero matured lazy hours at touch time**, not a background worker.

### 3.3 Materialization for burn

`BurnMark` also touches first — burns require **materialized** `stored_marks` after touch. Waiting only in the UI (effective column) is **not** enough for F5 until a touch or enough blocks that the operator performs another mutating tx.

---

## 4. TUI display gaps (why operator sees “nothing”)

| Surface | Field used | Behavior |
|---------|------------|----------|
| Table column **Marks** | `effective_marks_at_height(row, head)` | Lazy projection at poll time — **can** rise while head advances **if** `staked >= 1 PWM` and `delta_hours >= 1`. |
| Detail pane `Marks:` | `r.marks` (**stored** from RPC) | **Stays flat** until touch materializes or user only watches detail. |
| `marks_display.rs` | Hardcoded `DEF_BLOCKS_PER_HOUR` / `DEF_MARKS_HOUR` | Matches default genesis; **no** `/v1/status` genesis params today — risk if devnet genesis diverges. |
| Zero-marks / F5 copy | Still mentions **Claim** / ClaimTx retired | See `20260530-v5-tui-marks-operator-journey-review.md`. |

**Operator trap:** stake → watch detail line or stored-only mental model → see **0** for tens of minutes → conclude broken.

---

## 5. Recommended follow-ups

### 5.1 Already queued (copy/runbook)

`20260530-v5-tui-v5-marks-copy-operator-path-coding` — positive path, remove Claim wording, runbook.

### 5.2 New coding scope (mechanics UX — suggest ticket)

1. **Detail pane:** show `effective_marks` (and optionally `stored` + `marks_last_block` + blocks-to-next-hour).
2. **Zero-marks hint:** if `staked > 0` and `effective == stored`, show “accrual pending: need ~N more blocks (BPH=…)”.
3. **Optional protocol fix (separate slice):** move `touch_acct_mrks` **after** `staked_pwm_raw += amount` on Stake so first stake materializes hours accrued at **zero** stake correctly (spec/RFC alignment review required).
4. **Optional RPC:** expose `blocks_per_hour` / `marks_per_hour` on `/v1/status` for TUI `GenCfg` (avoid hardcoded defaults).

### 5.3 Not recommended

- Adding mark fields to every `tx commit delta` log line (noise) unless `verbosity-focus=wallet` debug slice.
- Reintroducing per-seal `accrue_marks` (contradicts V5 lazy model).

---

## 6. Operator checklist (CY devnet, now)

1. Confirm **Marks column** (not detail `Marks:`) and **Staked** > 0 after `S`.
2. Confirm stake amount ≥ **1.000000 PWM** (whole units).
3. Wait **≥ 3600 blocks** from last stake touch (~90 min at current CY cadence) for first +1 effective mark per 1 PWM staked.
4. Use **F5** only when marks > 0 (materialized path); another **Stake/Unstake/Transfer** touch can materialize early if hours matured.
5. Ignore “autoclaim” in logs — **there is no such event**.

---

## 7. Verdict line

**PASS (mechanics) / REQUEST_CHANGES (TUI observability)** — chain behavior and proposer logs are consistent with V5 lazy marks; operator confusion is time-to-accrue + stored vs effective + stale Claim copy.
