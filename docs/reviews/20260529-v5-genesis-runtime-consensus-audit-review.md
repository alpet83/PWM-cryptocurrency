# Review: Genesis vs runtime parameters — consensus drift audit

**Date:** 2026-05-29  
**Agent:** pwm-review  
**Ticket:** `20260529-v5-genesis-runtime-consensus-audit-review`  
**Scope:** review-only audit; no product code edits  
**Seed case:** `docs/reviews/20260529-v5-cy-cluster-seal-cadence-review.md`  

---

## 1. Scope recap

This audit traces the seal cadence vs `blocks_per_hour` mismatch and inventories other genesis/runtime parameters that can affect consensus, economics, cluster safety, or operator expectations.

The owner rule used for classification: **network behavior that affects consensus/economic timing should be locked by genesis or code, not silently changed by operator runtime knobs**.

Primary sources reviewed:

- `crates/pwmd/src/main.rs`, `crates/pwmd/src/config.rs`, `crates/pwmd/src/lifecycle.rs`
- `crates/pwm-core/src/genesis.rs`, `crates/pwm-core/src/marks.rs`, `crates/pwm-core/src/chain.rs`, `crates/pwm-core/src/state.rs`, `crates/pwm-core/src/tx.rs`
- `docs/rfc/12-claim-maturity-and-state-model.md`, `docs/rfc/19-float-inflation.md`, `docs/rfc/6-policy-engine.md`, `docs/plans/mvp_v5.md`
- CY launchers: `cy-cluster-proposer.ps1`, `cy-cluster-attester.ps1`, `cy-cluster-common.ps1`
- Related review/coding tickets: seal cadence review, cluster attest suppression review, genesis seal cadence coding, cluster attest timing coding.

---

## 2. Executive summary

**Verdict:** `PASS_WITH_FOLLOWUPS`

The original `2s` seal cadence drift is now addressed in the current working tree by `seal_interval_ms = 3_600_000 / GenCfg.blocks_per_hour`. The follow-up cluster timing coding ticket also derives RFC16 cluster timing from the same genesis cadence. That resolves the immediate V5 CY closeout blocker.

The broader audit found four remaining governance/documentation risks:

1. **Cluster timing CLI flags still exist but are now effectively overridden in cluster mode.** This is safer than runtime tuning, but docs/help should say they are legacy/default inputs, not independent consensus knobs.
2. **`season_enabled` and `season_coeff_ppm` are redundant/ambiguous.** Reward code uses `season_coeff_ppm == 0` fallback semantics, while `GenCfg::season_ppm()` still has a `season_enabled` branch.
3. **Several safety/cadence runtime knobs are silently clamped.** This is operationally safe but can mislead operators unless logged.
4. **Some hard constants are consensus-shaping (`MARKS_CAP`, `PWM_RAW_SCALE`, `MIN_IMPORT_FEE_UNITS`, `DET_SEAL_TS_BASE`) and should remain explicitly documented as code-locked chain spec constants.**

No product code was changed by this audit.

---

## 3. Git archaeology: cadence vs `blocks_per_hour`

| Date | Commit | Evidence | Interpretation |
|---|---|---|---|
| 2026-04-18 | `104dce3` `feat: PoA chain, pwmd REST...` | Introduced PoA chain and `pwmd` baseline; `crates/pwmd/src/lifecycle.rs` did not exist at that path in this commit | No periodic seal cadence mismatch yet; sealing was not yet the later lifecycle loop |
| 2026-05-05 | `dedb965` `feat(v2): RFC pack...` | RFC 0012 v1 specified `hours = floor(delta_seconds / 3600)`; V2 code used block Unix time for maturity | Economic time was wall-clock-hour based, not block-count based |
| 2026-05-13 | `62b89bb` `feat(pwmd): sync/transport/cluster...` | `spawn_seal_loop` introduced `tokio::time::interval(Duration::from_secs(2))` | Actual daemon cadence became 2s/block (≈1800 blocks/hour), still without genesis `blocks_per_hour` |
| 2026-05-23 | `c11840d` `feat(v5-2): extend gencfg...` | `GenCfg.blocks_per_hour`, `DEF_BLOCKS_PER_HOUR=3600` added | The mismatch became normative: genesis default implied 1s/block while runtime still sealed at 2s/block |
| 2026-05-23 | `87af492` `fix(v5-2): align review fixups` | V5 marks timing normalized to height-based `delta_blocks / blocks_per_hour` | The mismatch became economically visible: lazy marks accrued at half expected wall-clock rate under 2s seal |
| 2026-05-28 | V5 audit / closeout reviews | V5 Rust audit focused on safety/behavioral findings but did not flag cadence vs genesis | Review gap: cross-layer runtime cadence was outside local code-audit pattern matching |
| 2026-05-29 | `20260529-v5-cy-cluster-seal-cadence-review` | Owner observed ~20s per 10 blocks; review identified hard-coded 2s seal loop as root cause | First explicit detection of the cross-layer mismatch |
| 2026-05-29 | `20260529-v5-genesis-seal-cadence-align-coding` | Current working tree adds `seal_interval_ms(blocks_per_hour)` and `seal_cadence genesis_blocks_per_hour=...` startup log | Immediate mismatch fixed: seal cadence now follows genesis |
| 2026-05-29 | `20260529-v5-cy-cluster-attest-timing-align-coding` | Current working tree adds `cluster_timing_ms`, `apply_cluster_timing`, and proposer heartbeat cap to seal interval | Secondary cluster timing fallout fixed in code path, pending full CY soak validation |

### When divergence became normative

The divergence became normative at `c11840d` when `blocks_per_hour=3600` entered `GenCfg`, then was cemented by `87af492` when V5 marks converted from seconds to block-height timing. From that point, the economics model assumed 3600 blocks/hour while `pwmd` still produced about 1800 blocks/hour.

### RFC 0012 cross-check

`docs/rfc/12-claim-maturity-and-state-model.md` says:

> `blocks_per_hour` is a deterministic chain-height conversion parameter. It is not a wall-clock oracle.

That wording is internally correct: the formula is replayable and height-based. The missed piece was **runtime calibration**: if a default genesis says 3600 blocks/hour, the reference node should attempt that cadence or clearly document a different deployment profile. The current `seal_interval_ms = 3_600_000 / blocks_per_hour` aligns implementation with the RFC's intended conversion parameter.

---

## 4. Parameter inventory

Risk classes:

- **P0 split-brain / consensus:** changing value across validators can fork state, block acceptance, quorum membership, or rewards.
- **P1 economics / liveness drift:** affects rewards, marks timing, cluster liveness, failover, or security posture; may not immediately fork if local-only.
- **P2 ops-only:** observability, diagnostics, resource caps, or local UX.

| Parameter / group | Source(s) | Mutability | Risk | Duplicate / drift note | Recommendation |
|---|---|---|---|---|---|
| `blocks_per_hour` | `GenCfg`, `DEF_BLOCKS_PER_HOUR=3600` | Genesis immutable | P0/P1 | Drives marks timing and now seal cadence | **Lock in genesis**; current fix correct |
| `seal_interval_ms` | Code-derived from `blocks_per_hour` in `lifecycle.rs` | Code formula | P0/P1 | Previously hard-coded 2s; now derived | **Keep code-derived**; no CLI override |
| `cluster_timing_ms` (`tx_catchup_ms`, `attest_timeout_ms`) | Current code derives from seal interval when cluster enabled | Code-derived runtime normalization | P1 | CLI flags still exist but current `apply_cluster_timing` overwrites cluster timings | **Document/rename as legacy/default inputs** or remove independent CLI semantics |
| `cluster_members`, `quorum_k`, `quorum_n`, `node_instance_id` | CLI/config/wire | Runtime operator input | P0 | Static members must match stable node instance IDs; default instance ID is pid/time if omitted | **Require stable `--node-instance-id` when cluster enabled** |
| `cluster_role` | CLI/env | Runtime operator input | P0/P1 | `attester` implies no local sealing; role mismatch drops frames | Keep runtime, but document consensus/liveness impact |
| `seal_lease_backend`, lease TTL/takeover | CLI/env/config | Runtime operator input | P0/P1 | `process-local` disables multi-process split-brain protection; TTL/takeover silently clamped | Keep for labs; **warn on clamping** and restrict process-local in production profiles |
| `deployment_profile`, `seal_role` | CLI/env | Runtime operator input | P0/P1 | `multi_sealer_experimental` relaxes safety; active override rejected for attester | Keep explicit and noisy; no default relaxation |
| `marks_per_hour` | `GenCfg` / JSON name `marks_per_coin_per_hour` | Genesis immutable | P0/P1 | Main lazy marks rate | Lock in genesis; validate arithmetic bounds |
| `MARKS_CAP` | Code constant `u32::MAX` | Code-locked | P0 | RFC 0012 says not a genesis/runtime knob | Keep hard const; document fork-only change |
| `PWM_RAW_SCALE` | Code constant `1_000_000` | Code-locked | P0 | Raw unit conversion used by balances, stake and marks | Keep hard const |
| `base_emission_per_block` | `GenCfg` | Genesis immutable | P0/P1 | Used by V5 float reward formula | Lock in genesis |
| `season_coeff_ppm` | `GenCfg` | Genesis immutable | P1 | Reward code uses zero as fallback-to-legacy; docs require explicit non-zero for production | Lock in genesis; validate/document zero fallback |
| `season_enabled` | `GenCfg` | Genesis immutable | P1 | Redundant with `season_coeff_ppm`; `GenCfg::season_ppm()` still has boolean branch | **Follow-up cleanup/spec decision** |
| `block_reward` | `GenCfg` legacy fallback | Genesis immutable | P0/P1 | Used for legacy policy and reward fallback when coefficient is zero | Keep as legacy/fallback; docs should call it `legacy_block_reward` consistently |
| `policy_ver` | `GenCfg`, `LEGACY_POLICY_VER=1` | Genesis immutable | P0 | Gates legacy vs V5 reward path | Prefer enum/newtype in future; document values |
| `pwm_stake_min`, `marks_stake_min` | `GenCfg` defaults | Genesis immutable | P1 | Stake gates depend on policy/reward paths; defaults via sparse JSON | Lock in genesis; document policy-version dependency |
| `ipv4_claim_phases` | `GenCfg` | Genesis immutable | P0/P1 | Registry address/allocation controls on-chain claim acceptance | Lock in genesis; duplicate phase reject already addressed |
| `ActivationMode::Deferred.activate_at_height` | Transaction policy state | On-chain tx state | P0 | Height-based policy activation; deterministic | Keep height-based only |
| Policy kinds / bitset | Code enum + account state | Code/on-chain state | P0 | Runtime does not load script policies | Keep code enum for V5 |
| `MIN_IMPORT_FEE_UNITS` | Code const in `tx.rs` | Code-locked | P1 | Fee policy is protocol-visible but not genesis-controlled | Decide whether chain-spec const is acceptable; document fork-only |
| `TAIL_BLOCK_CAP` | Code const in `chain.rs` | Code-locked | P1/P2 | Retention only; canonical height remains authoritative | Ops-only enough; keep code const |
| `DET_SEAL_TS_BASE` | Code const in `chain.rs` | Code-locked dev/test | P1/P2 | Affects deterministic test hashes, not production wall-clock | Keep dev-only; document no production use |
| Snapshot backend / verify / keepalive | CLI/env | Runtime operator input | P1/P2 | Can affect startup/degraded readiness; should not change consensus if snapshot validates | Keep runtime; log degraded state loudly |
| Runtime identity (`network_id`, `domain_hi`, `cluster_id`, `node_id`) | CLI/env | Runtime operator input | P0/P1 | Peer trust, snapshot namespace, handoff provenance depend on identity | Keep explicit; reject ambiguous cluster configs |
| Transport heartbeat/retry/runaway | CLI/config | Runtime operator input; some values now adjusted for cluster proposer | P1/P2 | Heartbeat interval previously outran cluster seal cadence; current code caps proposer heartbeat when cluster enabled | Keep ops knobs but document cluster overrides |
| Operator log override (`/v1/operator/log/override`, `PWM_ADMIN_TOKEN`) | Runtime RPC/env | Runtime mutable | P2 | Observability only; no consensus effect found | Keep ops-only |
| Debug flags (`debug-disable-seal-loop`, deterministic seal time, align mid, dump divergence) | CLI/env | Runtime debug | P1/P2 | Can stop sealing or alter timestamps in dev/test | Keep with strong test/dev wording and warnings |

---

## 5. Duplicate and drift findings

### D1 — `blocks_per_hour` vs runtime seal cadence

- **Status:** fixed in current working tree.
- **Before:** `GenCfg.blocks_per_hour=3600`, `spawn_seal_loop=2s`.
- **Now:** `seal_interval_ms(blocks_per_hour)` and startup log `seal_cadence genesis_blocks_per_hour=N seal_interval_ms=M`.
- **Residual:** needs full CY soak evidence after related timing fixes.

### D2 — Cluster timing CLI flags vs genesis/code timing

- **Status:** partially fixed in current working tree.
- `ClusterCfg` and CLI still expose `cluster_tx_catchup_ms` and `cluster_attest_timeout_ms`.
- `apply_cluster_timing` now overwrites those values under `cluster.enabled` from the genesis-derived seal interval.
- This is safe for owner rule, but confusing: operators may think CLI values still set consensus timing.

Recommendation: update help/docs or remove independent semantics. If flags remain, label them as legacy/default non-authoritative under genesis-timed cluster mode.

### D3 — `cluster_tx_catchup_ms` meaning

Earlier review found it configured/logged but not meaningfully used in the cluster path. Current code derives it in `cluster_timing_ms`, but the main observable gate still uses `attest_timeout_ms`. If `tx_catchup_ms` remains in public config, it needs a concrete protocol use or should be documented as future/reserved.

### D4 — `season_enabled` vs `season_coeff_ppm`

`compute_block_reward()` uses:

```rust
if season_coeff_ppm == 0 { block_reward } else { base_emission_per_block * season_coeff_ppm / 1_000_000 }
```

`GenCfg::season_ppm()` still uses `season_enabled` to choose stored coefficient vs neutral coefficient. This creates two ways to express neutral/disabled seasonality.

Recommendation: create a small follow-up to either remove `season_enabled` from active V5 semantics or document it as legacy/reserved and make reward code/RFC wording single-source.

### D5 — Silent runtime clamping

`main.rs` clamps several runtime values with `.max(...)`:

- `seal_lease_ttl_ms.max(1000)`
- `seal_takeover_timeout_ms.max(1000)`
- `debug_dump_cap.max(1)`
- `debug_dump_trigger_streak.max(2)`
- transport heartbeat minimums are enforced downstream/defaulted.

Recommendation: warn when operator input was clamped. This is not a consensus blocker, but it avoids false operator assumptions.

### D6 — Sparse genesis defaults

`GenCfg` serde defaults fill important economics fields when absent:

- `blocks_per_hour`
- `marks_per_hour`
- `policy_ver`
- `base_emission_per_block`
- `pwm_stake_min`
- `marks_stake_min`
- `season_coeff_ppm`

Defaults are convenient for dev migration, but production genesis should be explicit. Recommendation: add a production-genesis validation/report mode that warns if consensus/economic defaults were implied rather than explicit.

---

## 6. Recommendations and follow-up ticket IDs

No product edits were made in this ticket. Suggested follow-up IDs only:

| Ticket ID | Priority | Scope | Recommendation |
|---|---|---|---|
| `20260529-v5-cy-cluster-attest-timing-align-coding` | P0/P1 | Already done in `tasks/done` | Keep; requires CY soak validation |
| `20260529-v5-runtime-param-doc-lock-review` | P1 | docs/runbooks + CLI help review | Clarify which CLI flags are authoritative vs overridden by genesis/code timing |
| `20260529-v5-cluster-stable-node-id-guard-coding` | P0/P1 | `main.rs`, cluster config validation | Require explicit stable `--node-instance-id` when `cluster.enabled && cluster_members` |
| `20260529-v5-genesis-explicit-prod-params-coding` | P1 | genesis loader / reporting | Warn or fail in production profile when important economics fields are filled by serde defaults |
| `20260529-v5-season-param-unification-review` | P1 | RFC 19 + `GenCfg` reward semantics | Decide whether `season_enabled` remains active, legacy, or removed in a future schema |
| `20260529-v5-runtime-clamp-warning-coding` | P2 | `main.rs` startup logs | Log when CLI/env timing/resource values are clamped |

---

## 7. Verification performed

Commands / checks used during this audit:

```text
git log --oneline --decorate --all -- crates/pwmd/src/lifecycle.rs crates/pwm-core/src/genesis.rs docs/rfc/12-claim-maturity-and-state-model.md
git log -1 --format='%h %cI %s' 104dce3 62b89bb dedb965 c11840d 87af492 3d988de
git show --stat --oneline 104dce3 dedb965 c11840d
git grep -n "from_secs(2)\|blocks_per_hour\|DEF_BLOCKS_PER_HOUR\|delta_seconds\|3600" -- crates docs/rfc docs/plans
```

Additional static reads/greps covered:

- `crates/pwmd/src/main.rs` CLI/env surface
- `crates/pwmd/src/config.rs` defaults/validation
- `crates/pwmd/src/lifecycle.rs` seal/cluster timing
- `crates/pwm-core/src/genesis.rs` genesis fields/defaults
- `crates/pwm-core/src/marks.rs` marks and reward formulas
- `crates/pwm-core/src/chain.rs` reward path and deterministic timestamp mode
- `docs/rfc/12-claim-maturity-and-state-model.md`
- `docs/rfc/19-float-inflation.md`
- `docs/rfc/6-policy-engine.md`

No live cluster was started for this broad audit.

---

## 8. Verdict

**PASS_WITH_FOLLOWUPS** — the specific seal cadence drift is fixed in the current code path, and the related cluster timing drift has a completed coding ticket. The broader parameter audit did not find another immediate V5 closeout blocker, but it found several governance/documentation follow-ups that should be tracked before production-hardening.

Most important closeout condition: run the pending CY soak/testing tickets against the current working tree and confirm that genesis-derived seal cadence plus cluster-derived timing produce stable head growth without sustained `missing_round_state` / `attestations_missing` storms.

---

## 9. Participation / token estimate

```yaml
agent: pwm-review
result: PASS_WITH_FOLLOWUPS
artifacts: docs/reviews/20260529-v5-genesis-runtime-consensus-audit-review.md
token_usage:
  source: estimate
  input: 42000
  output: 6500
  total: 48500
  confidence: medium
```

---

## 10. Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'P:\opt\docker\pwm-protocol'
git add 'docs/reviews/20260529-v5-genesis-runtime-consensus-audit-review.md'
git add 'tasks/done/20260529-v5-genesis-runtime-consensus-audit-review.json'
git commit -m 'docs(v5): audit genesis runtime consensus parameters'
```
