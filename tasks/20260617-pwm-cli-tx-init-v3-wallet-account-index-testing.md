# Testing: pwm-cli tx-init v3 wallet `--index`

**Ticket:** `20260617-pwm-cli-tx-init-v3-wallet-account-index`  
**Agent:** pwm-testing  
**Date:** 2026-06-17  
**Verdict:** **PASS**

---

## Commands

| Command | Result |
|---|---|
| `bash tools/dev/preflight_target_debug.sh` | OK — `target/debug` 3442 MiB (threshold 4096 MiB), `removed: no` |
| `CARGO_TARGET_DIR=F:/pwm-test/pwm-protocol cargo test -p pwm-cli tx_init_` | **4 passed** |
| `CARGO_TARGET_DIR=F:/pwm-test/pwm-protocol cargo test -p pwm-cli` | **189 passed** (182 lib + 3 claim_ipv4_batch + 4 cli_smoke) |

## Acceptance criteria

| Criterion | Status |
|---|---|
| v3 signer = `m/0/--index` via `load_wallet_account_signer` | Covered by `tx_init_sel_wallet_idx` |
| `SignedTx.derivation_index` / `TxBody::Init.index` == `--index` | Covered by `tx_init_sel_wallet_idx` |
| `fetch_nonce_init_opt`: 404 → nonce 0 | Covered by new `init_nonce_404_none` (`parse_nonce_init_response` → `None`) |
| `build_init_activation` / `--save-activation-tx` for init account | Covered by `prepared_activation_roundtrip`, `tx_init_act_nonce_add1` |
| v2 single-account no regression | Implicit via `resolve_wallet_account` v2 path; no dedicated smoke |
| `docs/pwm-cli.md` §tx-init | Present (coding slice); minor doc nit on default account contrast remains |
| `cargo test -p pwm-cli` green | **PASS** |

## Changes (testing slice)

- **`crates/pwm-cli/src/tests/mod.rs`:** added `init_nonce_404_none` — closes review nit for 404 → init nonce 0.

## Open nits (non-blocking)

1. **Docs (low):** §tx-init could explicitly contrast `--index` with wallet CLI default (min `derivation_index`).
2. **UX (low):** no fail-fast when account already initialized (`Some((_, true))`).
3. **Future parity:** `try_auto_init` still hardcodes `nonce=0` vs `fetch_nonce_init_opt`.
4. **Optional tests:** negative `--index` not in wallet; v2 `--index 0` via `load_tx_init_source`.

## Checklist

No `docs/MVP-checklist.md` rows flipped (ticket anchors `docs/pwm-cli.md` §tx-init, not checklist §3–§6).

## Handoff

```
agent: pwm-testing
result: PASS
artifacts: tasks/20260617-pwm-cli-tx-init-v3-wallet-account-index-testing.md
          crates/pwm-cli/src/tests/mod.rs (init_nonce_404_none)
preflight_target_debug: 3442MiB / 4096MiB threshold, removed: no, script: preflight_target_debug.sh
```
