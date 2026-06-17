# Testing: pwm-cli wallet v3 single-file `--index` on tx commands

**Ticket:** `20260620-pwm-cli-wallet-v3-single-file-tx-index`  
**Agent:** pwm-testing  
**Date:** 2026-06-17  
**Verdict:** **PASS**

---

## Commands

| Command | Result |
|---|---|
| `bash tools/dev/preflight_target_debug.sh` | OK — `target/debug` 3774 MiB (threshold 4096 MiB), `removed: no` |
| `CARGO_TARGET_DIR=F:/pwm-test/PWM-cryptocurrency cargo test -p pwm-cli tx_cmd_idx_parse` | **1 passed** |
| `CARGO_TARGET_DIR=F:/pwm-test/PWM-cryptocurrency cargo test -p pwm-cli tx_v2_idx_fallback` | **1 passed** |
| `CARGO_TARGET_DIR=F:/pwm-test/PWM-cryptocurrency cargo test -p pwm-cli tx_pol_nonce_409` | **1 passed** |
| `CARGO_TARGET_DIR=F:/pwm-test/PWM-cryptocurrency cargo test -p pwm-cli tx_pol_act_sw_idx` | **1 passed** |
| `CARGO_TARGET_DIR=F:/pwm-test/PWM-cryptocurrency cargo test -p pwm-cli tx_init_wallet_idx` | **1 passed** |
| `CARGO_TARGET_DIR=F:/pwm-test/PWM-cryptocurrency cargo test -p pwm-cli` | **195 passed** (188 lib + 3 claim_ipv4_batch + 4 cli_smoke) |

## Focus tests (review nit fix included)

| Requested name | Actual `fn` | Module | What it verifies |
|---|---|---|---|
| `tx_cmd_idx_parse` | `tx_cmd_idx_parse` | `tests::` | Clap `--index` on tx-send, tx-burn-mark, tx-stake, tx-unstake, tx-policy-set, tx-policy-deactivate |
| `tx_v2_idx_fallback` | `tx_v2_idx_fallback` | `cmd_tx::tests::` | v2 wallet: omitted `--index` (clap default 0) → `load_tx_wallet_signer` uses `wallet.derivation_index` |
| `tx_pol_nonce_hint_409` | `tx_pol_nonce_409` | `cmd_tx::tests::` | `enrich_act_nonce_err` on HTTP 409 bad nonce: file vs on-chain nonce + live `--index` / `--rescue-account-index` hint |
| `tx_pol_act_sw_idx` | `tx_pol_act_sw_idx` | `tests::` | Same-wallet emergency `tx-policy-activate` parse: `--index`, `--rescue-account-index`, `routing.emergency_redirect` |
| `tx_init_sel_wallet_idx` | `tx_init_wallet_idx` | `cmd_tx::tests::` | v3 multi-account wallet: `load_tx_wallet_signer` selects `m/0/N` signer |

## Acceptance criteria

| Criterion | Status |
|---|---|
| `--index` on tx-policy-activate/set/deactivate; v2 no regression | **Met** — `tx_cmd_idx_parse`, `tx_v2_idx_fallback` |
| `--index` on tx-send/stake/unstake/burn-mark via `load_tx_wallet_signer` | **Met** — `tx_cmd_idx_parse` |
| `--activation-tx` stale nonce → rich stderr hint | **Met** — `tx_pol_nonce_409`, `tx_pol_nonce_detect` |
| Live same-wallet emergency activate (unit/smoke) | **Partial** — parse-only `tx_pol_act_sw_idx`; full cosign build deferred to operator soak (runbook §7b) |
| `docs/pwm-cli.md` table + emergency example | **Met** (coding nit fix) — per ticket `notes`; not re-audited line-by-line |
| Runbook primary = one wallet v3 | **Met** (coding/review); not re-run live |
| Unit tests parse + signer + 409 | **Met** |
| `cargo test -p pwm-cli` green | **PASS** |

## Changes (testing slice)

No production or test edits — verification only.

## Open nits (non-blocking)

1. **AC #4 gap:** no automated smoke that builds and signs live emergency activation with rescue cosign from one wallet file (operator gate in ticket).
2. **Docs (low):** review noted no consolidated `--index` command table under «Карта команд» — optional polish.

## Checklist

No `docs/MVP-checklist.md` rows flipped (ticket is CLI operator UX, not checklist §3–§6).

## Handoff

```
agent: pwm-testing
result: PASS
artifacts: tasks/20260620-pwm-cli-wallet-v3-single-file-tx-index-testing.md
preflight_target_debug: 3774MiB / 4096MiB threshold, removed: no, script: preflight_target_debug.sh
cargo: CARGO_TARGET_DIR=F:/pwm-test/PWM-cryptocurrency, 195 passed, 0 failed
```
