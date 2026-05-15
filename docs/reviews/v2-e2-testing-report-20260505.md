# MVP v2 E-2 — testing gate (pwm-testing)

**Date:** 2026-05-05  
**Ticket:** `tasks/20260505-v2-e2-api-preflight-parity.json`  
**Scope:** Independent gate after pwm-coding — stable reject wire, `E_*` → `response_class` mapping, preflight/apply parity in `crates/pwmd/src/tests/http_status.rs`.

## Verdict

**PASS**

## Commands (host: Windows, repo `P:\opt\docker\PWM-cryptocurrency`)

| Command | Result | Notes |
|--------|--------|--------|
| `cargo fmt --check` | PASS | Exit 0 |
| `cargo test -p pwm-core` | PASS | 96 passed; 0 failed; 0 ignored |
| `cargo test -p pwmd` | PASS | 236 (lib) + 3 (main) passed; 0 failed; 0 ignored |

**Hang watchdog:** not triggered.  
**CQDS:** not used; builds were incremental (fast `Finished test` — no full rebuild).

## E-2 parity / reject-wire coverage (`http_status.rs`)

Exercised by integration-style tests via `assert_preflight_apply_parity`:

| Test | HTTP | `error.code` | `response_class` | `tx_kind` |
|------|------|--------------|------------------|-----------|
| `v1_tx_parity_burn_purpose_invalid` | 400 | `E_BURN_SCHEMA_INVALID` | `VALIDATION_ERROR` | `burn` |
| `v1_tx_parity_claim_daily_limit` | 400 | `E_FREE_CLAIM_DAILY_LIMIT` | `POLICY_REJECT` | `claim` |
| `v1_tx_parity_import_fee_too_low` | 400 | `E_IMPORT_FEE_TOO_LOW` | `POLICY_REJECT` | `import` |

Checks include: `ok: false`, `phase` preflight vs apply, matching `error.code` / `response_class` / `tx_kind`, non-empty matching `error.trace_id` across preflight JSON and synthetic apply JSON.

## Preflight / disk (`target/debug`)

Per `docs/AGENT_PROMPT_testing.md`, size guard scripts were **not** run in this session because the matrix completed quickly against an existing `rust-target-shared` debug tree. If `bash tools/dev/preflight_target_debug.sh` is unavailable on **PowerShell 5.1**, use `pwsh -NoProfile -ExecutionPolicy Bypass -File tools/dev/preflight_target_debug.ps1` before heavy `cargo build` / full `cargo test` runs.

## Artifacts

- This report: `docs/reviews/v2-e2-testing-report-20260505.md`

## Open risks

- None observed for E-2 gate; pwm-review remains on the ticket.
