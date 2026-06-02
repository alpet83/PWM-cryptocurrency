# Review: V5 pre-publish polish (integrated)

**Ticket:** `20260602-v5-prepublish-polish-umbrella` (child coding slices)  
**Date:** 2026-06-02  
**Reviewer:** pwm-review  
**Verdict:** PASS_WITH_NITS

---

## 1. Scope recap

Integrated review of five coding slices under the pre-publish polish umbrella:

| Slice | Claim |
|-------|--------|
| `20260602-v5-tui-prepublish-operator-ux-coding` | Detail pane effective marks, accrual hint, burn_form V5 copy, F5 `marks_available` from effective |
| `20260602-v5-tui-test-support-mk-acct-row-coding` | `mk_acct_row` DRY helper for integration tests |
| `20260602-v5-snapshot-genesis-anchor-land-coding` | Land `anchor.rs` + ADR 0008 wiring in snapshot stack |
| `20260602-v5-prepublish-naming-violations-fix-coding` | Close six naming violations from style sweep; `rel_ms` fix in `block_timing` |
| `20260602-v5-pwmd-remove-legacy-marks-quota-coding` | Remove dead `marks_quota` mirror + quota tests |

Prior gate: `20260602-v5-prepublish-naming-style-sweep-review` reported **FAIL** (6 violations). Fix ticket is in-tree.

MVP checklist: §5 snapshot v3 (anchor, quota cleanup); §6 operator / TUI devnet (marks UX).

**Integration note:** Working tree vs `HEAD` also contains substantial diffs **outside** these five tickets (notably `crates/pwm-core/src/state.rs` lazy-marks / deferred-policy changes, large `pwmd` transport refactors, task-queue churn). This report validates the five claimed slices; orchestrator should not treat unrelated hunks as implicitly approved here.

---

## 2. Requirements fit

### 2.1 TUI operator UX (`tui-prepublish-operator-ux`)

**PASS.**

- Detail pane: `detail_marks_txt` shows `effective (stored: …, last_block: …)` using `effective_marks.unwrap_or(marks)`.
- Accrual hint: `marks_hour_left` emits `Accrual: ~N blocks until next mark hour` when staked ≥ 1 PWM whole unit and effective == stored; gated on head height.
- Burn copy: `burn_form.rs` default status and `render_burn_modal` help line use V5 stake/wait/F5 text; no user-facing Claim path.
- F5 modal: poll refresh path sets `form.marks_available` from `effective_marks.unwrap_or(marks)`; `f5_build_burn_form` in `lib.rs` already used effective marks at open — consistent.
- Unit tests: `detail_marks_uses_effective`, `marks_hour_hint_gate`, burn_form Claim assertion.

**NIT-UX-1 (low):** Accrual uses `DEF_BLOCKS_PER_HOUR` constant, not live genesis `/v1/status` params — matches ticket design note; document in runbook if operators use non-default genesis.

### 2.2 TUI test support (`tui-test-support-mk-acct-row`)

**PASS.**

- `test_support.rs` exports `mk_acct_row` with canonical defaults aligned to `AcctRow` fixture shape.
- `tests/common/mod.rs` re-exports `mk_acct_row`; `send_form.rs` and `wallet_roaming.rs` migrated to `..mk_acct_row(id)` pattern — large struct spam removed.
- No production behavior change.

### 2.3 Genesis anchor land (`snapshot-genesis-anchor-land`)

**PASS** (re-confirms prior `20260612-v5-snapshot-genesis-anchor-light-review` on landed files).

Spot-check `anchor.rs` vs ADR 0008:

| ADR element | Implementation |
|-------------|----------------|
| Tag `PWMv0/SNAPGENANCHOR/v1` | `GEN_ANCH_TAG` |
| Fields: `genesis_state_root`, `gencfg_digest`, `block1_hdr_hash`, `signer_prod_idx`, `signature` | `SnapshotGenAnchor` + V3 hex wire |
| Preimage order | tag \|\| gen_root \|\| cfg_dig \|\| blk1_hash → `blake3_32`, then Ed25519 sign |
| `st_root` = `digest(state0())` | yes |
| `cfg_dig` = blake3(serde_json(GenCfg)) | yes |
| Unit tests sign/verify + genesis_root mismatch | `anch_sig_rt`, `anch_gen_root_mismatch_err` |
| `io.rs` trust load / migrate / preflight | present (unchanged semantics from June-12 review) |
| ADR status | Updated to **Implemented** |

Known ADR nits from June-12 review (`PWM_SNAPSHOT_ANCHOR_MIGRATE` without verify-chain pairing, `fill_anch` signer idx 0) remain documented operational caveats — not regressions from land slice.

### 2.4 Naming violations fix (`prepublish-naming-violations-fix`)

**PASS.**

Automated gate (re-run 2026-06-02):

```text
python scripts/check_entity_name_segments.py \
  crates/pwm-core/src crates/pwm-cli/src crates/pwmd/src crates/pwm-tui/src \
  crates/pwm-tui/tests crates/pwm-core/tests crates/pwm-cli/tests crates/pwmd/src/tests
→ 0 violations (all files empty violations[])
```

Renames confirmed:

- Prod: `INBOUND_SOCKET_READ_LOG_SLOW_MS` → `INBOUND_READ_SLOW_MS` (4 segments).
- Tests in `block_timing.rs`: `json_stats_merge_schema`, `jsonl_tail_keeps_latest`, `pendrec_serialize_2dp`, `pendrec_parse_mixed_ms`, `pending_tail_keeps_high` (≤5 segments).

**NIT-NAMING-1 (low):** Same file carries large non-rename `ProfileTime` / seal analytics additions beyond rename-only scope — acceptable if owned by separate proposer-metrics work; testing must cover `json_stats_merge_schema` (`rel_ms` 25/40 at start_ms=1000).

### 2.5 Legacy marks_quota removal (`pwmd-remove-legacy-marks-quota`)

**PASS.**

- `marks_quota` types, `validate_quota_rows`, v2/v3 wire paths removed from `snapshot/types.rs`.
- `snap_or_mk_quota` and `snap_reject_quota_mismatch` deleted from `snapshot_roaming.rs`.
- `docs/testing-issues.md` row documents cancellation rationale.
- V3 snapshots never emitted quota; removal aligns wire with lazy marks model.

**NIT-QUOTA-1 (low):** Injected `marks_quota` in hand-edited JSON is now silently ignored on decode rather than rejected — intentional per slice design; operators must not rely on quota mirror for integrity (anchor + `stored_marks` canonical).

---

## 3. Style and module shape

- Entity naming: **clean** (0 violations) across pwm-core/cli/pwmd/pwm-tui src+tests.
- `test_support.rs` has minimal `//!` banner (prior sweep “missing banner” nit closed).
- TUI helpers `detail_marks_txt`, `marks_hour_left` are ≤4 segments; tests ≤5.
- `anchor.rs`: narrow module (~108 lines), English banner, fits snapshot submodule pattern.
- Integrated tree still includes unrelated large blobs (`pwmd` transport, `block_timing` analytics expansion) — not style failures for these slices but worth separate review if merged wholesale.

### Wire JSON / u128

Wire JSON / u128: **not applicable** for TUI/test-support/naming/quota-removal slices. Genesis anchor fields on disk use hex strings and `u32` indices — no new peer `PeerWireMsg` / `u128` derive-only payloads in these five slices. Existing snapshot account balances retain pre-existing decimal-string encoding via `dec_of`/`dec_v2`.

---

## 4. Safety

- **TUI:** Display-only changes; F5 burn still gated by existing preflight/signing paths; effective marks cannot inflate spendable marks on-chain — UI honesty fix only.
- **Anchor:** Fail-closed trust load preserved; single-signer fool-guard; no new env bypass in land slice.
- **marks_quota removal:** Reduces attack surface confusion (dead mirror); does not weaken v3 account `stored_marks` integrity.
- **Scope bleed risk:** `pwm-core/state.rs` removes `touch_state_acct` from several tx arms and alters deferred-policy deactivate matching — **not in umbrella scope**; if merged unintentionally, could affect lazy-marks materialization and policy semantics. Orchestrator must confirm owning ticket before publish.

---

## 5. Tests

| Area | Present | Gap / pwm-testing ask |
|------|---------|------------------------|
| TUI detail/accrual/burn copy | `tui_loop` + `burn_form` unit tests | Run `cargo test -p pwm-tui --lib` |
| TUI integration DRY | migrated tests compile-time | Run `cargo test -p pwm-tui send_form wallet_roaming` |
| Anchor | `anchor.rs` unit + `io.rs` integration tests | Run `cargo test -p pwmd --lib` filter `anch_`, `preflight_blk1`, `attach_anchor` |
| block_timing rel_ms | `json_stats_merge_schema` | Run `cargo test -p pwmd --lib json_stats_merge_schema block_timing` |
| Snapshot roaming | quota tests removed; other snap tests remain | Run `cargo test -p pwmd --lib snapshot` (or `snap_` prefix) |
| Naming | scanner automation | Already 0 violations |

**pwm-testing recommendation:** Run **both** targets — not snapshot-only:

1. **pwm-tui:** `--lib` + integration `send_form` + `wallet_roaming` (slices 1–2).
2. **pwmd:** snapshot module tests + `block_timing` / `json_stats_merge_schema` (slices 3–5).

Skip full pwmd transport/production harness unless orchestrator explicitly bundles transport diff in same merge.

---

## 6. Verdict

**PASS_WITH_NITS**

Prior naming sweep **closed**: scanner 0 violations on pwm crates.

Prior naming sweep six items: **all resolved** in-tree.

User-facing Claim strings in pwm-tui src: **none** (only test assertions and internal `ipv4_claim_phases` field name).

### Nit list (prioritized)

1. **NIT-INTEG-1 (medium — orchestrator):** Working tree bundles out-of-scope `pwm-core/state.rs` and large `pwmd` transport changes — confirm ticket ownership before merge; not validated as part of polish umbrella.
2. **NIT-UX-1 (low):** Accrual hint uses `DEF_BLOCKS_PER_HOUR`, not live genesis params.
3. **NIT-NAMING-1 (low):** `block_timing.rs` contains substantial analytics feature work beyond rename-only; ensure proposer-metrics ticket traceability.
4. **NIT-QUOTA-1 (low):** Legacy `marks_quota` JSON injection no longer fails decode — document as dead field.
5. **NIT-ANCHOR-1 (low, carry-forward):** ADR 0008 migrate env bypass semantics per June-12 review — operational doc gap, not introduced here.

No **BLOCKED** items within the five claimed coding slices.

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts:
  - docs/reviews/20260602-v5-prepublish-polish-integrated-review.md
token_usage:
  source: estimate
  input: 12000
  output: 3500
  total: 15500
  confidence: medium
```

---

## Per-slice pwm-review results (orchestrator)

| Ticket | pwm-review result |
|--------|-------------------|
| `20260602-v5-tui-prepublish-operator-ux-coding` | PASS |
| `20260602-v5-tui-test-support-mk-acct-row-coding` | PASS |
| `20260602-v5-snapshot-genesis-anchor-land-coding` | PASS_WITH_NITS (carry-forward ADR migrate nit) |
| `20260602-v5-prepublish-naming-violations-fix-coding` | PASS_WITH_NITS (block_timing scope bleed) |
| `20260602-v5-pwmd-remove-legacy-marks-quota-coding` | PASS_WITH_NITS (silent ignore nit) |

**pwm-testing:** full **pwm-tui** lib + integration **and** **pwmd** snapshot + block_timing subset.
