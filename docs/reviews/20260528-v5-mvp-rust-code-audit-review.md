# Rust Code Audit — MVP V5 window

**Date:** 2026-05-28  
**Ticket:** `20260528-v5-mvp-rust-code-audit-review`  
**Audit window:** `fe06f6b..9d8d0b9`  
**Audited paths:** V5-touched Rust files, prioritized around core state/tx, API tx submit, genesis/snapshot parsing, CLI signing helpers, and TUI marks/claim surfaces  
**Categories checked:** all rust-code-audit categories + behavioral/security regressions + missing tests  
**Tool:** rust-code-audit skill (habr.com/ru/articles/1035712), commit-window triage, direct file reads, grep, entity-name check

---

## Executive summary

| Severity | Count |
|----------|-------|
| Critical | 0 |
| High | 2 |
| Warning | 5 |
| Note | 4 |
| **Total** | **11** |

**Verdict:** `needs attention`

No rust-code-audit category produced a merge-blocking memory-safety finding in the V5-touched production files: no V5 `unsafe`, no `std::sync::Mutex` held across `.await`, no lifetime laundering, no large stack allocation, and no public blanket-impl hazard.

The main risk is behavioral/security drift in the V5 integration layer:

1. `DeactivatePolicy` cannot deactivate a deferred reversible policy after it has auto-activated by height.
2. The `claim-ipv4-batch` harness helper exposes deterministic fallback signing keys and can reuse claimant key material as registry signing material.
3. V5 account-mutating paths (`ClaimIPv4Batch`, `Export`, `Import`) do not call the lazy-marks touch helper, which is at least a spec/invariant gap and needs an explicit decision.

---

## Scope and triage

### Commit window

Reviewed window per ticket: `fe06f6b..9d8d0b9`.

Representative commits in scope:

- `0a42522` / `bf68bd8` / `4df1e02` / `73aa13c`: V5 core model, claim retirement, IPv4 batch tx shape, snapshot v3.
- `b28e02f` / `80aeccc` / `02ea946`: lazy marks and float inflation runtime.
- `4471085` / `260dccb` / `d3bd26b`: deferred activation model/evaluator/tests.
- `f016074` / `795d170`: ClaimIPv4Batch apply and reject matrix.
- `8b69a3a` / `ebeb161` / `fed8426`: TUI marks display, account-info marks detail, deferred policy CLI.
- `fd94191` / `c930024` / `f5d4535` / `f21f243` / `9d8d0b9`: V5-8 smoke harness and closeout.

### Touched Rust files by subsystem

`git diff --name-only fe06f6b..9d8d0b9 -- '*.rs'` produced 37 Rust files:

- **Core state/tx/economics:** `crates/pwm-core/src/state.rs`, `crates/pwm-core/src/tx.rs`, `crates/pwm-core/src/marks.rs`, `crates/pwm-core/src/chain.rs`, `crates/pwm-core/src/genesis.rs`, `crates/pwm-core/src/types.rs`, `crates/pwm-core/src/lib.rs`.
- **Daemon API/snapshot:** `crates/pwmd/src/api/common.rs`, `crates/pwmd/src/api/handlers_tx.rs`, `crates/pwmd/src/api/types.rs`, `crates/pwmd/src/lifecycle.rs`, `crates/pwmd/src/snapshot/ch_http.rs`, `crates/pwmd/src/snapshot/genesis.rs`, `crates/pwmd/src/snapshot/io.rs`, `crates/pwmd/src/snapshot/repair.rs`, `crates/pwmd/src/snapshot/types.rs`, `crates/pwmd/src/tests/http_export.rs`, `crates/pwmd/src/tests/http_status.rs`, `crates/pwmd/src/transport/peer_session/mod.rs`, `crates/pwmd/src/transport/peer_session/sync_live.rs`.
- **CLI/signing/harness:** `crates/pwm-cli/src/bin/claim_ipv4_batch.rs`, `crates/pwm-cli/src/cli_cmd.rs`, `crates/pwm-cli/src/cli_dispatch.rs`, `crates/pwm-cli/src/cli_parse.rs`, `crates/pwm-cli/src/cmd_account.rs`, `crates/pwm-cli/src/cmd_genesis.rs`, `crates/pwm-cli/src/cmd_tx.rs`, `crates/pwm-cli/src/lib.rs`, `crates/pwm-cli/src/main.rs`, `crates/pwm-cli/src/signer.rs`, `crates/pwm-cli/src/tests/mod.rs`.
- **TUI marks/claim surfaces:** `crates/pwm-tui/src/account_view.rs`, `crates/pwm-tui/src/lib.rs`, `crates/pwm-tui/src/marks_display.rs`, `crates/pwm-tui/src/models.rs`, `crates/pwm-tui/src/tui_loop.rs`, `crates/pwm-tui/src/tx_submit.rs`.

### Narrowed high-risk scope proposal

Top files/functions for a follow-up fix pass:

1. `crates/pwm-core/src/state.rs`: `apply_policy_action`, `policy_is_active_at`, `set_pol_mode`, `apply_tx_with_ctx` arms for `ClaimIPv4Batch`, `Export`, `Import`.
2. `crates/pwm-cli/src/bin/claim_ipv4_batch.rs`: fallback signing material and registry/claimant key split.
3. `crates/pwm-cli/src/signer.rs`: `TxSignerSource` public fields, especially `sk`.
4. `crates/pwmd/src/api/handlers_tx.rs`: direct-seal path for `Export | Import | ClaimIPv4Batch`, cancellation-safety annotation/contract.
5. `crates/pwmd/src/snapshot/genesis.rs`: `parse_claim_phases` duplicate detection and u128 JSON contract.
6. `crates/pwm-tui/src/tui_loop.rs` and `crates/pwm-tui/src/tx_submit.rs`: retired `ClaimTx` UI flow and dead success/error branches.

Secondary watchlist:

- `crates/pwm-cli/src/cmd_account.rs` and `crates/pwm-tui/src/marks_display.rs`: duplicated display-only `GenCfg` construction and saturation helper.
- `crates/pwm-core/src/tx.rs`: manual `TxBody` deserialize mirror should keep round-trip tests for all variants.
- `crates/pwmd/src/snapshot/types.rs`: V2-to-V3 migration semantics and economic field string encoding.

---

## Findings

### HIGH-001: Deferred reversible policy cannot be deactivated after auto-activation

**Location:** `crates/pwm-core/src/state.rs:573`

```rust
PolicyAction::DeactivatePolicy { policy_id } => {
    let policy =
        PolicyKind::from_policy_id(*policy_id).ok_or(TxError::PolicySchemaInvalid)?;
    if !policy.is_reversible() {
        return Err(TxError::PolicyIrreversible);
    }
    if let Some(idx) = acc
        .deferred_policies
        .iter()
        .position(|row| row.policy == policy && inclusion_height < row.activate_at_height)
    {
        acc.deferred_policies.remove(idx);
        return Ok(());
    }
    let bit = policy.bit();
    if acc.active_policies & bit == 0 {
        return Err(TxError::PolicyNotActive);
    }
```

**Why:** Deferred activation is evaluated lazily by `policy_is_active_at`: a row in `deferred_policies` is considered active once `chain_tip_height >= activate_at_height`. But `DeactivatePolicy` only removes a deferred row before activation (`inclusion_height < activate_at_height`). After the activation height, the policy can reject transfers through `policy_is_active_at`, while the `active_policies` bit may still be unset. A later `DeactivatePolicy` therefore returns `PolicyNotActive` and leaves the deferred row in place.

**Impact:** A reversible policy installed as deferred can become effectively non-deactivatable after its activation height. This is a behavioral regression in policy governance semantics and can permanently lock an account into a filter/default policy unless another `SetPolicy` path overwrites it.

**Fix direction:** Treat an activated deferred row as active for deactivation too: remove the deferred row when `row.policy == policy`, then decide whether to move to dormant based on reversible semantics. Add tests for `DeactivatePolicy` at `activate_at_height` and after `activate_at_height`.

---

### HIGH-002: IPv4 claim harness uses deterministic fallback signing keys and may reuse claimant key as registry key

**Location:** `crates/pwm-cli/src/bin/claim_ipv4_batch.rs:98`

```rust
} else {
    // Fallback to default test seed
    let seed = [0x45u8; 32];
    let key = derive_ed25519_private_key(&seed, &[0, 0]);
```

**Location:** `crates/pwm-cli/src/bin/claim_ipv4_batch.rs:121`

```rust
} else if args.wallet.is_some() {
    let sk = SigningKey::from_bytes(&claim_sk.to_bytes());
    (sk, claim_id)
```

**Why:** The helper is documented as a harness binary, not a general user CLI, but it is a real binary under `src/bin`. Running it without explicit claimant material silently derives a public deterministic claimant from `[0x45; 32]`; running with `--wallet` and without `--registry-seed` signs the registry authorization with the claimant key.

**Impact:** Accidental operator use outside the smoke harness can mint transactions with well-known private keys or collapse the claimant/registry trust separation. The V5 smoke may intentionally configure the registry address to a demo wallet, but that contract is not enforced or named as dev-only.

**Fix direction:** Require explicit `--claimant-seed` or `--wallet`; require explicit `--registry-seed` or an explicit `--dev-registry-is-claimant` / `--dev-defaults` flag. Make the help text fail-closed for non-smoke use.

---

### WARN-001: V5 account-mutating paths do not materialize lazy marks

**Location:** `crates/pwm-core/src/state.rs:279`

```rust
TxBody::ClaimIPv4Batch { .. } => {
    ...
    let mut a = acc;
    a.balance_pwm = a.balance_pwm.saturating_add(phase_row.allocation);
    a.ipv4_claimed_phase = Some(*phase);
    a.nonce += 1;
```

**Location:** `crates/pwm-core/src/state.rs:299`

```rust
TxBody::Export { amount, fee, .. } => {
    ...
    let mut from = acc;
    from.balance_pwm -= total;
    from.nonce += 1;
```

**Location:** `crates/pwm-core/src/state.rs:332`

```rust
TxBody::Import { .. } => {
    ...
    let mut from = acc;
    ...
    from.balance_pwm -= fee;
```

**Why:** Transfer, Stake, Unstake, BurnMark, and Policy all call `touch_acct_mrks` before mutating account state. `ClaimIPv4Batch`, `Export`, and `Import` mutate balance/nonce/claim state without touching marks. Mathematically, this may not lose marks if stake is unchanged and a future touch uses the older cursor; however V5 plan language says touch semantics apply on state-touch paths, and these arms create inconsistent stored/cursor behavior after successful transactions.

**Impact:** Account state after claim/export/import can have stale `stored_marks` and `marks_last_block` even though the account was mutated. That can confuse operator surfaces that inspect stored values, complicate state-root expectations, and leave the exact touch invariant underspecified.

**Fix direction:** Either call `touch_acct_mrks` in all three arms, or document explicitly that non-stake-affecting V5 paths are allowed to skip materialization. Add regression tests for marks/cursor behavior on `ClaimIPv4Batch`, `Export`, and `Import`.

---

### WARN-002: Direct-seal HTTP path is not annotated for async cancellation safety

**Location:** `crates/pwmd/src/api/handlers_tx.rs:112`

```rust
if let Err((msg, _)) = g.chain.seal(vec![tx.clone()]) {
    rollback_commit(&mut g, bak);
    return Err((
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("seal after roaming tx failed: {msg}"),
    ));
}
...
let save_result = snap_save_locked(&a, &g);
...
drop(g);
let mut st = a.init.write().await;
```

**Category:** CAT-5 async cancellation safety.

**Why:** `v1_tx` mutates chain state synchronously after an `async` lock acquisition, then later reaches another `.await` while completing init-state publication. There is no `// NOT cancel-safe` or equivalent contract. If the HTTP future is cancelled after seal and before all post-seal bookkeeping completes, clients can retry without a clear idempotency story.

**Impact:** The code has rollback for explicit seal/snapshot errors, but not for future cancellation. Duplicate-submit behavior may be safe at the chain layer through nonce/replay checks, but that safety is not documented at the cancellation boundary.

**Fix direction:** Add a cancellation-safety contract comment to `v1_tx` or the direct-seal branch. If this path must be robust under cancellation, move post-seal persistence/bookkeeping into a durable idempotent section or background task. Add an integration test or design note for cancelled request/retry behavior.

---

### WARN-003: Public CLI library exposes raw signing key field

**Location:** `crates/pwm-cli/src/signer.rs:10`

```rust
pub struct TxSignerSource {
    pub sk: SigningKey,
    pub dom: u16,
    pub idx: u32,
    pub from: AccountId,
}
```

**Why:** V5-8 added `pwm-cli` as a library for harness binaries and made `TxSignerSource` public with public fields. The raw `SigningKey` is now exposed to every downstream crate depending on `pwm-cli`, not just the harness binary that needed it.

**Impact:** This expands the key-material API surface and makes accidental logging, cloning, or misuse easier. Even if `pwm-cli` is not treated as a stable public crate, this is a security boundary regression.

**Fix direction:** Keep the struct fields private, expose minimal methods required by `claim_ipv4_batch`, or move the harness helper behind a crate-internal module/API. If public exposure is intentional, document the API and its non-stable/security-sensitive status.

---

### WARN-004: Genesis IPv4 claim phases do not reject duplicate phase IDs

**Location:** `crates/pwmd/src/snapshot/genesis.rs:236`

```rust
fn parse_claim_phases(rows: Vec<GenesisClaimPhaseV4>) -> Result<Vec<ClaimPhaseConfig>, String> {
    rows.into_iter()
        .enumerate()
        .map(|(i, row)| {
            ...
            Ok(ClaimPhaseConfig {
                phase: row.phase,
                registry_address,
                allocation,
            })
        })
        .collect()
}
```

**Why:** Duplicate `phase` rows parse successfully. Later core lookup uses the first matching row in `find_claim_phase`, making duplicate rows ambiguous and order-dependent.

**Impact:** A malformed genesis can contain two phase definitions with different registry addresses or allocations. Operators may believe one row is active while the chain applies another.

**Fix direction:** Reject duplicate phase IDs during genesis parsing. Add a `gen_ipv4_phases_reject_dup` test with conflicting duplicate rows.

---

### WARN-005: Retired ClaimTx still drives TUI F5 burn flow through dead submit path

**Location:** `crates/pwm-tui/src/tui_loop.rs:616`

```rust
let claim_res = submit_claim(
    &owner.id,
    0,
    ui.head_height.unwrap_or(0),
    &identity,
);
```

**Location:** `crates/pwm-tui/src/tx_submit.rs:116`

```rust
pub fn submit_claim(...) -> Result<(), String> {
    Err("ClaimTx is retired in V5".to_string())
}
```

**Why:** V5 retired `ClaimTx`, but the TUI F5 flow still calls `submit_claim`, then keeps success and legacy `E_CLAIM_OVER_MATURED` branches that are now unreachable. It also polls a snapshot after the guaranteed error path.

**Impact:** The UI path is confusing for operators and future maintainers: it looks like ClaimTx can still be submitted, but it always fails. This is not a consensus bug, but it is a stale UX/maintenance hazard.

**Fix direction:** Replace the call with a direct V5 informational message and remove dead success/legacy claim branches. Add a test that F5 burn does not attempt retired claim submission.

---

### NOTE-001: CLI account parser accepts JSON-number u128 only up to u64

**Location:** `crates/pwm-cli/src/cmd_account.rs:146`

```rust
fn parse_u128_field(v: &Value, field: &str) -> Option<u128> {
    v.get(field).and_then(|n| match n {
        Value::Number(num) => num.as_u64().map(u128::from),
        Value::String(s) => s.parse::<u128>().ok(),
        _ => None,
    })
}
```

**Why:** The string path handles full `u128`, while the number path only accepts values representable as `u64`. Current V5 API/account surfaces appear to serialize economic `u128` as strings, so this is mainly a defensive compatibility note.

**Fix direction:** Prefer requiring string encoding for all economic `u128` fields; if number compatibility is desired, reject out-of-range numbers explicitly with a visible parse error instead of silently returning `None`.

---

### NOTE-002: Multi-file snapshot repair operation is not atomic as a group

**Location:** `crates/pwmd/src/snapshot/repair.rs:74`

```rust
rewrite_epochs(summary_path, &all, target_h)?;
rewrite_manifest(summary_path, &all, target_h, &replay_target.last_good_hash)?;
rewrite_summary(summary_path, ...)?;
```

**Category:** CAT-3 Drop / RAII trap (transaction-like multi-step update).

**Why:** Individual writes may be atomic, but the repair operation updates multiple snapshot files. If a later rewrite fails, earlier rewrites remain applied. Backup is optional and no group rollback is attempted.

**Impact:** Offline repair can leave partially rewritten snapshot state on disk after an error.

**Fix direction:** Use a temp repair directory and atomic swap, or force backup for non-dry-run repair. At minimum, document the partial-failure semantics in the command/reporting path.

---

### NOTE-003: Manual `TxBody` deserialize mirror should keep exhaustive round-trip tests

**Location:** `crates/pwm-core/src/tx.rs:188`

```rust
impl<'de> Deserialize<'de> for TxBody {
    ...
    enum RawTxBody { ... }
```

**Why:** The manual deserializer is appropriate for retiring `claim_mark` while keeping structured errors, and `ClaimIPv4Batch` has focused JSON/signing tests. But the duplicate enum mirror is easy to drift as V5+ tx variants evolve.

**Fix direction:** Keep/extend round-trip tests for every `TxBody` variant, especially after adding `ClaimIPv4Batch` and deferred policy action encoding.

---

### NOTE-004: Display-only marks helpers are duplicated in CLI and TUI

**Locations:** `crates/pwm-cli/src/cmd_account.rs:172`, `crates/pwm-tui/src/marks_display.rs`

**Why:** Both surfaces construct placeholder `GenCfg`/`Account` values and calculate saturation percentage for display. This is not a security bug, but V5 showed `GenCfg` shape churn; duplicated constructors are likely to break again when config fields evolve.

**Fix direction:** Consider a shared display helper in `pwm-core`, for example `marks_saturation_pct` and a small wrapper for effective marks display.

---

## rust-code-audit category results

### CAT-1 Lifetime laundering

No findings in V5-touched files. Serde lifetimes such as `Deserialize<'de>` return owned values and do not tie input borrows to mutable containers.

### CAT-2 `std::sync::Mutex` across `.await`

No findings in V5-touched production files. The async paths reviewed use Tokio locks. The repository still has test-only `std::sync::Mutex` usage for environment serialization, not a V5 production issue.

### CAT-3 Drop / RAII trap

One note: snapshot repair is transaction-like across multiple files and not group-atomic. No database `commit().await?` pattern was found in V5-touched files.

### CAT-4 `unsafe` without `// SAFETY:`

No V5 production findings. V5-touched production files do not add `unsafe` blocks. Repository-level grep still sees existing test-only environment mutation unsafe blocks from older work; those are outside this V5 audit scope unless separately requested.

### CAT-5 Async cancellation safety

One warning: `pwmd` direct-seal HTTP path performs non-idempotent chain mutation before a later `.await` and lacks an explicit cancellation-safety contract.

### CAT-6 Blanket impl semver hazard

No findings. V5 touched concrete `From` impls and ordinary helper functions, not public blanket implementations.

### CAT-7 Large stack allocation

No findings in V5-touched files. Repository grep saw `let mut buf = [0u8; 32768]` in `snap_bench_hlp.rs`, below the 64 KiB threshold and outside the V5 focus.

---

## Wire JSON / u128

Scope: V5 touched transaction JSON (`TxBody`), daemon API types, snapshot/genesis JSON, and transport-adjacent files. No new peer-wire `u128` derive-only payload was found in the V5 focus.

- `crates/pwm-core/src/tx.rs`: `Transfer`, `Stake`, `Unstake`, `Export`, `Import`, and `Policy` economic fields use `#[serde(with = "crate::ser_json_u128")]` in both `TxBody` serialize and manual deserialize mirror.
- `crates/pwm-core/src/state.rs`: `ExportProvenance.amount` uses `#[serde(with = "crate::ser_json_u128")]`.
- `crates/pwmd/src/api/types.rs`: daemon API `u128` amount fields use `ser_u128_as_str`; account-info surfaces expose balances/staked values as strings.
- `crates/pwmd/src/transport/**`: existing wire tests cover account views and cross-shard facts with large `u128` values and hex/string compatibility.
- Watchlist: `crates/pwm-core/src/types.rs::Account` still derives `Serialize/Deserialize` over `u128` fields directly. This appears to be core/local state representation rather than peer wire, but it should not be exposed as peer JSON without a wrapper.

Conclusion: no request-change finding for peer wire `u128` encoding in this V5 audit. Keep API/snapshot wrappers as the public JSON boundary.

---

## Style and module shape

- Ran `python scripts/check_entity_name_segments.py` on the high-risk focus files listed in the ticket. Result: no violations for production `fn`/types under the current policy (`prod_max=4`, `test_max=5`).
- New helper binary `crates/pwm-cli/src/bin/claim_ipv4_batch.rs` has a clear module banner that says it is a harness helper and not a general user CLI command.
- V5 introduced `pwm-cli` library exports for harness binaries. This fixed compile/use ergonomics, but the key-material export is too broad (see WARN/HIGH finding above).
- No new large facade blob was identified beyond the expected V5 additions in `state.rs`; `state.rs` remains the highest-risk dense module due to many transaction arms.

---

## Missing tests / targeted regression tests

Recommended tests for follow-up tickets:

1. `policy_deferred_deact_at_h` and `policy_deferred_deact_after_h`: deactivation of a reversible deferred policy at/after activation height should succeed or have an explicitly documented rejection semantics.
2. `claim_ipv4_touch_marks`, `export_touch_marks`, `import_touch_marks`: assert chosen V5 touch semantics for marks/cursor on all account-mutating paths.
3. `gen_ipv4_phases_reject_dup`: duplicate genesis `phase` rows should fail parse.
4. `claim_ipv4_batch_requires_explicit_keys`: harness should fail without explicit dev/default opt-in and should not silently reuse claimant key as registry key.
5. `tui_f5_claim_retired_no_submit`: F5 flow should not call retired `submit_claim`.
6. `snap_v2_to_v3_migration_round_trip`: full V2 snapshot decode -> V3 data -> encode/decode round-trip, beyond account-level conversion.
7. `v1_tx_direct_seal_cancel_contract`: document/test retry behavior when direct-seal path is cancelled or the client disconnects.

---

## Open assumptions / questions

- Is `ClaimIPv4Batch` intentionally allowed to skip lazy mark materialization because it changes liquid balance but not stake? If yes, this should be documented in the V5 model and tests should lock it down.
- Is `Export`/`Import` intentionally outside V5 touch semantics? The V5 plan enumerates Transfer/Stake/Unstake/BurnMark/PolicyTx/INIT, but the phrase "state-touch" suggests all account-mutating paths.
- Is `claim-ipv4-batch` intended to ship in normal `cargo build --workspace` artifacts, or is it strictly a private harness binary? The answer affects how severe the deterministic fallback keys are.
- Should a deferred reversible policy that has auto-activated be deactivated by `DeactivatePolicy`, or only overwritten by `SetPolicy`? Current behavior should be made explicit either way.

---

## Verification performed

- `git diff --name-only fe06f6b..9d8d0b9 -- '*.rs'` to enumerate V5 Rust scope.
- `git log --oneline --no-merges fe06f6b..9d8d0b9` to cluster work by sprint/subsystem.
- `python scripts/check_entity_name_segments.py ...` on ticket focus files: no naming violations.
- Grep scans for rust-code-audit signals: `unsafe`, `std::sync::Mutex`, `parking_lot::Mutex`, `.commit().await?`, public blanket impl patterns, large fixed arrays, V5 transaction/policy/claim symbols.
- Direct file reads for high-risk findings in `state.rs`, `tx.rs`, `handlers_tx.rs`, `snapshot/genesis.rs`, `cmd_account.rs`, `signer.rs`, `claim_ipv4_batch.rs`, `tui_loop.rs`, `tx_submit.rs`.

No product code was modified by this audit.

---

## Participation / token estimate

```yaml
agent: pwm-review
result: PARTIAL
artifacts: docs/reviews/20260528-v5-mvp-rust-code-audit-review.md
token_usage:
  source: estimate
  input: 36000
  output: 6200
  total: 42200
  confidence: medium
```

Result is `PARTIAL` rather than `PASS` because the audit found follow-up issues that need coding-owner decisions/fixes.

---

## Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'P:\opt\docker\PWM-cryptocurrency'
git add 'docs/reviews/20260528-v5-mvp-rust-code-audit-review.md'
git add 'tasks/done/20260528-v5-mvp-rust-code-audit-review.json'
git commit -m 'docs(v5): rust code audit review'
```
