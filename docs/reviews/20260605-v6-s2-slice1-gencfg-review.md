# Review: V6-2 slice 1 — GenCfg V6 parameters

**Ticket:** `tasks/20260605-v6-s2-slice1-gencfg.json`  
**Worktree:** `P:/opt/docker/pwm-protocol-worktrees/v6-sprint2-core-model` (branch `v6/20260605-v6-sprint2-core-model`)  
**Reviewer:** pwm-review  
**Date:** 2026-06-05  
**Specs:** `docs/rfc/addenda/v6-rfc4-validators-stake-admission.md`, `docs/rfc/addenda/v6-rfc9-mode-b-escrow.md`, `docs/adr/0009-address-flags-runtime-enforcement.md`

## 1. Scope recap

Slice 1 adds four V6 consensus/cross-shard parameters to `GenCfg` in `pwm-core` (types, serde defaults, unit tests), plus compile-only struct-literal updates wherever `GenCfg { … }` is constructed. Explicit non-goals respected: no `active_validator_indices`, no snapshot v4 schema bump, no `validate_tx_shape` / apply / enforcement paths.

**Claimed touchpoints (uncommitted worktree diff):**

| File | Role |
|------|------|
| `crates/pwm-core/src/genesis.rs` | New fields, `DEF_*` constants, default fns, extended tests |
| `crates/pwm-cli/src/cmd_account.rs`, `cmd_genesis.rs` | Fill new struct fields in test/dev genesis builders |
| `crates/pwm-tui/src/marks_display.rs` | Same (marks preview cfg) |
| `crates/pwmd/src/snapshot/genesis.rs` | Same when mapping v4 genesis envelope → `GenCfg` |
| `crates/pwm-tui/tests/wallet_roaming.rs` | rustfmt-only (unrelated) |

**MVP / checklist alignment:** `docs/plans/mvp_v6.md` §GenCfg parameter list; RFC4 §3 (`min_validator_stake`, `epoch_length_blocks`); RFC9 §3 (`cross_shard_lock_timeout_blocks`); ADR 0009 (`conservation_delay_blocks`, default 86400).

## 2. Requirements fit

| Acceptance criterion | Status | Notes |
|---------------------|--------|-------|
| `min_validator_stake: u128` + `ser_json_u128` | **Met** | `#[serde(default = "default_min_val_stake", with = "ser_json_u128")]`; default aliases `DEF_PWM_STAKE_MIN` (100_000). |
| `epoch_length_blocks: u64`, MUST be > 0 | **Mostly met** | Default `DEF_EPOCH_LEN_BLOCKS = 10_080`. Sparse-json test asserts `> 0` on default path only; **no** reject of `0` on deserialize (see nits). |
| `conservation_delay_blocks: u64`, default 86400 | **Met** | Matches ADR 0009 / mvp_v6 normative default. |
| `cross_shard_lock_timeout_blocks: u64`, default 604800 | **Met** | Rust field `xshard_lock_to_blocks` with `serde(rename = "cross_shard_lock_timeout_blocks")`; default `DEF_XSHARD_LOCK_TO = 604_800`. |
| Sensible devnet defaults; legacy fixtures via `#[serde(default)]` | **Met** | `gen_cfg_defaults_sparse_json` covers omitted keys; `dev_net()` updated. |
| JSON round-trip; u128 as decimal strings | **Met** | `gen_cfg_json_round_trip` asserts wire keys and string encoding; new `cfg_min_val_decimal` for `u128::MAX`. |
| `cargo check --workspace` | **Met** | Verified locally (pre-existing pwmd dead-code warnings only). |
| No enforcement / snapshot version bump | **Met** | No apply/seal/mempool changes; `GenesisFileV4` / `GenesisV4CfgOut` envelopes unchanged; no snapshot format version change. |

**Spec naming:** JSON wire name `cross_shard_lock_timeout_blocks` matches RFC9 §3; internal Rust name intentionally abbreviated (orchestrator focus item — acceptable with rename).

**Gap (informational, out of slice scope):** genesis-build JSON envelope (`GenesisV4CfgOut`) and pwmd `GenesisCfgV4` still omit V6 params; values are injected as compile-time defaults when building `GenCfg` (same pre-existing pattern as `base_emission_per_block`). Operators cannot override V6 params via on-disk genesis v5 envelope until a follow-up slice extends the envelope — not required by slice-1 ticket.

## 3. Style and module shape

- **`scripts/check_entity_name_segments.py`** on all touched production paths: **zero violations** (`prod_max: 4`, `test_max: 5`).
- New production identifiers respect ≤4-word policy: `min_validator_stake`, `epoch_length_blocks`, `conservation_delay_blocks`, `xshard_lock_to_blocks`, `default_min_val_stake`, etc.
- Module banner `//! Genesis rows + dev factory.` already present; English doc comments consistent with crate.
- Compile-fix churn in CLI/TUI/pwmd is proportional (four field lines per literal); no new large blobs in façade files.
- Minor scope noise: `wallet_roaming.rs` rustfmt-only hunk — harmless, not slice-functional.

### Wire JSON / u128

**Scope:** `GenCfg` genesis JSON (chain config loaded at node startup), not `PeerWireMsg` / peer sync catch-up wire.

- **`u128` fields:** `min_validator_stake` uses explicit `with = "ser_json_u128"` (decimal string). Tests confirm round-trip string encoding.
- **New `u64` fields:** serde default integers — JSON-safe.
- **RFC wire names:** serialize uses `cross_shard_lock_timeout_blocks` via `serde(rename)`; round-trip test asserts that key.
- **Peer wire / PolicyAction:** not touched — **not applicable** for peer decode stall class.

## 4. Safety

- Additive config fields only; no consensus apply paths, panics, or trust-boundary changes.
- Defaults are positive durations/thresholds appropriate for devnet profiles.
- No new unchecked `unwrap` in production paths in this diff.
- Hardcoded defaults in pwmd genesis parse mean misconfigured envelope cannot accidentally set absurd V6 params today — conservative for slice 1, limits operator flexibility until envelope extension (documented above).

## 5. Tests

**Present:**

- Extended `gen_cfg_json_round_trip` (all four fields, RFC JSON key for lock timeout).
- Extended `gen_cfg_defaults_sparse_json` (defaults + `epoch_length_blocks > 0`).
- New `cfg_min_val_decimal` (`u128::MAX` string decode).

**Gaps (non-blocking for slice 1):**

- No test that deserializing `"epoch_length_blocks": 0` is rejected (acceptance allows validate **or** doc test; only default-path assert exists).
- No dedicated test that JSON key `cross_shard_lock_timeout_blocks` alone (without Rust field name) round-trips — covered implicitly by `gen_cfg_json_round_trip` serialize side; deserialize-from-RFC-key-only could be added later.
- `pwm-core` genesis tests run clean (`gen_cfg_*`, `cfg_min_val_decimal`).

## 6. Verdict

**Approve with nits**

Prioritized nits for follow-up (no blockers for slice 1 merge):

1. **Low — `epoch_length_blocks == 0`:** Consider `GenCfg::validate()` or a negative deserialize test before epoch enforcement lands (RFC4 MUST > 0).
2. **Low — naming traceability:** Document in a brief `///` on `xshard_lock_to_blocks` that JSON/RFC name is `cross_shard_lock_timeout_blocks` (rename already correct on wire).
3. **Low — genesis envelope:** Track a future slice to expose V6 params in `GenesisV4CfgOut` / `GenesisCfgV4` so operators can override defaults without recompiling.
4. **Cosmetic —** drop or isolate rustfmt-only `wallet_roaming.rs` hunk if slice commits should stay strictly GenCfg-scoped.

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260605-v6-s2-slice1-gencfg-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 8500
  confidence: low
```

**One-line verdict:** **APPROVE_WITH_NITS** — GenCfg V6 fields, defaults, serde, and u128 JSON encoding match slice-1 spec; minor follow-ups on zero-epoch validation and genesis envelope exposure only.
