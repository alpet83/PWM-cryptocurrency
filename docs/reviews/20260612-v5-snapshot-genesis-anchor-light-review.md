# Review: 20260612-v5-snapshot-genesis-anchor-light

**Ticket:** `20260612-v5-snapshot-genesis-anchor-light-coding`  
**Date:** 2026-06-12  
**Reviewer:** pwm-review  
**Verdict:** ✅ PASS_WITH_NITS

---

## 1. Scope Recap

ADR 0008 implementation — genesis anchor for Epoch Snapshot:

| Item | Status |
|------|--------|
| `anchor.rs` — `st_root`, `cfg_dig`, `mk_anch`, `chk_anch`, preimage `PWMv0/SNAPGENANCHOR/v1` | ✅ present |
| `types.rs` — `SnapshotGenAnchor`, `SnapshotDataV3` wiring, `anchor_to_v3`/`anchor_from_v3` | ✅ present |
| `io.rs` — fail-closed on trust load, `preflight_blk1`, `attach_anch`, `allow_legacy_env`, `fill_anch` | ✅ present |
| `store.rs` — anchor persist path via `fill_anch` on seal/summary/shutdown | ✅ present |
| `incremental.rs` — `load_block_at_height` (block@1 preflight source) | ✅ pre-existing, wired correctly |
| Tests — `anch_gen_root_mismatch_err`, `preflight_blk1_tamper_tx_root`, `attach_anchor_legacy_with_signer` | ✅ present |
| Docs — `guide-node-storage-and-snapshot.md` § Genesis anchor | ✅ present (partial, see §3) |
| `issues-report.md` entry | ⚠️ absent (see §3, NIT-1) |

MVP checklist: §6 operator / devnet integrity.  
RFC 0020 bootstrap compat requirement: `gencfg_digest` and `genesis_state_root` must be stable and exportable — tracked, see NIT-4.

---

## 2. Requirements Fit

### 2.1 Fail-closed: tampered `state` / block@1 rejected on trust load

**Analysis: PASS.**

`validate_snapshot_trusted` for `tip == 0`:
- Computes `genesis_root = anchor::st_root(cfg)`, compares against `digest(&snapshot.state)` — direct state commitment check BEFORE anchor/migration logic. Tampered state is rejected before any env bypass is applied.
- When anchor present, calls `chk_anch`, which re-derives `genesis_state_root`, `gencfg_digest` from live cfg and verifies signature.

`validate_snapshot_trusted` for `tip > 0`:
- `preflight_blk1` replays txs of block 1 against `cfg.state0()`, compares resulting `digest(state)` with `blk.hdr.state_root`. This cryptographically binds the snapshot's genesis to the current `--genesis-file` (via `state0()` derivation).
- Final state root is checked against `last.hdr.state_root` for the tail tip.
- Block linkage + PoA sigs verified for all tail blocks.
- Tampered block@1 `tx_root` triggers `preflight_blk1` error — confirmed by `preflight_blk1_tamper_tx_root` test.

The fail-closed path is sound.

### 2.2 `PWM_SNAPSHOT_ANCHOR_MIGRATE=1` — bypass semantics

**Analysis: PASS_WITH_NITS (see NIT-2 below).**

The env bypass (`allow_legacy_env()`) allows a legacy snapshot (no `genesis_anchor` field, no signer key) to pass trust load without emitting an error. What the bypass skips:
- For `tip == 0`: the state-vs-genesis check already runs before the anchor branch; the bypass only skips the explicit `chk_anch()` call (signature check). Safe.
- For `tip > 0`: `preflight_blk1` always runs regardless of the bypass (it's called unconditionally before the anchor branch). The bypass skips: (a) the anchor signature, (b) the explicit `gencfg_digest` binding. The state chain linkage is still implicitly verified via the block@1 replay (which uses `cfg.state0()`) and tail state root match.

**Key divergence from ADR spec (NIT-2):** ADR says the bypass requires pairing with `--snapshot-verify-chain`. The code does not enforce this — the env alone is sufficient. Concretely: a legacy snapshot loaded with `PWM_SNAPSHOT_ANCHOR_MIGRATE=1` (no verify-chain) will skip the `gencfg_digest` check, meaning economic/policy parameter tampering in a swapped genesis file is undetected if `genesis_accounts` rows match and block@1 replay passes (which it would for a correctly crafted substitute).

### 2.3 Legacy snapshot migration path vs verify-chain

**Analysis: PASS.**

Three paths are implemented consistently in `validate_snapshot_trusted`:

1. Anchor present → `chk_anch` (commitments + signature check)  
2. No anchor, signer key available → `attach_anch` (preflight → `mk_anch` → in-memory anchor + `warn`)  
3. No anchor, no signer key, no bypass → error with explicit message pointing to `--snapshot-verify-chain` or env

The `attach_anch` → `mk_anch` path creates an anchor whose `genesis_state_root` and `gencfg_digest` are freshly computed from the current `cfg`, not copied from disk. This means migration anchors are always cryptographically tied to the loaded genesis. Correct design.

The test `attach_anchor_legacy_with_signer` covers path 2 and verifies the attached anchor passes `chk_anch`.

### 2.4 `gencfg_digest` = blake3(serde_json(GenCfg)) — stability for RFC 0020

**Analysis: PASS (current), with documented future concern (NIT-4).**

`GenCfg` contains no private keys (confirmed: `funding.accounts`, `vals.set` carry public keys only). The `serde_json::to_vec(cfg)` serializes the runtime Rust struct, meaning two functionally identical configs (loaded from different files with/without optional fields) produce the same digest since defaults are materialized into the struct before serialization.

Stability risk: if `GenCfg` gains new fields (even `#[serde(default)]`), existing anchors become invalid on the next save/migration since the JSON output changes. This is acceptable for ADR 0008 (operational/devnet layer) but must be documented for RFC 0020 bootstrap compat: any GenCfg schema bump invalidates existing epoch anchors and requires re-migration.

---

## 3. Style and Module Shape

**Entity naming check (automated):** `python scripts/check_entity_name_segments.py` — **zero violations** across all five touched files.

- `anchor.rs`, `types.rs`, `io.rs`, `store.rs`, `incremental.rs` all pass ≤ 4-segment production and ≤ 5-segment test policy.
- `//!` banner present in `anchor.rs`, `io.rs`, `store.rs`, `incremental.rs`. `types.rs` has a minimal banner.
- Module shape: `anchor.rs` is a narrow, well-scoped helper module (88 lines). `io.rs` is large but pre-existing; the ADR 0008 additions are cleanly isolated in `preflight_blk1`, `attach_anch`, `allow_legacy_env`, `snap_blk1`, `fill_anch`. No new blobs in `lib.rs`/`mod.rs`.
- English in `///`/`//!`: compliant.
- Protocol wire semver: ADR 0008 explicitly scopes this as **not** changing `NodeHello` or `PWM_PROTOCOL_VERSION` — confirmed in ADR §6. No bump needed.

### Wire JSON / u128

Wire JSON / u128: **not applicable** (no peer wire / RFC wire contract in this slice). The `genesis_anchor` fields are local disk format only; `SnapshotGenAnchorV3` on disk uses hex strings for all byte arrays and `u32` for `schema_v` and `signer_prod_idx` — no `u128` in anchor fields. Existing roaming/cross-shard `u128` fields in the snapshot use `dec_of`/`dec_v2` string encoding and are pre-existing; not touched by this slice.

**NIT-3 (low): `SnapshotGenAnchor` internal serde inconsistency.**  
The `SnapshotGenAnchor` pub(crate) struct derives `Serialize`/`Deserialize` with explicit hex serde only on `signature: [u8; 64]`. The three `[u8; 32]` hash fields (`genesis_state_root`, `gencfg_digest`, `block1_hdr_hash`) use default serde, which produces integer arrays in JSON. This is NOT a production bug — `SnapshotGenAnchor` is never directly serialized to disk (always via `anchor_to_v3` / `SnapshotGenAnchorV3`). But the inconsistency in derives could confuse future maintainers or diagnostic tooling. Recommend either removing `Serialize`/`Deserialize` from `SnapshotGenAnchor` (since disk I/O bypasses it) or applying explicit hex serde to all `[u8; 32]` fields for consistency.

---

## 4. Safety

### 4.1 Anchor preimage binding

`anch_msg` concatenates `GEN_ANCH_TAG || gen_root(32) || cfg_dig(32) || blk1_hash(32)` = 118 bytes, then blake3. The tag `PWMv0/SNAPGENANCHOR/v1` is length-fixed (not a variable prefix), making length-extension / confusion attacks against blake3 irrelevant. Domain separation is adequate.

Ed25519 signing in `mk_anch` follows `ed25519_dalek::Signer::sign(msg)` on the 32-byte blake3 output — correct.

Signature verification in `chk_anch` uses `VerifyingKey::from_bytes` + `Verifier::verify` — correct; no `from_bytes_unchecked`.

### 4.2 Missing block@1 (pruned) fail-closed

Both `find_blk1` (trust load) and `snap_blk1` (persist) return explicit errors when block@1 is absent: "missing genesis anchor block1 (pruned)". The error terminates loading/saving without bypass option. Correct per ADR §5.

### 4.3 `fill_anch` hardcodes `signer_prod_idx = 0`

`fill_anch` always uses `inner.chain.val_sks.first()` and `anchor_idx = 0`. This is consistent (sk[0] corresponds to validator[0]), but for multi-validator CY topologies where the local node does not hold key[0] (e.g., it's a non-primary node with key[1]), `val_sks.first()` would return the wrong key for idx=0, causing `chk_anch` to fail on reload. If `val_sks` is ordered by validator index, this is fine; if not, this is a latent bug for multi-validator CY.  
→ **NIT-5 (low/medium)**: confirm that `val_sks` ordering in `Inner` always corresponds to validator set order by index.

### 4.4 `allow_legacy_env` bypass: `gencfg_digest` not checked on tip > 0

As noted in §2.2: with `PWM_SNAPSHOT_ANCHOR_MIGRATE=1` and `tip > 0`, the `gencfg_digest` commitment is not verified. An attacker with filesystem access who can construct a genesis file with matching `genesis_accounts` rows, matching block@1 (replaying correctly), but different economic parameters (block_reward, marks_coeff, etc.) would pass trust load. This requires a sophisticated attack (crafting block@1 replay-compatible with different genesis params), making it low probability in devnet/CY context but worth documenting.  
→ **NIT-2 (medium)**: see §3.

### 4.5 No panics / unchecked unwraps in new code

`anchor.rs` — no `unwrap()` outside tests.  
`io.rs` new functions — `find_blk1` uses `find`+`clone` (safe), `preflight_blk1` propagates errors via `?`, `attach_anch` uses `let Some(sk) = ... else { return Err(...) }`.  
One `expect("non-empty blocks")` at line 471 is guarded by `if n == 0 { return Err(...) }` two lines above — logically safe.

---

## 5. Tests

| Test | Location | Covers |
|------|----------|--------|
| `anch_sig_rt` | `anchor.rs` | anchor create + verify round-trip |
| `anch_gen_root_mismatch_err` | `anchor.rs` | genesis_state_root tamper → error |
| `preflight_blk1_tamper_tx_root` | `io.rs` | block@1 tx_root tamper → error |
| `attach_anchor_legacy_with_signer` | `io.rs` | legacy migrate with signer → anchor verifies |

**Coverage gaps (all low severity unless noted):**

- **NIT-6a**: No test for `chk_anch` with tampered `gencfg_digest` field. `anch_gen_root_mismatch_err` only covers `genesis_state_root`. Add parallel `anch_cfg_dig_mismatch_err` test.
- **NIT-6b**: No test for `signer_prod_idx` out-of-range (e.g., set to `u32::MAX`).
- **NIT-6c**: No test for `preflight_blk1` with tampered PoA signature or tampered `state_root` after replay.
- **NIT-6d**: No test for `allow_legacy_env() = true` bypass path — i.e., that a legacy snapshot with no anchor passes trust load when env is set (and doesn't panic).
- **NIT-6e**: No test for missing block@1 (pruned) scenario → expect "missing genesis anchor block1 (pruned)" error.
- No regression test for `cfg_dig` stability across struct snapshots (low priority until GenCfg schema evolves).

The existing tests are sufficient for the acceptance criteria in the task brief. Gaps are nits for coverage robustness.

---

## 6. Owner Focus Areas — Direct Answers

### Q1: Fail-closed on tampered `state` / block@1
**YES, fail-closed is correctly implemented.** State and block@1 are independently verified; neither is bypassable by env (state check runs before anchor branch for tip=0; preflight_blk1 runs unconditionally for tip>0).

### Q2: `PWM_SNAPSHOT_ANCHOR_MIGRATE=1` — safety and footguns
The env is safer than the ADR might suggest for typical cases:
- **tip=0**: state is fully verified before the env takes effect. No real bypass of commitments.
- **tip>0**: block@1 replay (indirect genesis commitment) + tail linkage + state root all still run. Only `gencfg_digest` is bypassed.

**Documented footgun**: The error message says "or set `PWM_SNAPSHOT_ANCHOR_MIGRATE=1`" as an alternative to `--snapshot-verify-chain`. ADR says they should be combined. An operator reading only the error message might set the env without verify-chain, which skips `gencfg_digest`. The guide doc should say: "Use `PWM_SNAPSHOT_ANCHOR_MIGRATE=1` for temporary bypass; combine with `--snapshot-verify-chain` for full protection." → **NIT-2**.

The env name (`MIGRATE`) implies intent to migrate, which is correct. It does not allow silently accepting states that don't match genesis (state-root checks still run).

**No bypass of preflight_blk1 (PASS)**: `preflight_blk1` always runs for `tip > 0` regardless of env. The env only skips the signature / `gencfg_digest` layer, not the block@1 chain linkage.

### Q3: Legacy snapshot without anchor — migrate vs verify-chain
Migration path (`attach_anch` when `anchor_sk` is present) is correct: preflight runs first, then fresh anchor is computed from current cfg and attached to the in-memory snapshot. The warn message fires once. Next autosnapshot persists the anchor to disk.

When signer key is absent, the error message is clear. **Footgun**: the error says "set `PWM_SNAPSHOT_ANCHOR_MIGRATE=1` for temporary bypass" but this only works if `verify_chain=false` — if verify-chain is already on, the bypass condition `!opts.verify_chain && !allow_legacy_env()` is already false without the env. The message could be clearer. → **NIT-7 (low)**.

### Q4: `gencfg_digest` stability for RFC 0020 bootstrap compat
`cfg_dig = blake3(serde_json::to_vec(cfg))` is computed from the runtime Rust struct (fully deserialized), so it is deterministic for identical runtime configs regardless of JSON file format. No secrets in `GenCfg`. **Risk**: any new field in `GenCfg` (even with `#[serde(default)]`) changes the digest for all existing configs — existing anchors require re-migration. This must be documented as a migration event. → **NIT-4 (medium, future)**. Currently no mitigation is needed.

### Q5: Wire JSON / u128 subsection mandatory
`SnapshotGenAnchor` has no `u128` fields. Existing `u128` in snapshot (roaming amounts, fee pool, balances) pre-dates this slice and uses `dec_of`/`dec_v2` string encoding. **Wire JSON / u128: not applicable for anchor fields.**

### Q6: `check_entity_name_segments.py` on touched paths
Run result: **zero violations** in all five files. Full policy compliance.

---

## 7. Verdict

**PASS_WITH_NITS**

The core fail-closed semantics are correctly implemented. The main risks are documentation gaps and one medium-severity spec divergence (env bypass alone doesn't enforce verify-chain). None of the nits require owner design decisions — all are either documentation additions or minor test extensions.

| ID | Severity | Summary | Owner fixable? |
|----|----------|---------|----------------|
| NIT-1 | low | `issues-report.md` entry absent for ADR 0008 slice | Yes |
| NIT-2 | medium | Guide doc missing `PWM_SNAPSHOT_ANCHOR_MIGRATE` + error message should say "combine with --snapshot-verify-chain" | Yes (doc only) |
| NIT-3 | low | `SnapshotGenAnchor` internal `[u8;32]` fields lack hex serde (inconsistent with `signature`) | Yes (nit, internal only) |
| NIT-4 | medium | `cfg_dig` stability contract not documented: GenCfg schema bump invalidates anchors | Yes (doc note in anchor.rs) |
| NIT-5 | low/medium | `fill_anch` hardcodes idx=0; confirm `val_sks` ordering matches validator set by index | Verify only (likely fine for current topology) |
| NIT-6a–e | low | Test coverage gaps (gencfg_dig mismatch, OOB signer, PoA tamper, env bypass path, pruned block@1) | Yes |
| NIT-7 | low | `attach_anch` error message about bypass is unreachable / misleading | Yes |

**Nits the orchestrator can fix without owner:**  
NIT-1 (add issues-report entry), NIT-2 (add ANCHOR_MIGRATE paragraph to guide + fix error message wording), NIT-4 (add `///` note to `cfg_dig`), NIT-6a–e (add tests), NIT-7 (fix unreachable error branch comment).  

**Needs owner validation:** NIT-5 (val_sks ordering in multi-validator CY — confirm it's intentional).

---

## 8. Participation / Token Estimate

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts: docs/reviews/20260612-v5-snapshot-genesis-anchor-light-review.md
token_usage:
  source: estimate
  input: ~28000
  output: ~3200
  total: ~31200
  confidence: medium
```

---

## 9. GLOSSARY.md

GLOSSARY.md: без изменений (нового жаргона не появилось — `genesis_anchor`, `gencfg_digest`, `preflight_blk1`, `PWM_SNAPSHOT_ANCHOR_MIGRATE` вводятся ADR 0008, не финальным спринтом; добавить при финальном закрывающем ревью спринта).

---

# git-handoff

```powershell
# git-handoff
Set-Location 'P:\opt\docker\pwm-protocol'
git add 'docs/reviews/20260612-v5-snapshot-genesis-anchor-light-review.md'
git add 'tasks/20260612-v5-snapshot-genesis-anchor-light-coding.json'
git commit -m "docs(adr-0008): genesis anchor light review — PASS_WITH_NITS"
```
