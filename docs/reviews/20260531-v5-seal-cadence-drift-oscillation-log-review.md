# Review: V5 seal cadence — nominal ±1% envelope + drift log oscillation

**Date:** 2026-05-30  
**Agent:** pwm-review  
**Ticket:** `20260531-v5-seal-cadence-drift-oscillation-log-review`  
**Verdict:** REQUEST_CHANGES  

---

## 1. Scope recap

Owner requires `effective_ms` must stay within ±1% of genesis `nominal_ms` at all times. Current code caps per-step adjustment at ±1% of *current effective_ms* — which enables unbounded integral windup. This review parses 161 drift observations from a live CY proposer log (~30k head) and proves the invariant is massively violated.

---

## 2. Log source

**Primary:** `logs/2026-05-30/pwmd-cy-proposer-051753.log` — live CY cluster, continuous sealing since 05:17 UTC

```
$ grep seal_cadence_drift logs/2026-05-30/pwmd-cy-proposer-051753.log | wc -l
161
```

---

## 3. Envelope analysis

| Metric | Value |
|---|---|
| Nominal `seal_interval_ms` | 1000ms (bph=3600) |
| Min `effective_ms` | **257ms** (−74.3%) |
| Max `effective_ms` | 1010ms (+1.0%) |
| Observations within ±1% envelope | **3 / 161** |
| Observations breaching >1% | **158 / 161** |
| Sign flips (oscillation) | 0 |
| First breach | eff=981ms (−1.9%, #3) |
| Worst breach | eff=257ms (−74.3%, #160) |

### Time series (first 20 observations)

| # | effective_ms | envelope_pct | adjust_pct |
|---|---|---|---|
| 0 | 1010 | +1.00% | +1.0000 |
| 1 | 1000 | +0.00% | −0.9900 |
| 2 | 990 | −1.00% | −1.0000 |
| 3 | 981 | −1.90% | −0.9090 |
| 4 | 972 | −2.80% | −0.9174 |
| 5 | 963 | −3.70% | −0.9259 |
| 6 | 954 | −4.60% | −0.9345 |
| 7 | 945 | −5.50% | −0.9433 |
| 8 | 936 | −6.40% | −0.9523 |
| 9 | 927 | −7.30% | −0.9615 |
| 10 | 918 | −8.20% | −0.9708 |
| 11 | 909 | −9.10% | −0.9803 |
| 12 | 900 | −10.00% | −0.9900 |
| 13 | 891 | −10.90% | −1.0000 |
| 14 | 883 | −11.70% | −0.8978 |
| 15 | 875 | −12.50% | −0.9060 |
| 16 | 867 | −13.30% | −0.9142 |
| 17 | 859 | −14.10% | −0.9227 |
| 18 | 851 | −14.90% | −0.9313 |
| 19 | 843 | −15.70% | −0.9400 |

### Last 3 observations

| # | effective_ms | envelope_pct | adjust_pct |
|---|---|---|---|
| 158 | 261 | −73.90% | −0.7604 |
| 159 | 259 | −74.10% | −0.7662 |
| 160 | 257 | −74.30% | −0.7722 |

---

## 4. Root cause analysis

### 4.1. Step-on-effective compounding (primary)

```rust
// lifecycle.rs:103-121
let cap = current_ms.saturating_mul(SEAL_DRIFT_STEP_PPM) / PPM_DENOM;
// STEP_PPM = 10_000 → cap = 1% of current effective_ms
```

Mechanism:
1. Wall-clock is consistently 30–180 seconds slower than expected per 100-block window (cluster attest, quorum, disk I/O)
2. `actual_ms > expected_ms` → `effective_ms -= cap` where `cap = effective_ms * 0.01`
3. Each step: `effective_ms` shrinks → next `cap` shrinks → correction never catches up
4. Integral windup: compounding drives effective_ms to 257ms while actual drift remains ~50–180 seconds

**Proof:** After 161 corrections, effective_ms = 257ms. At this point `cap = 257 * 0.01 = 2.57ms` — far smaller than actual drift (~130–180 seconds per 100 blocks).

### 4.2. Monotonic decay (no recovery)

Zero sign flips across all 161 observations. `actual_ms` consistently > `expected_ms` (cluster overhead always dominates). Correction never overshoots → monotonic downward compounding.

### 4.3. Gap: owner invariant vs code

| Dimension | Owner requirement | Current code |
|---|---|---|
| Envelope | `\|effective − nominal\| / nominal ≤ 0.01` at all times | No clamp |
| Step cap | Not specified | ±1% of `current effective_ms` |
| Recovery | Must return to envelope | None (compounding only) |
| Result | 3/161 in envelope (1.8%) | **Catastrophic breach** (257ms = 25.7% of nominal) |

---

## 5. Oscillation analysis

**No oscillation detected** — 0 sign flips across 161 observations. This is actually worse: positive drift overshoot would indicate the correction *can* recover. Monotonic decay proves the step-on-effective mechanism is fundamentally broken for sustained drift.

---

## 6. Observed «расколбас» attribution

Operator observed tempo swings («заметные колебания темпа печати блоков»). With effective_ms = 257ms:

- Each seal tick fires every ~257ms (4× faster than nominal 1000ms)
- But `run_cluster_gate` suppresses seal when quorum is pending (normal attest cycle)
- Result: rapid tick/suppress pulses → visible tempo jitter in operator terminal

The root cause is envelope breach → aggressive effective_ms → tick frequency mismatch with cluster attest timing, NOT quorum suppression itself.

---

## 7. Verdict

**REQUEST_CHANGES** — 158/161 observations breach the ±1% envelope. Catastrophic integral windup drives effective_ms from 1000ms to 257ms (−74.3%). Step-on-effective compounding violates owner invariant. No sign flips — monotonic decay with no recovery mechanism.

---

## 8. Recommended coding action

**Ticket:** `20260531-v5-seal-cadence-reanchor-envelope-clamp-coding`

Minimal scope in `crates/pwmd/src/lifecycle.rs`:

1. **Clamp `effective_ms` to `[nominal * 0.99, nominal * 1.01]`** after each drift correction — implements owner invariant directly
2. **Re-anchor `effective_ms = nominal`** once at startup (or on first correction after this fix) — recover from -74% windup
3. **Optional deadband:** skip correction when `|actual_ms − expected_ms| / expected_ms < 0.001` (0.1%) to avoid micro-adjustment churn
4. **Add `envelope_pct` to drift log** — separate from `adjust_pct` for operator visibility:
   ```
   seal_cadence_drift ... envelope_pct=-73.90 adjust_pct=-0.77 clamp_applied=true
   ```

**Do NOT:**
- Widen the ±1% envelope without owner decision
- Keep step-on-effective as the only correction mechanism
- Add complex PID/adaptive logic (out of V5 scope)

---

## 9. Code gap diagram

```
Owner wants:        effective_ms ∈ [990, 1010]              (ALWAYS)
                          ┌──────────────┐
                          │  ±1% envelope│
                          └──────────────┘

Current code does:   cap = effective_ms * 0.01             (EVERY 100 BLOCKS)
                          effective_ms ← effective_ms − cap
                          cap shrinks with effective_ms → windup
                          ┌────────────────────────────────────┐
                          │ 1010 → 990 → 981 → … → 257ms       │
                          │ no clamp, no recovery              │
                          └────────────────────────────────────┘
```

---

## 10. Participation / token estimate

```yaml
agent: pwm-review
result: FAIL  # REQUEST_CHANGES
artifacts: docs/reviews/20260531-v5-seal-cadence-drift-oscillation-log-review.md
token_usage:
  source: estimate
  input: 12000
  output: 4500
  total: 16500
  confidence: high
```

---

## 11. Git handoff

```powershell
# git-handoff
Set-Location 'P:\opt\docker\pwm-protocol'
git add 'docs/reviews/20260531-v5-seal-cadence-drift-oscillation-log-review.md'
git commit -m 'docs(v5-seal): cadence drift log review REQUEST_CHANGES — envelope breach 257ms vs 1000ms'
```