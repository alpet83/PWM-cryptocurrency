# Sprint 13 Checklist — Inter-Shard MVP Cut (EXPORT/IMPORT)

Статус: `CLOSED` (execution freeze соблюден, Slice 0..7 closed)

## Execution freeze (Sprint 13)
- [x] Fixed plan: ровно 8 slices (`0..7`) без добавления новых slices.
- [x] Acceptance criteria для Sprint 13 зафиксированы и не расширяются вне inter-shard MVP.
- [x] Out-of-scope lock: без admission/advanced policy и без расширенного policy surface.
- [x] Conveyor делегирования зафиксирован: `pwm-coding -> pwm-testing -> pwm-review`.

## Scope lock (обязательный)
- [x] Зафиксирован fixed scope Sprint 13 (8 slices, без расширения в admission/advanced policy).
- [x] Зафиксированы acceptance критерии inter-shard MVP базиса.
- [x] Подтверждён контур делегирования: `pwm-coding -> pwm-testing -> pwm-review`.

## Slices
- [x] Slice 0 — design freeze + bootstrap артефактов + out-of-scope list.
- [x] Slice 1 — `pwm-core` EXPORT + `export_id` + unit tests.
- [x] Slice 2 — `pwm-core` IMPORT + replay guard + snapshot restore tests.
- [x] Slice 3 — `pwmd` API/runtime wiring export/import + error/status contract.
- [x] Slice 4 — `pwm-cli` minimal operator commands/flow.
- [x] Slice 5 — `pwm-tui` minimal inter-shard UX/status.
- [x] Slice 6 — e2e smoke 2-node (CY->DO) + negative suite.
- [x] Slice 7 — stabilization + consolidated closeout.

## Acceptance gates
- [x] Cross-domain `TRANSFER` даёт согласованный маршрут через EXPORT/IMPORT.
- [x] `EXPORT` фиксирует источник списания и `export_id`.
- [x] `IMPORT` применяет зачисление ровно один раз.
- [x] Duplicate import отклоняется детерминированно.
- [x] Replay guard сохраняется после restart/snapshot restore.
- [x] CLI/TUI/pwmd сообщения и статусы согласованы.

## Regression/safety
- [x] `cargo check --workspace` зелёный (после `cargo clean -p pwm-core`, чтобы сбросить stale fingerprint graph на Windows).
- [x] Таргетные тесты `pwm-core`/`pwmd`/`pwm-cli`/`pwm-tui` зелёные.
- [x] Документация и runbook синхронизированы с фактическим поведением (минимум: `docs/WHITE_SPEC_v0.md`, `docs/rfc/9-crossdomain-roaming.md`, `docs/pwmd.md` + sprint-13 review/test/status notes).

## Slice 0 closeout notes
- [x] Freeze-границы и acceptance gates зафиксированы в review docs Sprint 13.
- [x] Добавлен операторский runbook проверки CY->DO e2e ожиданий.

## Slice 2 P0 closure evidence (2026-04-26)
- [x] IMPORT provenance guard: принимается только при наличии зафиксированного EXPORT в `exported_registry` с совпадением `export_id` + `to/amount/target_domain`; произвольный `export_id` отклоняется (`InvalidImport`).
- [x] Duplicate IMPORT guard: повтор того же `export_id` стабильно отклоняется через `imported_set` (`DuplicateImport`).
- [x] pwmd snapshot persistence/recovery: `imported_set` и `exported_registry` сериализуются/восстанавливаются в `crates/pwmd/src/snapshot.rs` и покрыты тестом snapshot cycle через `pwmd`.
- [x] Проверки: `cargo fmt`; `cargo check -p pwm-core -p pwmd`; `cargo test -p pwm-core import_`; `cargo test -p pwmd snapshot_roundtrip_restores_import_replay_guard_and_provenance`.

## Slice 3 closure evidence (2026-04-27)
- [x] `POST /v1/tx` runtime wiring: `EXPORT/IMPORT` проходят через тот же ingress с shard/prefilter-guards; `IMPORT` дополнительно проверяется provenance/replay guard до `apply_tx`; `EXPORT/IMPORT` применяются синхронно (`apply_tx` + `seal([])`) вместо mempool admission.
- [x] HTTP contract synced: duplicate import -> `409 CONFLICT`; invalid import provenance -> `400 BAD_REQUEST`; happy-path export/import -> `204 NO_CONTENT`.
- [x] Observability: `GET /v1/status` расширен public counters `bridge_exported_registry_size` и `bridge_imported_set_size`.
- [x] Проверки Slice 3: `cargo fmt`; `cargo check --workspace`; `cargo test -p pwmd v1_tx_ -- --nocapture` (включая `v1_tx_rejects_import_unknown_export_id`, `v1_status_bridge_counters_grow_after_http_export_import`, `v1_tx_http_export_import_advances_head_height_via_sync_seal`, и обновлённый `v1_tx_rejects_cross_shard_transfer_on_local_path`).

## Slice 4 closure evidence (2026-04-27)
- [x] `pwm-cli` получил минимальные operator-команды inter-shard flow: `tx-export` (source step) и `tx-import` (target step) с тем же signing source contract (`--wallet` или `--master`+`--domain`) и `POST /v1/tx`.
- [x] Error UX сохранён консистентным: reject возвращается в стиле существующих tx-команд (`tx submit: HTTP <status> (<url>): <reason>`), включая HTTP-код и серверную причину.
- [x] Добавлены таргетные тесты `pwm-cli` на новый flow: happy parse для `tx-export`/`tx-import` и negative для `--export-id` формата.
- [x] Проверки Slice 4: `cargo fmt`; `cargo check -p pwm-cli`; `cargo test -p pwm-cli tx_export_cli_parsing_happy_path tx_import_cli_parsing_happy_path parse_export_id_hex_arg_rejects_non_hex_or_wrong_length`.

## Slice 5 closure evidence (2026-04-27)
- [x] `pwm-tui` получил минимальный inter-shard UX в текущей архитектуре: новый quick-help `F7` (inter-shard -> CLI), а в `F6 send` добавлен pre-submit guard для cross-domain маршрута с операторской подсказкой `tx-export` -> `tx-import`.
- [x] Ошибки submit на TUI-уровне стали консистентнее для EXPORT/IMPORT пути: при policy reject (`cross-domain transfer is disabled ... EXPORT/IMPORT`) сообщение дополнительно содержит короткий inter-shard operator route.
- [x] Добавлены таргетные unit-тесты `pwm-tui`: happy (`inter_shard_flow_cli_message_mentions_export_import_steps`) и negative (`format_submit_transfer_error_keeps_generic_for_other_failures`) + контрактный тест на inter-shard reject hint.
- [x] Проверки Slice 5: `cargo fmt`; `cargo check -p pwm-tui`; `cargo test -p pwm-tui inter_shard_flow_cli_message_mentions_export_import_steps`; `cargo test -p pwm-tui format_submit_transfer_error_adds_inter_shard_hint_for_policy_reject`; `cargo test -p pwm-tui format_submit_transfer_error_keeps_generic_for_other_failures`; `cargo test -p pwm-tui status_footer_line_rpc_offline_leads_then_poll_err`.

## Slice 6 closure evidence (2026-04-27)
- [x] Добавлен воспроизводимый automated 2-node smoke `CY -> DO` в `pwmd` test harness: source node фиксирует `EXPORT`, target node применяет `IMPORT` только после operator handoff существующего provenance-контракта (`exported_registry`) без добавления новых протокольных правил.
- [x] Negative suite закрыта в том же e2e test case: duplicate import reject (`409 CONFLICT`) и invalid/unknown import provenance reject (`400 BAD_REQUEST`, `export_id is not known`).
- [x] Проверки Slice 6: `cargo fmt`; `cargo check -p pwmd`; `cargo test -p pwmd v1_tx_two_node_smoke_cy_to_do_with_negative_suite -- --nocapture`.

## Slice 7 closure evidence (2026-04-27)
- [x] Консолидированы Sprint 13 артефакты (`checklist`/`test-report`/`status-note`/`review-report`) без расширения scope и без изменений plan-файлов.
- [x] Final verdict синхронизирован: Sprint 13 coding closeout завершен, пакет готов к финальному независимому прогону `pwm-testing`/`pwm-review`.
- [x] Post-cut notes зафиксированы как не-блокирующие (`P2`) и не открывают новый sprint scope.
- [x] Финальный sanity check set зафиксирован в `sprint-13-test-report.md` (умеренный объём: compile + ключевой e2e negative suite).

## Post-freeze extension: Slice 13.8 closure evidence (2026-04-27)
- [x] Dual pool введён: сохранён `pool` для локальных tx + добавлен federated `roaming_pool` для EXPORT/IMPORT intent lifecycle.
- [x] Lifecycle минимум закрыт: `queued/exported/relayed/imported/expired/failed` (с фактическими переходами `queued->exported`, `exported->imported|expired|failed`).
- [x] TTL semantics по высоте блока реализованы (`expires_at_height`) с авто-expire при превышении.
- [x] Funds lock semantics включены: active roaming-intent блокирует конкурирующие локальные tx по source account (`409 CONFLICT`, deterministic message).
- [x] `pwmd` API расширен минимально: `POST /v1/roaming-intents` (home-shard intent create) + `GET /v1/roaming-intents/:id` (intent status).
- [x] Snapshot persistence покрывает roaming intent pool + lock-state и восстанавливает их после restart.
- [x] Таргетные тесты добавлены и зелёные (lifecycle/ttl/lock/idempotency duplicate delivery + duplicate import guard).

## Post-freeze extension: Slice 13.9 closure evidence (2026-04-27)
- [x] CLI one-window cross-domain send: `tx-send` через home-shard roaming-intent API (`POST /v1/roaming-intents`) с lifecycle polling.
- [x] Backward compatibility сохранена: `tx-export`/`tx-import` остаются рабочим fallback/debug маршрутом.
- [x] TUI `F6` cross-domain route: создаёт roaming-intent и показывает lifecycle статусы (`queued/exported/relayed/imported/expired/failed`).
- [x] Error UX: детерминированные пользовательские сообщения на duplicate/invalid/expired; local send path не сломан.
- [x] Добавлены таргетные тесты CLI/TUI на one-window flow/error mapping + regression для local path.
