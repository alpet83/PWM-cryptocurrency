# Sprint 6 Status Note

Дата: 2026-04-24

## Что сделано (slice #1, coding)

- В `crates/pwmd/src/lib.rs` вынесены повторяющиеся блоки обновления transport/churn counters и `last_attempt`/`last_result` state в private helpers:
  - `record_transport_attempt(...)`,
  - `record_churn_attempt(...)`.
- Stub transport tick и real transport tick теперь используют единый путь записи transport attempt-метрик.
- Семантика и инварианты сохранены: no range heuristics, без tx-path изменений, без API drift.

## Gate state

- coding: pass (slice #1 implemented, low-risk refactor only)
- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed; helper-sensitive transport/churn/parity + regression invariants green)
- review: pass (baseline reset accepted; no semantic blockers found)
- orchestrator: ready_for_next_slice

## Следующие шаги

- Slice #1 зафиксирован как baseline reset (процессный scope drift acknowledged, семантических блокеров не выявлено).
- Запустить Slice #2 в строгом optimization-only режиме с узким diff (shared native-live/degraded helper extraction).

## Что сделано (slice #2, coding)

- В `crates/pwmd/src/lib.rs` добавлены private helper-функции:
  - `count_native_live_peers(...)` для общего подсчета live native peers,
  - `refresh_native_health(...)` для единой точки refresh/degraded evaluation вызова.
- Убрано дублирование в трех путях:
  - transport tick (`run_transport_tick_with`),
  - policy update после peer hello (`process_incoming_peer_hello`),
  - dev readback path (`v1_dev_peers`).
- Поведение сохранено:
  - transport degraded evaluation выполняется только там, где выполнялся ранее (transport tick),
  - в policy/dev readback сохраняется только policy refresh без новых side-effects.

## Gate state (slice #2 coding)

- coding: pass (optimization-only helper extraction complete)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #2 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- helper extraction parity: pass (policy/degraded calculations unchanged; transport degraded/underflow semantics unchanged; dev readback consistency unchanged)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #2 review)

- review: request_changes (scope-discipline evidence gap on broad working tree; no semantic blockers confirmed)
- orchestrator: ready_for_next_slice (baseline policy applied; next slice must provide strict narrow-diff evidence)

## Что сделано (slice #3, coding)

- В `crates/pwmd/src/lib.rs` устранено дублирование backoff/retry delay calculator:
  - добавлен private helper `compute_backoff_delay_ms(...)`,
  - `backoff_delay_ms(...)` и `retry_delay_ms(...)` переведены на общий helper.
- Поведение сохранено: формула, saturating arithmetic, cap (`max`), и порядок применения delay в transport/retry путях не менялись.
- Контрактные границы сохранены: tx-path/guards, HTTP routes, response fields и endpoint semantics не изменялись.

## Gate state (slice #3 coding)

- coding: pass (optimization-only narrow refactor complete)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #3 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- backoff parity: pass (delay formula/behavior unchanged; envelope and retry windows preserved)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #3 review)

- review: request_changes (process-evidence on strict narrow diff; semantic drift not confirmed)
- orchestrator: ready_for_next_slice (baseline policy applied; continue iterative optimization with stronger scope evidence)

## Что сделано (slice #4, coding)

- В `crates/pwmd/src/lib.rs` добавлен type-safe слой class labels:
  - private enum `ClassLabel` (`Native`/`Foreign`/`Unknown`),
  - mapping helper `ClassLabel::from_peer_class(...)`,
  - единая строковая проекция через `as_str()`/`Display`.
- Снижено stringly-typed дублирование в hot paths:
  - `class_label(...)` теперь использует typed mapping,
  - `PeerPolicyConfig::default().class_weights` использует typed labels,
  - transport attempt class-key fallback для unknown использует typed label.
- Семантика сохранена: текст label-ключей на выходе не менялся (`native`/`foreign`/`unknown`), tx/API/messages/route behavior без изменений.

## Gate state (slice #4 coding)

- coding: pass (optimization-only typed label extraction complete)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #4 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- label keys/metrics parity: pass (`native`/`foreign`/`unknown` labels preserved; class-based counters/policy semantics unchanged)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #4 review)

- review: request_changes (process-evidence on strict narrow diff; semantic drift not confirmed)
- orchestrator: ready_for_next_slice (baseline policy applied; continue with explicit scope-evidence package in next slice)

## Что сделано (slice #5, coding)

- В `crates/pwmd/src/lib.rs` добавлен private helper `compose_class_result_key(...)` для централизованного формирования ключа `dial_attempt_by_class_result`.
- В `record_transport_attempt(...)` формирование ключа переключено на helper без изменения выходного формата (`<class>:<result>`).
- Семантика сохранена: transport logic, tx-path/guards, HTTP routes/response fields/error messages не изменялись.

## Gate state (slice #5 coding)

- coding: pass (optimization-only helper extraction for class/result counter key)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #5 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- dial counter key parity: pass (формат ключей `<class>:<result>` сохранен; `native:success`/`foreign:retryable_fail`/`unknown:retryable_fail` подтверждены зелеными тестами)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #5 review)

- review: request_changes (process-evidence on strict narrow diff; semantic drift not confirmed)
- orchestrator: ready_for_next_slice (baseline policy applied; continue iterative optimization)

## Что сделано (slice #6, coding)

- В `crates/pwmd/src/lib.rs` добавлен private helper `compose_class_state_key(...)` как единая точка формирования class-key для state maps:
  - `last_attempt_ms_by_class`,
  - `last_result_by_class`.
- В `record_transport_attempt(...)` оба insert-path переведены на centralized key composition без изменения ключей/значений.
- Семантика сохранена: transport logic, tx-path/guards, HTTP routes/response fields/error messages не изменялись.

## Gate state (slice #6 coding)

- coding: pass (optimization-only helper extraction for class state map keys)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #6 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- class-state key parity: pass (после centralization `compose_class_state_key(...)` оба state-map paths `last_attempt_ms_by_class`/`last_result_by_class` используют единый ключевой helper без contract drift)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #6 review)

- review: approve_with_nits (semantic pass; process-evidence improved via scope-proof + structured manifest)
- orchestrator: ready_for_next_slice

## Что сделано (slice #7, coding)

- В `crates/pwmd/src/lib.rs` добавлен private helper `increment_reject_reason_total(...)` как единая точка string-key update для `reject_reason_total`.
- В `process_incoming_peer_hello(...)` reject-path переведен на helper; инкремент `rejected_total`, label mapping и warn-лог сохранены без semantic drift.
- Семантика сохранена: tx-path/guards, HTTP routes/response fields/error messages и API surface не изменялись.

## Gate state (slice #7 coding)

- coding: pass (optimization-only reject-reason counter key update centralization)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #7 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- reject_reason_total parity: pass (после helper extraction `increment_reject_reason_total(...)` reject counters и reason labels сохранены; `bad_signature` counters подтверждены в HTTP и real-transport тестах)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #7 review)

- review: request_changes (process-evidence sync was incomplete at review time; semantic drift not confirmed)
- orchestrator: ready_for_next_slice (baseline policy applied; manifest-driven pattern continues)

## Что сделано (slice #8, coding)

- В `crates/pwmd/src/lib.rs` добавлен private helper `increment_class_accept_total(...)` как единая точка обновления `class_accept_total`.
- В `process_incoming_peer_hello(...)` accept-path переведен на helper; инкремент `accepted_total`, class mapping и info-лог сохранены без semantic drift.
- Семантика сохранена: tx-path/guards, HTTP routes/response fields/error messages и API surface не изменялись.

## Gate state (slice #8 coding)

- coding: pass (optimization-only class_accept_total update centralization)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #8 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- class_accept_total parity: pass (после helper extraction `increment_class_accept_total(...)` accept counters и class labels сохранены; `native`/`foreign` class_accept_total подтверждены в status endpoint тестах)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #8 review)

- review: request_changes (process-evidence sync incomplete at review time; semantic drift not confirmed)
- orchestrator: ready_for_next_slice (baseline policy applied; continue manifest-driven optimization)

## Что сделано (slice #9, coding)

- В `crates/pwmd/src/lib.rs` добавлен private helper `increment_class_bucket(...)` для единообразного инкремента string-keyed per-class counters (`class_label` keys).
- `increment_class_accept_total(...)` делегирует в `increment_class_bucket(...)`; агрегация `connected_by_class` в `v1_dev_peers(...)` использует тот же helper вместо дублирующего inline map update.
- Семантика сохранена: те же ключи и счетчики, без изменений tx-path/guards, routes, response fields и error messages.

## Gate state (slice #9 coding)

- coding: pass (optimization-only class-bucket map increment centralization)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #9 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- class-bucket parity: pass (`increment_class_bucket(...)` для `class_accept_total` и `connected_by_class`; ключи и фильтр статусов в `v1_dev_peers` без drift)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #9 review)

- review: pass (semantic optimization-only; manifest slice #9; review-sync завершён в этом slice-closeout)
- orchestrator: ready_for_next_slice (manifest-driven optimization продолжается по backlog)

## Что сделано (slice #10, coding)

- В `crates/pwmd/src/lib.rs` добавлен private helper `is_peer_liveish(...)` как единая точка проверки liveish-статуса пира (`Accepted | Connected | Retrying`).
- В `prioritize_peer_candidates(...)`, `count_native_live_peers(...)` и `v1_dev_peers(...)` повторяющиеся `matches!(...)` заменены на helper.
- Семантика сохранена: тот же набор статусов в фильтрах, без изменений tx-path/guards, routes, response fields и error messages.

## Gate state (slice #10 coding)

- coding: pass (optimization-only liveish status predicate centralization)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #10 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- liveish filter parity: pass (`is_peer_liveish(...)` и все его call-sites используют тот же статус-набор `Accepted | Connected | Retrying`)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #10 review)

- review: pass (semantic optimization-only; process review-sync completed in slice-closeout)
- orchestrator: ready_for_next_slice (manifest-driven optimization continues)

## Что сделано (slice #11, coding)

- В `crates/pwmd/src/lib.rs` добавлен private helper `is_native_for_local(...)` для централизации проверки `classify_peer(local, peer_domain_hi) == PeerClass::Native`.
- В `prioritize_peer_candidates(...)` и `count_native_live_peers(...)` повторяющиеся inline-проверки заменены на helper без изменения правила domain equality (no-range).
- Семантика сохранена: без изменений tx-path/guards, routes, response fields и error messages.

## Gate state (slice #11 coding)

- coding: pass (optimization-only native-classification predicate centralization)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #11 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- native classification parity: pass (`is_native_for_local(...)` эквивалентен прежнему `classify_peer(...) == PeerClass::Native` в call-sites slice #11)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #11 review)

- review: pass (semantic optimization-only; manifest slice #11; review-sync completed in slice-closeout)
- orchestrator: ready_for_next_slice (manifest-driven optimization continues)

## Что сделано (slice #12, coding)

- В `crates/pwmd/src/lib.rs` добавлен private helper `classify_peer_for_hs(...)` рядом с `classify_peer(...)` как thin wrapper `classify_peer(hs.local_domain_hi, peer_domain_hi)`.
- В `run_transport_tick_with(...)` и `process_incoming_peer_hello(...)` вызовы `classify_peer(hs.local_domain_hi, …)` заменены на `classify_peer_for_hs(hs, …)` без изменения правила domain equality.
- Семантика сохранена: без изменений tx-path/guards, routes, response fields и error messages.

## Gate state (slice #12 coding)

- coding: pass (optimization-only handshake-scoped classify_peer wrapper + два call-site)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #12 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- classify_peer wrapper parity: pass (`classify_peer_for_hs(hs, …)` эквивалентен прежнему `classify_peer(hs.local_domain_hi, …)` в двух production call-site)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #12 review)

- review: pass (semantic optimization-only; manifest slice #12; review-sync завершён оркестратором после testing gate)
- orchestrator: ready_for_next_slice (manifest-driven optimization continues)

## Что сделано (slice #13, coding)

- В `crates/pwmd/src/lib.rs` добавлен private helper `increment_string_u64_bucket(...)` для общего паттерна инкремента счётчиков в `HashMap<String, u64>`.
- `increment_reject_reason_total(...)` и `increment_class_bucket(...)` переведены на helper: прежние ключи (`reason_label.to_string()`, `class_label(class).to_string()`) и целевые map без изменений.
- Семантика сохранена: без изменений tx-path/guards, routes, response fields и error messages.

## Gate state (slice #13 coding)

- coding: pass (optimization-only string-keyed u64 bucket helper)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #13 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- string-keyed counter parity: pass (`increment_string_u64_bucket(...)` сохраняет прежнюю семантику ключей для `reject_reason_total` и class buckets)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #13 review)

- review: pass (semantic optimization-only; manifest slice #13; `scoped_diff_stat` синхронизирован через `slice-artifacts.ps1 -Mode fill-diff`)
- orchestrator: ready_for_next_slice (manifest-driven optimization continues)

## Что сделано (slice #14, coding)

- В `crates/pwmd/src/lib.rs` `record_transport_attempt(...)` делегирует инкремент `dial_attempt_by_class_result` в `increment_string_u64_bucket(&mut snapshot.counters.dial_attempt_by_class_result, key)` после `compose_class_result_key(...)`; ключ `String` передаётся по ownership.
- `record_churn_attempt(...)` делегирует в `increment_string_u64_bucket(&mut churn.seed_attempt_by_result, result.as_label().to_string())`.
- Семантика ключей и map без изменений; last_attempt/last_result maps в transport path не трогались.

## Gate state (slice #14 coding)

- coding: pass (optimization-only reuse `increment_string_u64_bucket` для transport/churn dial counters)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #14 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- dial/churn counter parity: pass (`increment_string_u64_bucket` для `dial_attempt_by_class_result` и `seed_attempt_by_result`; ключи через `compose_class_result_key` / `result.as_label()` без drift)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #14 review)

- review: pass (semantic optimization-only; manifest slice #14; `scoped_diff_stat` синхронизирован оркестратором одним батч-патчем с `git diff --numstat HEAD`)
- orchestrator: ready_for_next_slice (manifest-driven optimization continues)

## Что сделано (slice #15, coding / tooling)

- В `tools/slice-artifacts.ps1` добавлен режим `patch-manifest-numstat`: обновление только `scoped_diff_stat` и `generated_at` для `review_evidence_manifest_sliceN` по сырому UTF-8 JSON (без полной пересериализации файла).
- В `tools/README-slice-artifacts.md` задокументирован режим и сценарий «кириллица в task json → не fill-diff».
- Sprint 6 артефакты slice #15 синхронизированы батч-патчем; `pwmd` исходники не менялись.

## Gate state (slice #15 coding)

- coding: pass (tooling-only; patch-manifest-numstat + README)
- fmt/check: n/a (Rust не менялся)

## Gate state (slice #15 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed; DryRun `patch-manifest-numstat` без ошибок)
- regressions: none detected (Rust tree unchanged for slice #15)

## Gate state (slice #15 review)

- review: pass (tooling scope; manifest slice #15; numstat через patch-manifest-numstat)
- orchestrator: ready_for_next_slice (при необходимости — дальнейшие optimization slices в `pwmd` или расширение tooling)

## Что сделано (slice #16, coding)

- В `crates/pwmd/src/lib.rs` добавлен `transport_outbound_slot(policy, class, scheduled_native, scheduled_foreign) -> (&mut u32, u32)`; `run_transport_tick_with` использует один вызов вместо двух `match class` на лимиты и счётчики.
- Поведение outbound caps и порядок инкремента `*scheduled` сохранены; `select_backoff_for_class`, `record_transport_attempt` и остальной transport path без изменений семантики.

## Gate state (slice #16 coding)

- coding: pass (optimization-only `transport_outbound_slot` + `run_transport_tick_with`)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #16 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- transport outbound parity: pass (лимиты native/foreign outbound и scheduled counters через helper без drift)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #16 review)

- review: pass (semantic optimization-only; manifest slice #16; `scoped_diff_stat` через `patch-manifest-numstat`, code paths only)
- orchestrator: ready_for_next_slice (manifest-driven optimization continues)

## Что сделано (slice #17, coding)

- В `crates/pwmd/src/lib.rs` добавлен `dial_attempt_class_key(class: Option<&PeerClass>)`; real transport seed loop использует helper вместо ручного `ClassLabel::from_peer_class` + `Unknown` fallback.
- Строковые ключи класса для `record_transport_attempt` совпадают с прежним контрактом (`native`/`foreign`/`unknown`).

## Gate state (slice #17 coding)

- coding: pass (optimization-only `dial_attempt_class_key` + real transport tick call-site)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #17 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- dial class key parity: pass (те же label-строки для Success/RetryableFail seed path)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #17 review)

- review: pass (semantic optimization-only; manifest slice #17; `scoped_diff_stat` через `patch-manifest-numstat`, code paths only)
- orchestrator: ready_for_next_slice (manifest-driven optimization continues)

## Что сделано (slice #18, coding)

- В `crates/pwmd/src/lib.rs` добавлен `enqueue_seed_by_last_peer_class`; цикл построения `due` в `run_real_transport_tick` делегирует в helper раскладку seed по последнему `PeerClass` (или unknown).
- Порядок последующих `extend` и бюджетирование попыток без изменений.

## Gate state (slice #18 coding)

- coding: pass (optimization-only `enqueue_seed_by_last_peer_class` + `run_real_transport_tick`)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #18 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- seed ordering parity: pass (native / unknown / foreign очереди и порядок `extend` сохранены)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #18 review)

- review: pass (semantic optimization-only; manifest slice #18; `scoped_diff_stat` через `patch-manifest-numstat`, code paths only)
- orchestrator: ready_for_next_slice (manifest-driven optimization continues)

## Что сделано (slice #19, coding)

- Выполнен batched micro-refactor (4 правки) в `crates/pwmd/src/lib.rs`: `apply_transport_peer_result`, `seed_peer_state_mut`, `update_known_peer_status`, `retryable_connect_outcome` + замена соответствующих call-site.
- Изменения локальны в transport/seed paths; semantic contracts сохранены.

## Gate state (slice #19 coding)

- coding: pass (batched optimization-only helper extraction)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #19 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- parity: pass (scheduler/backoff/degraded, real reconnect/status transitions, dev snapshot consistency)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #19 review)

- review: pass (semantic optimization-only; manifest slice #19; `scoped_diff_stat` через `patch-manifest-numstat`, code paths only)
- orchestrator: ready_for_next_slice (manifest-driven optimization continues)

## Что сделано (slice #20, coding)

- Выполнен batched micro-refactor (4 правки) в `crates/pwmd/src/lib.rs`: `rotate_seed_order`, `update_seed_peer_after_attempt`, `set_seed_peer_next_due`, `apply_reconnect_streak_tick` + замена соответствующих call-site.
- Изменения локальны в real transport tick paths; semantic contracts сохранены.

## Gate state (slice #20 coding)

- coding: pass (batched optimization-only helper extraction)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #20 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- parity: pass (scheduler/backoff/degraded, real reconnect/status transitions, dev snapshot consistency)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #20 review)

- review: pass (semantic optimization-only; manifest slice #20; `scoped_diff_stat` через `patch-manifest-numstat`, code paths only)
- orchestrator: ready_for_next_slice (manifest-driven optimization continues)

## Что сделано (slice #21, coding)

- Выполнен batched micro-refactor в `crates/pwmd/src/lib.rs`: `soak_counter_cap`, `refresh_real_tick_state`, `collect_due_seed_attempts`, `apply_seed_attempt_result`, `finalize_real_tick` + обновление `run_real_transport_tick`.
- Изменения локальны в real transport tick paths; semantic contracts сохранены.

## Gate state (slice #21 coding)

- coding: pass (batched optimization-only helper extraction)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #21 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- parity: pass (scheduler/backoff/degraded, real reconnect/status transitions, dev snapshot consistency)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #21 review)

- review: pass (semantic optimization-only; manifest slice #21; `scoped_diff_stat` через `patch-manifest-numstat`, code paths only)
- orchestrator: ready_for_next_slice (manifest-driven optimization continues)

## Что сделано (slice #22, coding)

- В `crates/pwmd/src/lib.rs` выполнен batched cleanup в transport/state helpers: локализация snapshot-доступа, удаление `compose_class_state_key`, упрощение key-write в `record_transport_attempt`, локальный `due_len` в due-budget расчёте.
- Изменения ограничены behavior-preserving реорганизацией локального кода.

## Gate state (slice #22 coding)

- coding: pass (optimization-only cleanup batch)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #22 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- parity: pass (scheduler/backoff/degraded, real reconnect/status transitions, dev snapshot consistency)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #22 review)

- review: pass (semantic optimization-only; manifest slice #22; `scoped_diff_stat` через `patch-manifest-numstat`, code paths only)
- orchestrator: ready_for_next_slice (manifest-driven optimization continues)

## Что сделано (slice #23, coding)

- В `crates/pwmd/src/lib.rs` выполнен batched optimization-only cleanup: удалён неиспользуемый `ClassLabel::from_peer_class`, упрощены `class_label(...)` и `is_native_for_local(...)`, убран лишний `clone()` класса в seed-приоритизации.
- Изменения локальны и behavior-preserving (без изменения контрактов/API).

## Gate state (slice #23 coding)

- coding: pass (optimization-only cleanup batch)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #23 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- parity: pass (scheduler/backoff/degraded, real reconnect/status transitions, dev snapshot consistency)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #23 review)

- review: pass (semantic optimization-only; manifest slice #23; `scoped_diff_stat` через `patch-manifest-numstat`, code paths only)
- orchestrator: ready_for_next_slice (manifest-driven optimization continues)

## Что сделано (slice #24, coding)

- В `crates/pwmd/src/lib.rs` выполнен batched optimization-only cleanup: вынесен `peer_priority_rank(...)`, локализован `under_native_min` в `refresh_native_health(...)`, оптимизирован `compose_class_result_key(...)`, добавлен fast-path в `increment_string_u64_bucket(...)` и синхронизированы callsites.
- Изменения локальны и behavior-preserving (без изменения API/контрактов).

## Gate state (slice #24 coding)

- coding: pass (optimization-only cleanup batch)
- fmt/check: pass (`cargo fmt`, `cargo check -p pwmd`)

## Gate state (slice #24 testing)

- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed)
- parity: pass (scheduler/backoff/degraded, real reconnect/status transitions, dev snapshot consistency)
- regressions: none detected (tx-path invariants, no-range heuristics, dev endpoints compatibility preserved)

## Gate state (slice #24 review)

- review: pass (semantic optimization-only; manifest slice #24; `scoped_diff_stat` через `patch-manifest-numstat`, code paths only)
- orchestrator: ready_for_next_slice (готово к итоговой сводке Sprint 6 и плану следующего optimization спринта)
## Оркестрация (политика темпа, 2026-04-25)

- Дальше в Sprint 6 **группировать 3–4 узких coding-изменения** в одном рабочем цикле перед одним `slice-commit` / одним номером slice (меньше overhead артефактов и коммитов при том же объёме DRY).
- **Декомпозиция `lib.rs` по файлам/модулям** вынесена в **отдельный спринт**; текущий конвейер остаётся optimization-only без крупных переносов кода между модулями.
