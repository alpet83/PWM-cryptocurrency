# Review: V7-8 S2 journal writer + S1 nits (159ce82)

- date: 2026-06-29
- ticket: `20260629-v7-8-s2-journal-writer-review`
- coding_ticket: `20260629-v7-8-s2-journal-writer`
- commit: `159ce82`
- scope: `crates/pwm-tui/src/journal.rs`, `config.rs` (S1 tests), `lib.rs`, `tui_loop.rs` (F5/F6 wiring)

## 1. Scope recap

V7-8 Slice 2 adds append-only JSONL wallet journal:

| deliverable | location |
|-------------|----------|
| `JournalEntry` + `append_tx` | `journal.rs` |
| `make_journal_filename` | `journal.rs:66-119` |
| F5 burn / F6 send wiring | `tui_loop.rs:141-151`, `424-446`, `538-556` |
| S1 nit tests | `config.rs:214-236` |
| Public API | `lib.rs:7-19` |

TUI-only; no `pwmd` / `pwm-core` changes.

## 2. Focus-area verification

| # | Focus | Verdict | Evidence |
|---|-------|---------|----------|
| 1 | `append_tx` append+create, no truncation | **PASS** | `OpenOptions::new().create(true).append(true)` (`journal.rs:57-60`); `writeln!` appends one JSON line. No `truncate(true)` or overwrite mode. |
| 2 | `make_journal_filename` / uniqueness | **PASS** with nit | Derives stem from `account_id_to_human` via `pretty_from_hex` → `stem_from_pretty` (strip `-t{tail}`) → `sanitize_stem` (`journal.rs:70-118`). Test: `CY-7E-f00000000.jsonl` (`:127-130`). **Nit:** filename is unique per **domain+flags prefix**, not per full `AccountId` — two wallet accounts sharing `pwm1-{domain}-f{flags}` collapse to one `.jsonl`; `JournalEntry` has no `from` field to disambiguate. |
| 3 | F6 send wiring | **PASS** with nit | `pending_journal` inserted on confirm when `journal_dir.is_some()` (`:538-556`); `SubmitDone` writes only if `result.is_ok()` (`:146-150`). Fields: `kind=send`, `to=account_id_to_human(&to)`, `amount`/`fee` via `format_amount_compact`, `nonce` from owner row. **Nit:** entry keeps `status:"pending"` even after successful RPC — never upgraded to `ok`/`sealed`. |
| 4 | F5 burn wiring | **PASS** | Same gating; `kind=burn`, `to` from beneficiary human, `amount` = mark amount, `fee=0`, `nonce` from owner (`:424-446`). Burn `SubmitDone` path clears inflight before history handler (`:153-161`). |
| 5 | S1 nit tests | **PASS** | `wallet_dir_named_json` (`config.rs:214-224`), `init_history_creates_dir` (`:226-236`). Journal tests: `filename_from_hex`, `append_tx_jsonl` (`journal.rs:126-158`). `cargo test` not run in sandbox. |
| 6 | No write when `wallet_dir` None | **PASS** | `journal_dir = wallet_dir(&args)` (`tui_loop.rs:71`); F5/F6 journal insert guarded by `journal_dir.is_some()`; append guarded by `journal_dir.as_deref()` on success. Dev/fallback mode skips journal entirely. |
| 7 | `cargo check -p pwm-tui` | **PARTIAL** | Sandbox terminal unavailable; static compile path looks sound (`TX_HISTORY_DIR` re-exported `lib.rs:18`, `journal.rs` imports `crate::TX_HISTORY_DIR`). |

## 3. Submit gating analysis (blocker check)

Flow:

1. User confirms F5/F6 → `pending_journal.insert(req_id, …)` (pre-allocates entry with `status: pending`).
2. RPC worker returns `SubmitDone { req_id, result }`.
3. Main loop: `pending_journal.remove(req_id)`; **write only when `result.is_ok()`** (`tui_loop.rs:146-150`).

Failed submissions: pending entry removed, **no file write** — satisfies ticket blocker (“no journal on failed submissions”).

**No truncation risk:** append mode only.

**Silent I/O failure:** `let _ = append_tx(...)` discards errors after successful RPC — acceptable for S2; Slice 3+ may surface to operator.

## 4. Style and module shape

New `journal.rs` module; helpers ≤4 word names. `PendingJournal` struct local to `tui_loop.rs` — reasonable.

### Wire JSON / u128

Wire JSON / u128: not applicable (local JSONL wallet journal only).

## 5. Safety

- Journal paths confined to `wallet_dir/tx-history/` — no path traversal from user input (filename from derived stem).
- Amounts serialized as display strings, not raw u128 — consistent with operator-readable journal.

## 6. Tests

| test | coverage |
|------|----------|
| `wallet_dir_named_json` | S1 nit |
| `init_history_creates_dir` | S1 nit |
| `filename_from_hex` | stem derivation |
| `append_tx_jsonl` | append + JSONL line |
| F5/F6 gating integration | **none** (manual/trace only) |
| Filename collision (same flags, different tail) | **none** |

## 7. Concurrency / parallelism

RPC worker thread submits txs; journal `append_tx` runs only on the main TUI loop when processing `SubmitDone` — no concurrent writes from multiple threads. `pending_journal` is main-loop-local `HashMap` — no shared mutex. **No hazards found.**

## 8. BLOCKERs

None. Append mode prevents truncation; journal is not written on failed `SubmitDone`.

## 9. Nits (non-blocking)

1. **NIT-1:** `JournalEntry.status` remains `"pending"` on successful write — update to `"submitted"` (or similar) before `append_tx` on Ok.
2. **NIT-2:** Document or extend filename scheme if multi-account wallets can share `pwm1-{domain}-f{flags}` stem (add `-t{tail}` suffix or `from` field in JSONL).
3. **NIT-3:** Log or surface `append_tx` I/O errors instead of `let _ =`.
4. **NIT-4:** Integration test for `SubmitDone` Err → no jsonl line appended.

## 10. Verdict

**Approve with nits** — append-only journal writer and F5/F6 success-gated wiring meet V7-8 S2 spec; S1 nit tests present; dev mode correctly skips journal. Filename stem collision and stale `status:"pending"` on success are design/doc nits, not merge blockers.

## 11. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260629-v7-8-s2-journal-writer-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 28000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260629-v7-8-s2-journal-writer-review.md'
git commit -m 'docs(v7-8/s2): journal writer review (159ce82)'
```