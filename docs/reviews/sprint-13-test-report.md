# Sprint 13 Test Report — Inter-Shard MVP Cut

Статус: `CLOSED` (Slice 0-7 documented; consolidated closeout completed)

## Slice 0 freeze baseline
- Execution freeze зафиксирован: ровно 8 slices (`0..7`), без scope expansion.
- Acceptance checkpoints заморожены для Sprint 13 и используются как единый gate.
- Out-of-scope lock подтверждён: без admission/advanced policy.
- Conveyor подтверждён: `pwm-coding -> pwm-testing -> pwm-review`.

## Slice 0-1 evidence (baseline + Slice 1 snapshot)
- Snapshot date: 2026-04-26.
- Scope: baseline compilation + targeted runtime-path tests (`pwmd`/`pwm-cli`/`pwm-tui`) без long matrix.

| Command | Result | Notes |
|---|---|---|
| `cargo check --workspace` | pass | Workspace baseline compiles on current branch. |
| `cargo test -p pwmd v1_tx_rejects_cross_shard_transfer_on_local_path -- --nocapture` | pass | `1 passed`; local tx path rejects cross-domain transfer by policy guard (expected before EXPORT/IMPORT). |
| `cargo test -p pwm-cli tx_path_recipient_policy_rejects_unknown_reserve_witness -- --nocapture` | pass | `1 passed`; CLI recipient policy guard intact on runtime path. |
| `cargo test -p pwm-tui validate_send_form_rejects_ambiguous_legacy_pretty_to_input -- --nocapture` | pass | `1 passed`; TUI input validation guard stable on runtime path. |

- Known behavior confirmed: cross-domain transfer on local path is rejected as expected by policy guard (`cross-domain transfer is disabled ... use explicit EXPORT/IMPORT flow`).
- Slice 1 status synced: `pwm-core` EXPORT + `export_id` unit evidence зафиксированы как closed для текущего sprint progress.
- Slice 2 status synced: `pwm-core` IMPORT path + replay guard (`imported_set`) + snapshot-restore replay test зафиксированы как closed.

## Test strategy
- Unit-level: `pwm-core` export/import state transitions и replay guard.
- Integration-level: `pwmd` API/runtime контракты для export/import.
- Operator-level: CLI/TUI сценарий inter-shard transfer (CY->DO).
- Negative: duplicate import, invalid proof, wrong-domain routing.

## Planned evidence matrix
| Slice | Проверка | Результат | Примечание |
|---|---|---|---|
| 0 | Design freeze + docs baseline sync | done | Freeze contract и runbook зафиксированы в review docs |
| 1 | EXPORT unit tests | done | `cargo test -p pwm-core export_ -- --nocapture` + `cargo test -p pwm-core export_id_is_stable_for_identical_tx_fields -- --nocapture` passed (Slice 1). |
| 2 | IMPORT + replay tests | done | `cargo test -p pwm-core import_ -- --nocapture` + `cargo test -p pwm-core snapshot_restore_keeps_import_replay_guard -- --nocapture` passed (Slice 2). |
| 3 | `pwmd` API/status tests | done | `cargo fmt`; `cargo check -p pwmd`; `cargo test -p pwmd v1_tx_ -- --nocapture` (включая `v1_tx_accepts_export`, `v1_tx_accepts_import_after_export`, duplicate/invalid/unknown import negatives, `v1_status_bridge_counters_grow_after_http_export_import`, `v1_tx_http_export_import_advances_head_height_via_sync_seal`). |
| 4 | CLI operator flow smoke | done | `cargo fmt`; `cargo check -p pwm-cli`; targeted CLI parsing tests passed (Slice 4 evidence). |
| 5 | TUI operator flow smoke | done | `pwm-tui` inter-shard UX/status baseline added (F7 route help + cross-domain send guard + status/error hint mapping). |
| 6 | 2-node e2e + negative | done | `cargo test -p pwmd v1_tx_two_node_smoke_cy_to_do_with_negative_suite -- --nocapture` passed: source-node `EXPORT` -> target-node `IMPORT`; duplicate import reject + unknown `export_id` reject verified. |
| 7 | Stabilization + consolidated closeout sanity set | done | `cargo check --workspace` + `cargo test -p pwmd v1_tx_two_node_smoke_cy_to_do_with_negative_suite -- --nocapture` passed; quartet docs synchronized without scope expansion. |

## Operator runbook (CY->DO e2e, planned verification)
1. Поднять/проверить 2-node стенд (`CY` source, `DO` destination) и зафиксировать стартовые балансы.
2. Выполнить cross-domain `TRANSFER` из `CY` в `DO` через целевой operator flow (pwmd + cli/tui).
3. Подтвердить, что на `CY` сформирован `EXPORT` с детерминированным `export_id` и source debit.
4. Подтвердить, что на `DO` `IMPORT` применился один раз и отражён credit.
5. Повторить `IMPORT` с тем же идентификатором и проверить детерминированный reject (duplicate guard).
6. Перезапустить узлы/восстановить snapshot и убедиться, что replay guard для уже импортированного `export_id` сохранился.
7. Сверить статусы/ошибки между `pwmd`, `pwm-cli`, `pwm-tui`; расхождения фиксировать как дефекты Sprint 13.

## Acceptance test checkpoints
- [x] `EXPORT` списывает source и формирует `export_id`.
- [x] `IMPORT` валидирует входной материал и зачисляет 1 раз.
- [x] Повторный `IMPORT` того же идентификатора отвергается.
- [x] После restart/snapshot restore replay-защита сохраняется (подтверждено Slice 2/3 snapshot/replay evidence).
- [x] CLI/TUI/pwmd дают согласованные статусы и ошибки.

## Slice 3 execution evidence (2026-04-27)
- `cargo fmt` -> pass.
- `cargo check -p pwmd` -> pass.
- `cargo test -p pwmd v1_tx_accepts_export -- --nocapture` -> pass.
- `cargo test -p pwmd v1_tx_accepts_import_after_export -- --nocapture` -> pass.
- `cargo test -p pwmd v1_tx_rejects_duplicate_import_with_conflict -- --nocapture` -> pass.
- `cargo test -p pwmd v1_tx_rejects_invalid_import_with_bad_request -- --nocapture` -> pass.
- `cargo test -p pwmd v1_status_bridge_counters_grow_after_http_export_import -- --nocapture` -> pass.
- `cargo test -p pwmd v1_tx_http_export_import_advances_head_height_via_sync_seal -- --nocapture` -> pass.

## Slice 4 execution evidence (2026-04-27)
- Scope: minimal CLI operator flow for inter-shard `EXPORT/IMPORT` (`pwm-cli` only, no scope expansion).
- Implemented CLI surface:
  - `pwm tx-export --wallet|--master+--domain --to --target-domain --amount --fee`
  - `pwm tx-import --wallet|--master+--domain --to --amount --export-id`
- Targeted tests:
  - `tx_export_cli_parsing_happy_path` -> pass.
  - `tx_import_cli_parsing_happy_path` -> pass.
  - `parse_export_id_hex_arg_rejects_non_hex_or_wrong_length` -> pass (negative).
- Command checks:
  - `cargo fmt` -> pass.
  - `cargo check -p pwm-cli` -> pass.
  - `cargo test -p pwm-cli tx_export_cli_parsing_happy_path` -> pass.
  - `cargo test -p pwm-cli tx_import_cli_parsing_happy_path` -> pass.
  - `cargo test -p pwm-cli parse_export_id_hex_arg_rejects_non_hex_or_wrong_length` -> pass.
- Note: HTTP reject message format remained aligned with existing tx submit path (`tx submit: HTTP <status> (<url>): <reason>`); existing commands were unchanged semantically (`tx-init`, `tx-send`, `tx-stake`, `tx-unstake`, `tx-burn-mark`).

## Slice 5 execution evidence (2026-04-27)
- Scope: minimal inter-shard UX/status in `pwm-tui` only (no scope expansion, no send-flow refactor).
- UX updates:
  - Footer hotkeys now include `F7 inter-shard->CLI`.
  - `F7` opens a compact operator hint modal with explicit route: source `tx-export` -> target `tx-import`.
  - `F6` send pre-submit guard blocks cross-domain `TRANSFER` and shows the same route hint instead of dispatching doomed submit.
- Status/error updates:
  - submit reject formatter now detects cross-domain policy reject (`cross-domain transfer is disabled ... EXPORT/IMPORT`) and appends inter-shard operator hint.
  - non-inter-shard errors keep existing generic format (no regression in normal send-flow error text style).
- Targeted tests:
  - `inter_shard_flow_cli_message_mentions_export_import_steps` -> pass (happy).
  - `format_submit_transfer_error_adds_inter_shard_hint_for_policy_reject` -> pass.
  - `format_submit_transfer_error_keeps_generic_for_other_failures` -> pass (negative).
  - `status_footer_line_rpc_offline_leads_then_poll_err` -> pass (footer regression guard with new F7 hint).
- Command checks:
  - `cargo fmt` -> pass.
  - `cargo check -p pwm-tui` -> pass.
  - `cargo test -p pwm-tui inter_shard_flow_cli_message_mentions_export_import_steps` -> pass.
  - `cargo test -p pwm-tui format_submit_transfer_error_adds_inter_shard_hint_for_policy_reject` -> pass.
  - `cargo test -p pwm-tui format_submit_transfer_error_keeps_generic_for_other_failures` -> pass.
  - `cargo test -p pwm-tui status_footer_line_rpc_offline_leads_then_poll_err` -> pass.

## Slice 6 execution evidence (2026-04-27)
- Scope: reproducible 2-node smoke `CY -> DO` + mandatory negative suite, strictly within existing EXPORT/IMPORT contract from Slice 3-5.
- Added test: `v1_tx_two_node_smoke_cy_to_do_with_negative_suite` in `crates/pwmd/src/lib.rs`.
  - Builds source/target nodes in test harness.
  - Uses `CY` sender domain for source `EXPORT` and `DO` target domain for destination `IMPORT`.
  - Performs operator handoff of recorded export provenance (`exported_registry`) from source to target as current contract baseline (no new protocol rules).
  - Verifies success path (`204 NO_CONTENT`) and one-time import application.
  - Verifies negatives:
    - duplicate import -> `409 CONFLICT` with `duplicate import`;
    - unknown provenance (`export_id` not known) -> `400 BAD_REQUEST`.
- Command checks:
  - `cargo fmt` -> pass.
  - `cargo check -p pwmd` -> pass.
  - `cargo test -p pwmd v1_tx_two_node_smoke_cy_to_do_with_negative_suite -- --nocapture` -> pass.

## Slice 7 execution evidence (2026-04-27)
- Scope: stabilization + consolidated closeout only (no new feature scope, no plan-file edits).
- Artifact sync completed:
  - `sprint-13-checklist.md`
  - `sprint-13-test-report.md`
  - `sprint-13-status-note.md`
  - `sprint-13-review-report.md`
- Final sanity checks:
  - `cargo check --workspace` -> pass.
  - `cargo test -p pwmd v1_tx_two_node_smoke_cy_to_do_with_negative_suite -- --nocapture` -> pass (`1 passed; 0 failed`).
- Closeout note: Sprint 13 evidence matrix полностью закрыта в рамках fixed scope `0..7`; residual items перенесены в post-cut notes как non-blocking.

## Slice 13.8 execution evidence (post-freeze extension, 2026-04-27)
- Scope: backend federated intent mempool + TTL + lock semantics (coding only, без расширения plan scope).
- Реализация покрыла:
  - dual pool (`pool` + `roaming_pool`);
  - lifecycle statuses (`queued/exported/relayed/imported/expired/failed`);
  - TTL expiry по `height`;
  - deterministic lock-reject конкурирующих tx;
  - API `POST /v1/roaming-intents`, `GET /v1/roaming-intents/:id`;
  - snapshot persistence/restore для intent+locks.
- Commands:
  - `cargo fmt` -> pass.
  - `cargo check -p pwm-core -p pwmd` -> pass.
  - `cargo test -p pwmd v1_roaming_intent_create_and_get_status -- --nocapture` -> pass.
  - `cargo test -p pwmd v1_roaming_intent_lock_blocks_competing_local_tx -- --nocapture` -> pass.
  - `cargo test -p pwmd v1_roaming_intent_expires_by_ttl_height -- --nocapture` -> pass.
  - `cargo test -p pwmd v1_roaming_intent_create_is_idempotent_for_duplicate_export_delivery -- --nocapture` -> pass.
  - `cargo test -p pwmd expires_after_ttl_height -- --nocapture` -> pass.
  - `cargo test -p pwmd v1_tx_rejects_duplicate_import_with_conflict -- --nocapture` -> pass.

## Slice 13.9 execution evidence (post-freeze extension, 2026-04-27)
- Scope: one-window CLI/TUI send flow поверх roaming-intent API (без backend scope expansion).
- Реализация покрыла:
  - CLI `tx-send` cross-domain -> roaming-intent create + lifecycle polling;
  - TUI `F6` cross-domain -> roaming-intent submit + lifecycle статус в форме/истории;
  - deterministic error UX на duplicate/invalid/expired;
  - fallback/debug команды `tx-export`/`tx-import` сохранены.
- Commands:
  - `cargo test -p pwm-cli roaming_intent_error_maps_duplicate_conflict roaming_intent_error_maps_invalid_request -- --nocapture` -> pass.
  - `cargo test -p pwm-tui format_roaming_error_maps_duplicate_invalid_and_expired submit_done_updates_history_even_when_form_closed -- --nocapture` -> pass.

