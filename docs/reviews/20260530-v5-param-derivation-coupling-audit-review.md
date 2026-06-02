# Review: V5 parameter derivation coupling audit

**Date:** 2026-05-30  
**Agent:** pwm-review  
**Ticket:** `20260530-v5-param-derivation-coupling-audit-review`  
**Scope:** review-only hygiene audit; no product code edits  
**Prior audits:** `docs/reviews/20260529-v5-genesis-runtime-consensus-audit-review.md`, `docs/reviews/20260529-v5-cy-cluster-suppression-r2-review.md`  

---

## 1. Scope recap

This is the second-pass maintainability audit after the genesis/runtime consensus drift audit. The goal is not to find a new immediate contradiction, but to map where timing/economics semantics are smeared across genesis fields, CLI/config defaults, code literals, and transport ordering.

Reviewed current tree after these coding slices:

- `20260529-v5-cy-cluster-propose-seal-align-coding`
- `20260530-v5-seal-drift-correction-orphan-params-coding`

Primary files:

- `crates/pwmd/src/lifecycle.rs`
- `crates/pwmd/src/config.rs`
- `crates/pwmd/src/main.rs`
- `crates/pwmd/src/transport.rs`
- `crates/pwmd/src/transport/peer_session/mod.rs`
- `crates/pwmd/src/transport/peer_session/seed/session/steady_session.rs`
- `crates/pwm-core/src/genesis.rs`
- `crates/pwm-core/src/marks.rs`
- `crates/pwm-core/src/chain.rs`

---

## 2. Executive summary

**Verdict:** `PASS_WITH_NITS`

The current tree is materially cleaner than the state captured by the 2026-05-29 audits:

- `--cluster-tx-catchup-ms` and `--cluster-attest-timeout-ms` are removed from the `pwmd` CLI.
- `ClusterCfg.tx_catchup_ms` is gone.
- `cluster_timing_ms` now returns only derived `attest_timeout_ms`.
- Cluster proposer heartbeat is capped to the genesis-derived seal interval.
- `spawn_seal_loop` opens local round state before `run_cluster_gate` via `record_cluster_prop_tick`.
- `steady_session` sends `ClusterPropose` immediately after heartbeat and before sync/cross-shard/account outbound work.
- Seal drift correction is named and bounded (`SEAL_DRIFT_WINDOW_BLOCKS=100`, `SEAL_DRIFT_STEP_PPM=10_000`).

No closeout blocker was found in this hygiene pass. Remaining items are cleanup/documentation nits: centralize/rename magic literals, document which constants are chain-spec vs scheduler-local, and resolve the `season_enabled` / `season_coeff_ppm` dual semantics.

**Verdict line:** `PASS_WITH_NITS — parameter derivation is now mostly single-sourced; residual smear is documentation/constant consolidation, not a V5 closeout blocker.`

---

## 3. Coupling graph

```text
GenCfg.blocks_per_hour
  ├─ lifecycle::seal_interval_ms(bph)
  │    ├─ spawn_seal_loop nominal interval
  │    ├─ seal_cadence startup log
  │    ├─ SEAL_DRIFT_WINDOW_BLOCKS / SEAL_DRIFT_STEP_PPM local scheduler correction
  │    ├─ cluster_timing_ms(seal_ms) -> ClusterCfg.attest_timeout_ms
  │    └─ cluster_prop_ms(seal_ms, heartbeat_ms) -> TransportConfig.heartbeat_interval_ms cap for proposer
  └─ pwm-core::marks::compute_lazy_marks delta_blocks / blocks_per_hour

TransportConfig.heartbeat_interval_ms
  ├─ steady_session sleep cadence
  ├─ send_cluster_prop cadence (after heartbeat, before sync)
  ├─ sync_live retry/progress cadence
  ├─ account freshness windows
  └─ sticky session window max(heartbeat_timeout*2, heartbeat_interval*4, 500)

GenCfg economy fields
  ├─ marks_per_hour -> compute_lazy_marks rate
  ├─ base_emission_per_block + season_coeff_ppm -> compute_block_reward
  ├─ season_enabled -> GenCfg::season_ppm only (residual dual path)
  ├─ block_reward -> legacy/fallback reward
  ├─ pwm_stake_min / marks_stake_min -> stake/reward gates
  └─ ipv4_claim_phases -> ClaimIPv4Batch registry/allocation acceptance
```

---

## 4. Primary knob table

| Primary knob | Derived values | Consumers | Current status | Recommendation |
|---|---|---|---|---|
| `GenCfg.blocks_per_hour` | `seal_interval_ms`, marks `delta_hours`, derived cluster timeout, proposer heartbeat cap | `lifecycle.rs`, `marks.rs`, runbook/operator logs | Good: now single source for seal cadence | Keep genesis-locked; add one doc table saying it controls both economics and reference-node cadence |
| `seal_interval_ms` | nominal seal sleep; expected drift window; cluster timeout; proposer heartbeat cap | `spawn_seal_loop`, `cluster_timing_ms`, `cluster_prop_ms` | Good: pure function, unit-tested | Keep in `lifecycle.rs`; consider moving timing constants into a small `seal_timing` submodule if this grows |
| `ClusterCfg.attest_timeout_ms` | Derived from `seal_ms` using `cluster_timing_ms` | `run_cluster_gate` quorum timeout | Good: no longer CLI-controlled | Keep internal; rename comment to “derived at startup” and avoid config docs implying operator authority |
| `TransportConfig.heartbeat_interval_ms` | Base peer heartbeat, capped for cluster proposer | `steady_session`, connect scheduling, sync live, freshness windows | Acceptable smear: ops default exists, cluster proposer derivation mutates it | Document cap in CLI help/runbook; consider storing both `operator_heartbeat_ms` and `effective_heartbeat_ms` if confusion persists |
| `SEAL_DRIFT_*` | scheduler correction only | local active sealer | Good but new | Document as local scheduler, not consensus fork parameter |
| `GenCfg.season_coeff_ppm` | block reward multiplier | `compute_block_reward` | Good as genesis field | Make it the single active season knob |
| `GenCfg.season_enabled` | `GenCfg::season_ppm` only | no direct reward path in `compute_block_reward` | Residual semantic duplication | Follow-up review/coding to deprecate, remove, or document as legacy/reserved |
| `MIN_IMPORT_FEE_UNITS` | import fee minimum | tx validation/state apply | Code-locked protocol constant | Document as chain-spec constant; do not hide behind runtime config |
| `MARKS_CAP`, `PWM_RAW_SCALE` | marks saturation, raw balance/stake unit | core state/economics | Good hard constants | Document fork-only changes in economics spec/glossary |

---

## 5. Magic literals inventory

### Replace with formula or named const — already mostly done

| Literal | Location | Current meaning | Status | Recommendation |
|---|---|---|---|---|
| `3_600_000` | `lifecycle.rs:36` | milliseconds per nominal hour for `blocks_per_hour` conversion | Named `SEAL_HOUR_MS` | Keep; maybe rename to `MS_PER_HOUR` for clarity |
| `500` | `lifecycle.rs:37`, `cluster_timing_ms` | cluster attest slack beyond one seal tick | Named `CLUSTER_ATTEST_JITTER_MS` | Keep, but document why 500ms is enough for CY lab/Windows scheduler |
| `100` | `lifecycle.rs:38` | drift correction sample window | Named `SEAL_DRIFT_WINDOW_BLOCKS` | Keep |
| `10_000` | `lifecycle.rs:39` | 1% drift adjustment step in ppm | Named `SEAL_DRIFT_STEP_PPM` | Keep |
| `1_000_000` | `lifecycle.rs:40`; core ppm/raw scale contexts | ppm denominator or raw scale depending module | Named locally as `PPM_DENOM`; `PWM_RAW_SCALE` in core | Keep separate; avoid mixing ppm and raw-unit semantics in docs |
| `1500`, `4500`, `500` | `main.rs`/`config.rs` transport defaults | ops transport heartbeat/timeout/retry defaults | Still CLI/config defaults | Acceptable P2, but document cluster proposer cap and defaults origin |
| `500` | `peer_session/mod.rs:756`, API freshness helper | minimum sticky/freshness window | Literal remains | Name as a transport freshness floor if touched again |
| `200` | `steady_session.rs:26`, sync/connect paths | minimum heartbeat interval | Literal remains | Name as `MIN_HEARTBEAT_INTERVAL_MS` in transport config/module |

### Document as chain-spec hard const

| Constant | Location | Why not a runtime knob |
|---|---|---|
| `DEF_BLOCKS_PER_HOUR=3600` | `pwm-core/src/genesis.rs` | default genesis cadence; production genesis should be explicit |
| `DEF_MARKS_HOUR=1` | `pwm-core/src/genesis.rs` | default marks economics; production genesis should be explicit |
| `MARKS_CAP=u32::MAX` | `pwm-core/src/marks.rs` | state model/saturation bound |
| `PWM_RAW_SCALE=1_000_000` | `pwm-core/src/display.rs` | token unit scale |
| `MIN_IMPORT_FEE_UNITS=10_000` | `pwm-core/src/tx.rs` | protocol-visible fee minimum |
| `DET_SEAL_TS_BASE=1_700_000_000` | `pwm-core/src/chain.rs` | test/dev deterministic timestamp base only |
| `TAIL_BLOCK_CAP=1000` | `pwm-core/src/chain.rs` | in-memory retention, not consensus height |

---

## 6. Smeared logic findings

### NIT-001: Transport heartbeat has two roles

`TransportConfig.heartbeat_interval_ms` is both an ops-level peer heartbeat default and, in cluster proposer mode, an effective upper bound for `ClusterPropose` cadence. `apply_cluster_timing` correctly caps it, but the name and CLI help still sound purely transport-level.

Impact: operator confusion, not current consensus drift.

Recommendation: document as “base heartbeat; cluster proposer effective heartbeat is capped to genesis seal interval.” If more timing work lands, introduce `effective_heartbeat_interval_ms` in startup logs/config state so the original CLI value and effective cluster value are visibly distinct.

### NIT-002: `ClusterCfg.attest_timeout_ms` still has a default even though it is derived in cluster mode

`ClusterCfg::default().attest_timeout_ms = 1000` remains as an initial value and for tests/non-cluster construction. In runtime cluster mode it is overwritten by `apply_cluster_timing`.

Impact: low; no CLI control remains.

Recommendation: add a comment near the default saying “placeholder before `apply_cluster_timing`; not operator-authoritative in cluster mode.”

### NIT-003: `season_enabled` / `season_coeff_ppm` remains the most important semantic duplicate

This audit confirms the earlier finding: `compute_block_reward()` directly uses `season_coeff_ppm == 0` fallback behavior, while `GenCfg::season_ppm()` still encodes a `season_enabled` branch. That is a real semantics smear, though not directly connected to CY timing.

Recommendation: follow-up `20260530-v5-season-param-unification-review` to decide whether `season_enabled` is legacy/reserved or should be wired into reward computation. Do not leave two “season disabled” encodings undocumented.

### NIT-004: Wall-clock drift correction is local scheduler state; docs should prevent consensus misunderstanding

The new drift correction changes local effective sleep, not block header validity or genesis state. Because it has `seal_cadence_drift` logs and affects observed wall-clock rate, it should be explicitly described as **scheduler correction only**.

Recommendation: keep current bounded approach; add docs wording that consensus/economics still derive from height and genesis `blocks_per_hour`, not measured wall clock.

---

## 7. Post-coding verification

Verified current tree includes the post R2 and orphan-param fixes:

- `record_cluster_prop_tick` is called before `run_cluster_gate` in `spawn_seal_loop` (`crates/pwmd/src/lifecycle.rs:561`).
- `steady_session` sends `send_cluster_prop` immediately after heartbeat and before cross-shard/account/sync outbound work (`crates/pwmd/src/transport/peer_session/seed/session/steady_session.rs:60`).
- `--cluster-tx-catchup-ms` and `--cluster-attest-timeout-ms` are absent from `crates/pwmd/src/main.rs`.
- `tx_catchup_ms` is absent from `crates/pwmd/src` and the CY runbook.
- `cluster_timing_ms` derives only `attest_timeout_ms` from seal cadence.
- `seal_cadence_drift` is logged every 100 local seals when correction triggers.

---

## 8. Verification performed

Commands run:

```text
python scripts/check_entity_name_segments.py crates/pwmd/src/lifecycle.rs crates/pwmd/src/config.rs crates/pwmd/src/main.rs crates/pwmd/src/transport.rs crates/pwmd/src/transport/peer_session/mod.rs crates/pwmd/src/transport/peer_session/seed/session/steady_session.rs
cargo test -p pwmd --lib lifecycle
cargo test -p pwmd --lib config
cargo test -p pwmd --lib peer_session
cargo check -p pwmd -p pwm-core
rg -n "tx_catchup_ms|cluster-tx-catchup-ms|cluster-attest-timeout-ms" crates/pwmd/src docs/runbooks/v5-cy-cluster-precloseout-soak.md
```

Results:

- Naming policy: PASS, no violations.
- `cargo test -p pwmd --lib lifecycle`: PASS, 19 tests.
- `cargo test -p pwmd --lib config`: PASS, 15 tests.
- `cargo test -p pwmd --lib peer_session`: PASS, 38 tests.
- `cargo check -p pwmd -p pwm-core`: PASS.
- Removed cluster timing knobs grep: PASS, no matches.

No live CY soak was run by this review.

---

## 9. Follow-up ticket IDs only

| Ticket ID | Priority | Purpose |
|---|---|---|
| `20260530-v5-param-const-consolidation-coding` | P2 | Name remaining transport freshness/min heartbeat literals and clarify placeholder defaults |
| `20260530-v5-season-param-unification-review` | P1 | Resolve `season_enabled` vs `season_coeff_ppm` active semantics |
| `20260530-v5-runtime-param-doc-lock-review` | P2 | Add one operator-facing table: genesis/code-locked vs runtime ops knobs |
| `20260530-v5-production-genesis-explicitness-coding` | P1 | Warn/fail when production genesis relies on serde defaults for economic fields |

---

## 10. Verdict

**PASS_WITH_NITS** — the present parameter derivation graph is acceptable for V5 CY closeout hygiene. The most dangerous default-era timing smears (`2s` seal, cluster timeout CLI, tx catchup ghost field, propose-after-sync ordering) are gone or fixed. Remaining issues are mostly naming/docs/semantic cleanup.

**Verdict line:** `PASS_WITH_NITS — no closeout blocker; track constant consolidation and season parameter unification.`

---

## 11. Participation / token estimate

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts: docs/reviews/20260530-v5-param-derivation-coupling-audit-review.md
token_usage:
  source: estimate
  input: 30000
  output: 5200
  total: 35200
  confidence: medium
```

---

## 12. Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'P:\opt\docker\PWM-cryptocurrency'
git add 'docs/reviews/20260530-v5-param-derivation-coupling-audit-review.md'
git add 'tasks/done/20260530-v5-param-derivation-coupling-audit-review.json'
git commit -m 'docs(v5): audit parameter derivation coupling'
```
