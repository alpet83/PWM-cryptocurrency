# Review: pwm-cli tx-init v3 wallet `--index` (20260617)

**Ticket:** `20260617-pwm-cli-tx-init-v3-wallet-account-index`  
**Slice:** `crates/pwm-cli/src/cmd_tx.rs`, `docs/pwm-cli.md`  
**Reviewer:** pwm-review  
**Date:** 2026-06-17

---

## 1. Scope recap

Operator-reported bug: `pwm tx-init --wallet <v3-multi-account> --index N` signed with the wallet **default** signer (min `derivation_index` in `accounts[]`), sent `nonce=0` for the wrong account, while `TxBody::Init.index` carried `N`. Result: HTTP 409 bad nonce on the default account and no init on the target address.

Slice goal (ticket + `docs/pwm-cli.md` §tx-init, runbook soak dependency): align manual `tx-init` / `--save-activation-tx` with `addr-bruteforce` auto-init semantics — signer, body index, and init nonce must all refer to the `--index` account.

---

## 2. Requirements fit

| Acceptance criterion | Status | Notes |
|---|---|---|
| v3 `--wallet`: signer = `m/0/--index` via `load_wallet_account_signer`; clear error if missing | **Met** | New `load_tx_init_source` routes wallet path to `load_wallet_account_signer(&path, index, …)`. Missing account surfaces existing `resolve_wallet_account` message (`wallet account m/0/{index} not found…`). |
| `SignedTx.derivation_index == --index`; `TxBody::Init.index == --index` (parity with `try_auto_init`) | **Met** | `sign_body(..., source.idx, …, TxBody::Init { index, flags })` with `source.idx == index` from `load_wallet_account_signer`. Matches `try_auto_init` signer/body-index pairing. |
| Nonce via `fetch_nonce_init_opt`: 404 → 0; chain nonce if stub/uninit; AlreadyInit if initialized | **Mostly met** | `fetch_nonce_init_opt` → `None` → `unwrap_or(0)`; `Some((nonce, _))` → chain nonce. `initialized` flag is **ignored** before submit — already-initialized accounts rely on node `TxError::AlreadyInit` (acceptable deferral, weaker UX). |
| `build_init_activation` / `--save-activation-tx`: target + cosign for init account | **Met** | `target_account: source.from` (index-selected signer). Activation nonce `calc_activation_nonce(init_nonce)` (`init_nonce + 1`) replaces hardcoded `1`; test updated. |
| v2 single-account: no regression | **Met** | `resolve_wallet_account` v2 fallback when `account_index == wallet.derivation_index`. `--index 0` default unchanged. `--master` still uses `load_tx_signer_source` (pre-existing dev path). |
| Unit tests: multi-account signer; mocked RPC 404 → nonce 0 | **Partial** | `tx_init_sel_wallet_idx` covers v3 signer selection. **No** test for 404 → nonce 0 (nor direct `parse_nonce_init_response` exercise in this slice). |
| `docs/pwm-cli.md`: `--index` = signer + Init body; not wallet default | **Mostly met** | `--index` and Signing flow updated. Does not explicitly contrast with CLI “default account” (min `derivation_index` from `wallet account list`). |
| `cargo test -p pwm-cli` green | **Met** | `tx_init_*` tests pass under `CARGO_TARGET_DIR=F:/pwm-test/PWM-cryptocurrency`. |

**Root-cause fix:** Confirmed — wallet-path `tx-init` no longer calls `load_tx_signer_source` (default signer); it selects by `--index`.

**Out-of-slice note:** `try_auto_init` in `cmd_addr.rs` still hardcodes `nonce=0` and does not call `fetch_nonce_init_opt`. Manual `tx-init` is now **stricter** than auto-init for stub/uninit re-init; acceptable per ticket AC (explicit `fetch_nonce_init_opt` requirement) but leaves a future parity nit on auto-init.

---

## 3. Style and module shape

- New helpers `load_tx_init_source`, `calc_activation_nonce` fit existing `cmd_tx.rs` patterns; extraction from `run_tx_init` improves testability (`load_tx_init_source` exercised in unit test).
- `python scripts/check_entity_name_segments.py crates/pwm-cli/src/cmd_tx.rs` → **no violations** (prod ≤4 words, tests ≤5).
- Module `//!` banner present at file top.
- Test names `tx_init_sel_wallet_idx`, `tx_init_act_nonce_add1` within test budget.

**Doc diff scope:** `docs/pwm-cli.md` also documents `addr-bruteforce` offline / `--max-try` resume — unrelated to ticket but accurate; no harm.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

---

## 4. Safety

- No new trust boundaries. Signer resolution still local wallet unlock; RPC nonce read unchanged helper.
- `calc_activation_nonce` uses `saturating_add(1)` — safe for `u64::MAX` edge (degenerate).
- Error paths remain `exit_user_error` / `Result` — no new panics in hot path.
- Selecting wrong account was the operational safety bug; fix reduces mis-signed init risk.

---

## 5. Tests

**Present**

- `tx_init_sel_wallet_idx` — v3 wallet with two accounts; asserts `source.idx == sel_idx` and derived id matches `source.from`.
- `tx_init_act_nonce_add1` — activation nonce offset.
- `prepared_activation_roundtrip` — updated for `init_nonce` parameter; asserts activation `nonce == 1` when init nonce is 0.

**Missing (ticket AC)**

- Mocked / parsed RPC **404 account not found → init nonce 0** (e.g. unit test on `parse_nonce_init_response` or thin wrapper around `fetch_nonce_init_opt` logic). Logic exists in `rpc_helpers.rs` but is **untested** in-repo and not wired in this slice’s new tests.
- Optional: negative test — `--index` not in wallet returns expected error string.
- Optional: v2 wallet `--index 0` smoke via `load_tx_init_source` (regression guard).

Recommend `pwm-testing` add 404-nonce test before closing ticket, or coding nit follow-up.

---

## 6. Verdict

**PASS_WITH_NITS**

### Prioritized nits (non-blocking)

1. **Medium — AC test gap:** Add unit test for init nonce when account lookup returns 404 (`parse_nonce_init_response` → `None` → effective nonce `0` in `run_tx_init` path).
2. **Low — Docs:** In §`tx-init`, one sentence that `--index` is **not** the wallet CLI default (min `derivation_index`); operators must pass the target account’s derivation index explicitly.
3. **Low — UX:** Consider failing fast when `fetch_nonce_init_opt` returns `Some((_, true))` with a clear “already initialized” message instead of posting Init and relying on node reject.
4. **Low — Future parity:** `try_auto_init` still uses hardcoded `nonce=0`; align in a follow-up if stub re-init scenarios matter for auto-init.

---

## 7. Participation / token estimate

```
agent: pwm-review
result: PASS_WITH_NITS
artifacts: docs/reviews/20260617-pwm-cli-tx-init-v3-wallet-account-index-review.md
token_usage: { "source": "estimate", "input": null, "output": null, "total": 28000, "confidence": "medium" }
```

**Verdict:** PASS_WITH_NITS — core fix correct and addresses operator blocker; add 404→nonce unit test and minor doc clarification before ticket closeout.
