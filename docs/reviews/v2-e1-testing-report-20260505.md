# Testing gate report — MVP v2 E-1 (core claim/autoclaim, burn purpose, import fee)

**Date:** 2026-05-05  
**Ticket:** `20260505-v2-e1-core-claim-autoclaim`  
**Agent:** pwm-testing  

## Verdict

**PASS** (**initial gate**, до fix-итерации по review): автоматические прогоны и выравнивание фикстур по v2. Актуальные счётчики и проверки контекста apply — в § **Re-test** и **Final verdict** ниже.

Automated gates for `pwm-core` and `pwmd` completed successfully after aligning integration/snapshot fixtures with v2 rules (`Account` claim-related fields, import fee floor debited from importer balance, replay-safe snapshot blocks).

_Table below — артефакт первичного отчёта; числа тестов устарели относительно re-test (**96**/**233**/… — см. § Re-test)._

## Commands

| Command | Result | Notes |
|--------|--------|--------|
| `cargo fmt --check` | PASS | Repo root |
| `cargo test -p pwm-core --no-fail-fast` | PASS | 95 tests, ~28 s |
| `cargo test -p pwmd --no-fail-fast` | PASS | 230 lib + 3 bin tests, ~35 s |
| `cargo bench -p pwmd --bench snapshot_load --no-run` | PASS | Harness compile only |
| `tools/dev/preflight_target_debug.ps1` | SKIPPED | Parser error on host PowerShell 5.1; `pwsh` not on PATH |
| `preflight_target_debug.sh` | SKIPPED | Git Bash/WSL path not used this session |

Hang watchdog: not triggered.

## Coverage vs E-1 (targeted behaviour)

### pwm-core — claim / auto-claim / marks

- `state::tests::claim_tx_materializes_marks` — happy-path claim → marks + anchor continuity.
- `state::tests::stake_autoclaim_noop_when_zero_matured` — auto-claim path does not fire without matured units.
- `chain::seal` exercised indirectly via existing chain tests (no new regressions).

### pwm-core — import fee floor (`MIN_IMPORT_FEE_UNITS`)

- `tx::tests::import_fee_rejects_below_minimum` — `validate_tx_shape`.
- `state::tests::import_min_fee_rule_enforced` — apply-time rejection + state cleanliness.

### pwm-core — burn purpose (`BurnMark`)

- `tx::tests::burn_purpose_rejects_control_chars` — structural validation.
- `tx::tests::fee_zero_for_burn_mark` — fee semantics for burn.
- `state::tests::burn_*` — burn application / quota paths unchanged at gate level.

### pwmd — HTTP / snapshot integration

- Multiple `http_export::*` flows now credit importers where needed (`credit_min_import_fee_for_tests`) so `Import` debit matches production rules.
- Snapshot round-trip tests use **on-chain** `Transfer` from dev validator to importer where replay must reproduce state (`snap_rt_imp_guard_pv`, `snap_rt_handoff_import_ok`), avoiding non-replayable in-memory balance hacks between seals.

## Harness / test-only changes (not product code)

| Area | Change |
|------|--------|
| `crates/pwmd/src/tests/helpers.rs` | `credit_min_import_fee_for_tests`; `seed_handoff_provenance_for_import` credits importer post-init |
| `crates/pwmd/src/tests/http_export.rs` | `Account { ..Default::default() }` for new fields; explicit credits / target funding for import fee |
| `crates/pwmd/src/tests/snapshot_roaming.rs` | Replay-safe funding via `Transfer` + adjusted validator nonces |

## Gaps / follow-ups

- **Preflight disk guard:** not executed on this Windows host (script/`pwsh` issues); recommend owner runs `preflight_target_debug.sh` from Git Bash or fixes PS1 encoding for PowerShell 5.1.
- **Dedicated pwmd RPC tests for `ClaimTx`:** not added in this gate; behaviour covered primarily in `pwm-core`. Consider thin HTTP/Router negative tests if API surface grows.
- **`fee_pool` observable via pwmd HTTP:** not asserted here (core state tests own economics).

## Risks

None blocking for E-1 coding slice after harness alignment; production crates were not modified.

## snapshot_benches

`compiled_only` — PASS (`cargo bench -p pwmd --bench snapshot_load --no-run`) — reaffirmed after re-test (see below).

## Re-test (post fix-iteration E-1, 2026-05-05)

**Scope:** confirm pwm-coding fix for review blockers — tip-aware **`precheck_apply`** context, **`apply_tx_with_ctx`** in snapshot **replay**/**repair**/ClickHouse paths, **`lifecycle`** seal-skip diagnostics — plus regression coverage for claim/autoclaim/import-fee and safe JSON errors in **`handlers_tx`**.

### Commands

| Command | Result | Notes |
|---------|--------|--------|
| `cargo fmt --check` | PASS | Repo root (same session as `pwm-core` test) |
| `cargo test -p pwm-core --no-fail-fast` | PASS | **96** tests (~26 s) |
| `cargo test -p pwmd --no-fail-fast` | PASS | **233** lib + **3** bin (~34 s) |
| `cargo test -p pwmd --features clickhouse-snapshot --no-fail-fast` | PASS | **237** lib + **3** bin (~36 s); extra CH/replay tests |
| `cargo bench -p pwmd --bench snapshot_load --no-run` | PASS | Harness compile only |
| `tools/dev/preflight_target_debug.ps1` | FAIL | PowerShell 5.1 parser/encoding on `—` in script (known host issue); `pwsh` not on PATH |

Hang watchdog: not triggered.

### Regression anchors (ctx / replay)

- `state::tests::precheck_tip_uses_next_height_ctx` — `pwm-core` precheck uses next-seal height context.
- `snapshot::io::tests::validate_snapshot_replay_uses_block_ctx` — snapshot validation replay uses block height/ts.
- `snapshot::repair::tests::repair_replay_uses_block_ctx` — repair replay uses block context.
- `lifecycle::tests::seal_skip_ctx_uses_block_height` — seal-skip path uses block height for apply context.
- With `--features clickhouse-snapshot`: `snapshot::ch_http::tests::replay_state_at_uses_block_ctx`, `tests::snapshot_backend_replay::snap_ch_wire_jsonfile_mock`.

**Product code:** not modified in this re-test pass.

## Final verdict

**PASS** — After the fix-iteration, automated gates remain green: `pwm-core` **96/96**, `pwmd` **233+3** (default), `pwmd` **237+3** with `clickhouse-snapshot`, **`cargo fmt --check`**, snapshot bench harness **`--no-run`**. Review blockers around **wrong apply height/ts** on precheck vs seal and on replay/repair/CH/lifecycle are covered by the new/updated tests above. **Preflight** disk guard still not validated on this Windows host (same PS 5.1 / `pwsh` limitation as initial gate).

## Cleanup

No long-lived `pwmd` / `pwm-tui` processes spawned for this gate (`cleaned: yes`).
