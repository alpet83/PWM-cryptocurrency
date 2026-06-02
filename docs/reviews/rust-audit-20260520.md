# Rust Code Audit — crates/pwm-core, pwmd, pwm-cli, pwm-tui

**Date:** 2026-05-20  
**Audited paths:** Pass 1: `crates/pwm-core/src/`, `crates/pwmd/src/`, `crates/pwm-cli/src/`, `crates/pwm-tui/src/`; Pass 2: `crates/pwmd/tests/`, `crates/pwmd/src/bin/`, `crates/pwmd/benches/`, `crates/pwm-cli/tests/`, `crates/pwm-tui/tests/`  
**Categories checked:** all  
**Tool:** rust-code-audit skill (habr.com/ru/articles/1035712)

---

## Summary

| Severity | Count |
|----------|-------|
| 🔴 Critical | 3 |
| 🟡 Warning  | 2 |
| 🔵 Note     | 0 |
| **Total**   | 5 |

**Verdict:** `critical findings — do not merge`

Per skill severity table, unresolved CAT-4 items are 🔴 Critical (even though they are confined to **integration tests**, not shipped binary code). Operational risk primarily remains the two CAT-5 roaming/relay warnings in production `pwmd` paths.

**Combined totals:** Pass 1 contributed 2 CAT-5 warnings; Pass 2 contributed 3 CAT-4 critical findings.

---

## Findings

### CAT-1 Lifetime laundering

No findings.

---

### CAT-2 std::sync::Mutex across .await

No findings.

Checked `std::sync::Mutex` / `.lock()` hits in scope. The flagged `pwmd` locks are dropped before later `.await` points, used in synchronous `Write` impls/tests, or are async `tokio` locks rather than `std::sync::MutexGuard` held across `.await`.

---

### CAT-3 Drop / RAII trap

No findings.

No `commit().await?` transaction pattern was found in the audited paths.

---

### CAT-4 unsafe without SAFETY comment

**Pass 1:** no issues — the only `unsafe { ... }` blocks found under `**/src/` are test-only env-var mutations in `crates/pwmd/src/logging.rs`; both preceded by `// SAFETY:`.

**Pass 2** (integration tests / bins / benches; details under *Pass 2* section): three `remove_var` calls without adjacent `// SAFETY:` in `crates/pwm-tui/tests/send_form.rs` — **[C4-003]** … **[C4-005]**.

---

### CAT-5 Async cancellation safety

**[C5-001]** `crates/pwmd/src/relay.rs:566` and `crates/pwmd/src/relay.rs:627`  
```rust
let resp = client.post(&url).json(tx).send().await.map_err(|e| {
    // remote import request
})?;
// ...
// Mirror roaming completion on the source shard (relay bypasses local seal path).
{
    let mut g = app.inner.write().await;
    let bak = take_bak(&g);
    g.roaming_pool.mark_import_by_export(export_key);
```
_Why:_ `relay_import` performs a non-idempotent remote submit (`POST /v1/tx`) and only later updates the source shard's local roaming state and snapshot through additional `.await` points. The function is awaited directly from the HTTP handler `v1_tx`; if the future is cancelled after the remote import succeeds but before the local source mark/snapshot completes, the target can observe the import while the source still looks unimported or relay-pending. There is no `// NOT cancel-safe` annotation or detached non-cancellable section documenting this contract.  
_Fix direction:_ make the post-success local reconciliation non-cancellable/idempotent, or explicitly mark the function as not cancel-safe and route it through a task/queue with retryable reconciliation.

**[C5-002]** `crates/pwmd/src/api/handlers_roaming.rs:235` and `crates/pwmd/src/api/handlers_roaming.rs:356`  
```rust
match crate::relay::relay_handoff(&a, &handoff).await {
    Ok(()) => {
        out.status =
            mark_relay_ok(&a, intent_id, tx.export_id().unwrap_or([0u8; 32])).await?;
    }
```
_Why:_ the roaming handlers call `relay_handoff`, which performs remote `POST /v1/export-provenance`, then update local intent status in a later awaited step (`mark_relay_ok`). Cancellation between remote success and the local status update can leave the local intent in queued/exported state even though the target provenance was already delivered. The same pattern appears in finalize (`should_relay` path). No cancel-safety annotation documents whether retry/duplicate delivery is acceptable.  
_Fix direction:_ make relay handoff completion idempotent and persist local status in a cancellation-safe section, or annotate the handler path as not cancel-safe and move relay completion to a durable background workflow.

---

### CAT-6 Blanket impl semver hazard

No findings.

No public `impl<T: ...> Trait for T` blanket impl pattern was found in the audited paths.

---

### CAT-7 Large stack allocation

No findings.

One fixed buffer hit was reviewed: `let mut buf = [0u8; 32768];` in `crates/pwmd/src/snap_bench_hlp.rs`. It is 32 KiB and below the skill threshold (`N * size_of(T) > 65536`), so it is not reported as a finding.

---

## Pass 2 — integration tests, pwmd binaries, benches

**Date:** 2026-05-20  
**Audited paths:** `crates/pwmd/tests/`, `crates/pwmd/src/bin/`, `crates/pwmd/benches/`, `crates/pwm-cli/tests/`, `crates/pwm-tui/tests/`  
**Categories checked:** all

### CAT-1 Lifetime laundering

No findings.

---

### CAT-2 std::sync::Mutex across .await

No findings.

The only `std::sync::Mutex` signal in Pass 2 is `TEST_ENV_LOCK` in `crates/pwm-tui/tests/common/mod.rs`; it is used in synchronous tests and is not held across any `.await`.

---

### CAT-3 Drop / RAII trap

No findings.

No `commit().await?` transaction pattern was found in Pass 2 scope.

---

### CAT-4 unsafe without SAFETY comment

**[C4-003]** `crates/pwm-tui/tests/send_form.rs:304`  
```rust
let _guard = TEST_ENV_LOCK.lock().unwrap();
unsafe { std::env::remove_var("PWM_TUI_WALLET_UNLOCK_SECS") };
```
_Why:_ Rust 2024 marks environment mutation as unsafe because it changes process-global state. The test serializes with `TEST_ENV_LOCK`, but this `unsafe` block has no immediate `// SAFETY:` comment documenting the single-thread/test-isolation invariant.  
_Fix direction:_ add a `// SAFETY:` comment immediately before the block, or centralize env mutation behind a small test helper that documents and enforces the `TEST_ENV_LOCK` contract.

**[C4-004]** `crates/pwm-tui/tests/send_form.rs:314`  
```rust
let _guard = TEST_ENV_LOCK.lock().unwrap();
unsafe { std::env::remove_var("PWM_TUI_WALLET_UNLOCK_SECS") };
```
_Why:_ same env-var mutation pattern as C4-003: the block is probably intended to be serialized by `TEST_ENV_LOCK`, but the required safety invariant is not documented adjacent to the `unsafe` block.  
_Fix direction:_ add a local `// SAFETY:` comment or route through a documented locked helper.

**[C4-005]** `crates/pwm-tui/tests/send_form.rs:344`  
```rust
let _guard = TEST_ENV_LOCK.lock().unwrap();
unsafe { std::env::remove_var("PWM_TUI_WALLET_UNLOCK_SECS") };
```
_Why:_ same missing `// SAFETY:` documentation for process-global environment mutation. This file already documents later env-var `set_var`/`remove_var` blocks, so the fix is local and mechanical.  
_Fix direction:_ add a local `// SAFETY:` comment or route through a documented locked helper.

---

### CAT-5 Async cancellation safety

No findings.

Pass 2 scope has no `.await` hits in the audited tests, `pwmd` binaries, or `pwm-cli`/`pwm-tui` integration tests. The benchmark uses synchronous Criterion closures and does not introduce a cancellation boundary.

---

### CAT-6 Blanket impl semver hazard

No findings.

No public `impl<T: ...> Trait for T` blanket impl pattern was found in Pass 2 scope.

---

### CAT-7 Large stack allocation

No findings.

Fixed-size buffers reviewed in Pass 2 are below the 64 KiB threshold, e.g. `let mut buf = [0u8; 8192];` in `crates/pwm-tui/tests/common/mod.rs`.

---

### Not found (Pass 2)

Clean categories: CAT-1 lifetime laundering, CAT-2 `std::sync::Mutex` across `.await`, CAT-3 Drop / RAII trap, CAT-5 async cancellation safety, CAT-6 blanket impl semver hazard, CAT-7 large stack allocation.

---

## Not found

Clean categories across combined Pass 1+2: CAT-1 lifetime laundering, CAT-2 `std::sync::Mutex` across `.await`, CAT-3 Drop / RAII trap, CAT-6 blanket impl semver hazard, CAT-7 large stack allocation.

---

## Notes / caveats

- CQDS `cq_files_ctl start_grep` was used for navigation/signals with `project_id: 5` and per-crate `path_prefix` scans. Pass 1 indexed `.rs` coverage: `pwm-core` 22 files, `pwmd` 86 files, `pwm-cli` 27 files, `pwm-tui` 19 files.
- Pass 2 covered 9 `.rs` files: `crates/pwmd/tests/lease_two_proc.rs`; 3 files under `crates/pwmd/src/bin/`; 1 file under `crates/pwmd/benches/`; `crates/pwm-cli/tests/cli_smoke.rs`; and 3 files under `crates/pwm-tui/tests/`.
- `crates/pwm-core/` has no crate-level `tests/` directory in this checkout; only `src/` was present for Pass 1.
- Candidate files were read for context before classifying findings. This is static analysis only; dynamic cancellation paths were not traced.
- `unsafe` audit covers lexical unsafe blocks found by CQDS; macro-generated unsafe may be missed.
- No source files were modified.
