# Review: V7-8 S1 wallet dir detection + tx-history init (87f67db)

- date: 2026-06-29
- ticket: `20260629-v7-8-s1-wallet-dir-review`
- coding_ticket: `20260629-v7-8-s1-wallet-dir`
- commit: `87f67db`
- scope: `crates/pwm-tui/src/config.rs`, `lib.rs`, `main.rs` (TUI-only)

## 1. Scope recap

V7-8 Slice 1 adds wallet-directory resolution and `tx-history/` subdir creation at TUI startup:

| deliverable | location |
|-------------|----------|
| `resolve_wallet_dir` | `config.rs:32-40` |
| `resolve_wallet_file` (normalization) | `config.rs:42-50` |
| `init_tx_history_dir` | `config.rs:52-57` |
| `wallet_dir(args)` helper | `config.rs:59-61` |
| Startup wiring | `main.rs:11-14` |
| Public exports | `lib.rs:17` |

No `pwmd` / `pwm-core` changes in slice scope.

## 2. Focus-area verification

| # | Focus | Verdict | Evidence |
|---|-------|---------|----------|
| 1 | `resolve_wallet_dir` four cases | **PASS** with nit | **File → parent:** `is_file()` branch (`:33-35`). **Dir + `wallet.json`:** `wallet_file_in_dir` + `is_dir` chain (`:36-39`, `:64-66`). **Dir + `{name}.json`:** `wallet_file_in_dir` stem fallback (`:68-70`). **No wallet / invalid dir:** `None` when not file, not dir, or dir lacks wallet file. **Dev mode:** `args.wallet == None` → `wallet_dir` / `init_tx_history_dir` return early. |
| 2 | `tx-history` creation, soft failure | **PASS** | `init_tx_history_dir` uses `let _ = std::fs::create_dir_all(...)` (`:56`) — no panic on error; missing `wallet_dir` is no-op (`:53-55`). |
| 3 | `wallet_dir` exposed for Slice 2 | **PASS** | `pub fn wallet_dir(args: &Args)` + `pub use config::{..., wallet_dir, Args}` (`lib.rs:17`). `tui_loop` not wired yet — expected for S1. |
| 4 | Unit tests file + dir paths | **PASS** with nit | `wallet_dir_file_parent`, `wallet_dir_contains_json` (`config.rs:191-211`). **Gap:** no test for `{dirname}.json` naming convention. |
| 5 | Dev mode regression (no `--wallet`) | **PASS** | `main.rs:8-10` optional wallet; `init_tx_history_dir(None)` no-op; `choose_identity` → `SeedFallback` when `args.wallet` absent (`wallet.rs:540-542`). Existing fallback banner path unchanged. |
| 6 | `cargo check` / tests PASS | **PARTIAL** | Sandbox terminal unavailable; static review only. Unit tests are straightforward temp-dir I/O; recommend orchestrator run `cargo check -p pwm-tui && cargo test -p pwm-tui`. |

## 3. Startup flow analysis

`main.rs` sequence:

1. Optional `default_wallet_if_present()` fill.
2. Normalize `args.wallet` to concrete wallet **file** when `resolve_wallet_file` succeeds.
3. `init_tx_history_dir(args.wallet.as_deref())` — resolves **parent dir** from normalized file path (or from dir path if normalization failed but dir contains wallet file).

This matches the ticket convention: journal root is the wallet directory, not the JSON file itself.

**Edge behavior (acceptable):** `--wallet` pointing to a directory without `wallet.json` or `{dirname}.json` yields `resolve_wallet_dir == None` — no `tx-history/` created. Wallet load may still fail later; not a new panic surface from this slice.

## 4. Style and module shape

Helpers live in existing `config.rs` module; names ≤4 segments (`resolve_wallet_dir`, `init_tx_history_dir`, `wallet_file_in_dir`). Production paths use `Option` — no hot-path `unwrap`.

### Wire JSON / u128

Wire JSON / u128: not applicable (TUI-local filesystem paths only).

## 5. Safety

- No secrets logged; paths derived from user-supplied `--wallet` / `PWM_TUI_WALLET`.
- Silent `create_dir_all` failure could hide permission errors — acceptable for S1 init (Slice 2 journal writes will surface I/O errors).

## 6. Tests

| case | covered |
|------|---------|
| File path → parent dir | yes |
| Dir containing `wallet.json` | yes |
| Dir containing `{dirname}.json` | **no** |
| `init_tx_history_dir` creates subdir | **no** |
| Dev mode (`None`) skips creation | implicit via code path only |

## 7. Concurrency / parallelism

Concurrency / parallelism: not in diff scope (single-threaded startup `create_dir_all`).

## 8. BLOCKERs

None. `wallet_dir` is publicly accessible; `tx-history` init does not panic on failure.

## 9. Nits (non-blocking)

1. **NIT-1:** Add unit test `wallet_dir_named_json` — dir `foo/` with `foo.json` only.
2. **NIT-2:** Add test that `init_tx_history_dir` creates `tx-history/` under resolved dir.
3. **NIT-3:** Re-export `TX_HISTORY_DIR` from `lib.rs` if Slice 2 journal code should avoid string literal duplication.
4. **NIT-4:** Document in module comment that `default.yml` auto-wallet resolves tx-history to CWD parent (pre-existing `default_wallet_if_present` interaction).

## 10. Verdict

**Approve with nits** — wallet dir resolution and soft `tx-history/` init match V7-8 S1 spec; public `wallet_dir(args)` API ready for Slice 2; dev-mode path preserved. Missing `{dirname}.json` test and unverified `cargo test` in review sandbox are non-blocking.

## 11. Participation

- `agent`: `pwm-review`
- `result`: `PASS`
- `artifacts`: `docs/reviews/20260629-v7-8-s1-wallet-dir-review.md`
- `token_usage`: `{ "source": "estimate", "input": null, "output": null, "total": 24000, "confidence": "medium" }`

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260629-v7-8-s1-wallet-dir-review.md'
git commit -m 'docs(v7-8/s1): wallet dir + tx-history init review (87f67db)'
```