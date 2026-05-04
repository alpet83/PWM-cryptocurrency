# Sprint 6 Checklist (optimization)

## Slice #1 Scope (low-risk quick win)

- [x] В `crates/pwmd/src/lib.rs` устранено дублирование обновления transport attempt-метрик (`dial_attempt_by_class_result`, `last_attempt_ms_by_class`, `last_result_by_class`) между stub и real transport paths.
- [x] Добавлен единый helper `record_transport_attempt(...)` для унифицированной записи attempt/result state.
- [x] Добавлен единый helper `record_churn_attempt(...)` для унифицированной записи `churn.seed_attempt_by_result` на real path.
- [x] Поведение не изменено: no range heuristics, без изменений tx-path, без расширения публичных API контрактов.
- [x] Выполнены только low-risk локальные изменения без больших архитектурных переносов.

## Done Criteria (slice #1)

- [x] Stub и real transport paths используют общий код обновления transport-state/metrics.
- [x] Локальная декомпозиция ограничена private helpers в `lib.rs`.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #1)

- [x] Не добавлять новый сетевой функционал transport.
- [x] Не расширять тест-матрицу (оставлено для `pwm-testing`).

## Slice #2 Scope (optimization-only)

- [x] В `crates/pwmd/src/lib.rs` вынесен private helper `count_native_live_peers(...)` для повторяющегося подсчета live native peers.
- [x] В `crates/pwmd/src/lib.rs` вынесен private helper `refresh_native_health(...)` как единая точка вызова refresh/degraded evaluation:
  - transport path: policy refresh + transport degraded evaluation,
  - policy/dev readback paths: только policy refresh (без transport degraded side-effects).
- [x] Повторяющиеся inline-фрагменты в transport tick, peer hello processing и `v1_dev_peers` заменены на вызовы helper-функций.
- [x] Ограничения соблюдены: без изменений tx guards, endpoint contracts, error messages и routing semantics.

## Done Criteria (slice #2)

- [x] Shared helper для native-live подсчета используется в policy/transport/dev readback путях.
- [x] Unified helper для refresh/degraded evaluation подключен в местах, где это возможно без semantic drift.
- [x] Дифф узкий, без изменения transport алгоритма.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #2)

- [x] Не добавлять новые API поля.
- [x] Не менять transport алгоритм/семантику.

## Slice #3 Scope (optimization-only)

- [x] В `crates/pwmd/src/lib.rs` выделен единый private helper `compute_backoff_delay_ms(...)` для расчета delay по `base/max/attempts`.
- [x] Дублирование между `backoff_delay_ms(...)` и `retry_delay_ms(...)` устранено через единый helper.
- [x] Сохранены значения и порядок применения delay в transport envelope path и reconnect retry path (behavior-preserving).
- [x] Ограничения соблюдены: без изменений tx guards, HTTP routes, response fields и endpoint semantics.

## Done Criteria (slice #3)

- [x] Единая backoff/retry delay логика используется в обоих местах без копипаста калькулятора.
- [x] Алгоритм backoff/retry не изменен (те же saturating/shift/cap правила).
- [x] Дифф узкий и ограничен scope файлами slice #3.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #3)

- [x] Не менять tx-path/tx guards.
- [x] Не менять API/endpoint contracts, маршруты и поля ответов.

## Slice #4 Scope (optimization-only, review-evidence)

- [x] В `crates/pwmd/src/lib.rs` добавлен типобезопасный слой class labels через `ClassLabel` + mapping helper `from_peer_class(...)`.
- [x] Повторяющиеся string literals (`native`/`foreign`) в hot paths заменены на typed mapping там, где это behavior-safe.
- [x] Текст выходных ключей/labels сохранен без изменений (`native`, `foreign`, `unknown`).
- [x] Ограничения соблюдены: без изменений tx guards, routes, response fields, error messages.

## Done Criteria (slice #4)

- [x] `class_label(...)` переведен на typed enum mapping (single source of truth для label-строк).
- [x] `class_weights` defaults и transport class-key fallback используют typed labels вместо raw literals.
- [x] Дифф узкий и ограничен разрешенными файлами slice #4.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #4)

- [x] Не менять transport/tx алгоритмы.
- [x] Не менять API/endpoints/messages контракты.

## Slice #5 Scope (optimization-only, review-evidence)

- [x] В `crates/pwmd/src/lib.rs` устранено дублирование формирования map-key для `dial_attempt_by_class_result` через единый private helper `compose_class_result_key(...)`.
- [x] Формат и значения ключей сохранены без изменений (`<class>:<result>`).
- [x] Изменения ограничены узким scope: без изменений tx-path/guards, HTTP routes, response fields, error messages.

## Done Criteria (slice #5)

- [x] Формирование ключа для class/result counter централизовано в одном helper.
- [x] Поведение transport attempt counters не изменено.
- [x] Добавлен pre-review scope proof блок в review report.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #5)

- [x] Не менять transport scheduling/backoff семантику.
- [x] Не менять tx-path и публичные API контракты.

## Slice #6 Scope (optimization-only, review-evidence)

- [x] В `crates/pwmd/src/lib.rs` централизовано формирование map-key для `last_attempt_ms_by_class` и `last_result_by_class` через private helper `compose_class_state_key(...)`.
- [x] `record_transport_attempt(...)` переведен на единую точку формирования class-key для обоих access paths.
- [x] Значения ключей и алгоритм сохранены без изменений (behavior-preserving refactor only).
- [x] Изменения ограничены узким scope: без изменений tx-path/guards, HTTP routes, response fields, error messages.

## Done Criteria (slice #6)

- [x] Формирование class-key для `last_attempt_ms_by_class` / `last_result_by_class` централизовано в одном helper.
- [x] Семантика transport attempt state updates не изменена.
- [x] Добавлен pre-review scope proof блок в review report для slice #6.
- [x] В task artifact добавлен `review_evidence_manifest_slice6`.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #6)

- [x] Не менять transport scheduling/backoff семантику.
- [x] Не менять tx-path и публичные API контракты.

## Slice #7 Scope (optimization-only, review-evidence)

- [x] В `crates/pwmd/src/lib.rs` централизовано обновление string key для `reject_reason_total` через private helper `increment_reject_reason_total(...)`.
- [x] Reject-path в `process_incoming_peer_hello(...)` переведен на helper без изменения логики инкремента и возвращаемого label.
- [x] Значения reject reason keys и поведение reject counters сохранены без изменений (behavior-preserving refactor only).
- [x] Изменения ограничены узким scope: без изменений tx-path/guards, HTTP routes, response fields, error messages.

## Done Criteria (slice #7)

- [x] Обновление `reject_reason_total` выполняется через единую helper-точку.
- [x] Семантика reject accounting не изменена.
- [x] Добавлен pre-review scope proof блок в review report для slice #7.
- [x] В task artifact добавлен `review_evidence_manifest_slice7`.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #7)

- [x] Не менять tx-path/tx guards.
- [x] Не менять HTTP routes/response fields/error messages.
- [x] Не добавлять новые API поля.

## Slice #8 Scope (optimization-only, manifest-driven)

- [x] В `crates/pwmd/src/lib.rs` централизовано обновление `class_accept_total` через private helper `increment_class_accept_total(...)`.
- [x] В `process_incoming_peer_hello(...)` direct map update заменен на helper без изменения class label mapping и accepted accounting.
- [x] Значения class keys и поведение accept counters сохранены без изменений (behavior-preserving refactor only).
- [x] Изменения ограничены узким scope: без изменений tx-path/guards, HTTP routes, response fields, error messages.

## Done Criteria (slice #8)

- [x] Обновление `class_accept_total` выполняется через единую helper-точку.
- [x] Семантика accept accounting не изменена.
- [x] Добавлен pre-review scope proof блок в review report для slice #8.
- [x] В task artifact добавлен `review_evidence_manifest_slice8`.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #8)

- [x] Не менять tx-path/tx guards.
- [x] Не менять HTTP routes/response fields/error messages.
- [x] Не добавлять новые API поля.

## Slice #9 Scope (optimization-only, manifest-driven)

- [x] В `crates/pwmd/src/lib.rs` добавлен private helper `increment_class_bucket(...)` как единая точка инкремента `HashMap<String, u64>` по ключу `class_label(class)` (class-connected counter map pattern).
- [x] `increment_class_accept_total(...)` переведен на вызов `increment_class_bucket(...)` без изменения ключей и семантики accept accounting.
- [x] Агрегация `connected_by_class` в `v1_dev_peers(...)` использует тот же helper вместо inline `entry`/`or_insert` (behavior-preserving; dev readback only).
- [x] Изменения ограничены узким scope: без изменений tx-path/guards, HTTP routes, response fields, error messages.

## Done Criteria (slice #9)

- [x] Инкремент per-class string-key counters для accept metrics и `connected_by_class` проходит через единый helper.
- [x] Семантика подсчетов и ключей (`native`/`foreign`) не изменена.
- [x] Добавлен pre-review scope proof блок в review report для slice #9.
- [x] В task artifact добавлен `review_evidence_manifest_slice9`.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #9)

- [x] Не менять tx-path/tx guards.
- [x] Не менять HTTP routes/response fields/error messages.
- [x] Не добавлять новые API поля.

## Slice #10 Scope (optimization-only, manifest-driven)

- [x] В `crates/pwmd/src/lib.rs` добавлен private helper `is_peer_liveish(...)` для централизации repeated status-check (`Accepted | Connected | Retrying`).
- [x] Inline проверки liveish-статуса заменены на helper в 3 локальных местах: `prioritize_peer_candidates(...)`, `count_native_live_peers(...)`, `v1_dev_peers(...)`.
- [x] Семантика фильтра статусов сохранена без изменений (behavior-preserving refactor only).
- [x] Изменения ограничены узким scope: без изменений tx-path/guards, HTTP routes, response fields, error messages.

## Done Criteria (slice #10)

- [x] Повторяющаяся проверка liveish-статусов централизована в одном helper.
- [x] `prioritize_peer_candidates`, `count_native_live_peers` и `v1_dev_peers` используют единый helper без contract drift.
- [x] Добавлен pre-review scope proof блок в review report для slice #10.
- [x] В task artifact добавлен `review_evidence_manifest_slice10`.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #10)

- [x] Не менять tx-path/tx guards.
- [x] Не менять HTTP routes/response fields/error messages.
- [x] Не добавлять новые API поля.

## Slice #11 Scope (optimization-only, manifest-driven)

- [x] В `crates/pwmd/src/lib.rs` добавлен private helper `is_native_for_local(...)` как единая точка проверки `classify_peer(...) == PeerClass::Native` для пары `(local_domain_hi, peer_domain_hi)`.
- [x] Дублирующие inline-проверки native-классификации заменены на helper в `prioritize_peer_candidates(...)` и `count_native_live_peers(...)` без изменения правила domain equality.
- [x] Изменения ограничены узким scope: без изменений tx-path/guards, HTTP routes, response fields, error messages.

## Done Criteria (slice #11)

- [x] Повторяющаяся проверка native-классификации централизована в одном helper.
- [x] `prioritize_peer_candidates` и `count_native_live_peers` используют helper без contract drift.
- [x] Добавлен pre-review scope proof блок в review report для slice #11.
- [x] В task artifact добавлен `review_evidence_manifest_slice11`.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #11)

- [x] Не менять tx-path/tx guards.
- [x] Не менять HTTP routes/response fields/error messages.
- [x] Не добавлять новые API поля.

## Slice #12 Scope (optimization-only, manifest-driven)

- [x] В `crates/pwmd/src/lib.rs` добавлен private helper `classify_peer_for_hs(...)` как thin wrapper над `classify_peer(hs.local_domain_hi, peer_domain_hi)` для handshake-scoped call-sites.
- [x] Ровно два production call-site переведены на helper: `run_transport_tick_with(...)`, `process_incoming_peer_hello(...)`; семантика классификации peer без изменений.
- [x] Изменения ограничены узким scope: без изменений tx-path/guards, HTTP routes, response fields, error messages.

## Done Criteria (slice #12)

- [x] Дублирование пары `(hs.local_domain_hi, peer_domain_hi)` для `classify_peer` устранено в двух указанных местах без contract drift.
- [x] Добавлен pre-review scope proof блок в review report для slice #12.
- [x] В task artifact добавлен `review_evidence_manifest_slice12`.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #12)

- [x] Не менять tx-path/tx guards.
- [x] Не менять HTTP routes/response fields/error messages.
- [x] Не добавлять новые API поля.

## Slice #13 Scope (optimization-only, manifest-driven)

- [x] В `crates/pwmd/src/lib.rs` добавлен private helper `increment_string_u64_bucket(...)` для единого паттерна `*map.entry(key).or_insert(0) += 1` на `HashMap<String, u64>`.
- [x] `increment_reject_reason_total(...)` делегирует в helper с тем же ключом `reason_label.to_string()` и тем же map `reject_reason_total`.
- [x] `increment_class_bucket(...)` делегирует в helper с тем же ключом `class_label(class).to_string()` и тем же переданным map.
- [x] Изменения ограничены узким scope: без изменений tx-path/guards, HTTP routes, response fields, error messages.

## Done Criteria (slice #13)

- [x] Дублирование entry/or_insert/increment для string-keyed `u64` buckets устранено без семантического drift.
- [x] Добавлен pre-review scope proof блок в review report для slice #13.
- [x] В task artifact добавлен `review_evidence_manifest_slice13`.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #13)

- [x] Не менять tx-path/tx guards.
- [x] Не менять HTTP routes/response fields/error messages.
- [x] Не добавлять новые API поля.

## Slice #14 Scope (optimization-only, manifest-driven)

- [x] В `crates/pwmd/src/lib.rs` `record_transport_attempt(...)` после `compose_class_result_key(...)` инкрементирует `dial_attempt_by_class_result` через `increment_string_u64_bucket(...)` с ownership `String` ключа без лишнего `clone`.
- [x] `record_churn_attempt(...)` инкрементирует `seed_attempt_by_result` через `increment_string_u64_bucket(..., result.as_label().to_string())` — те же ключи и map, что и ранее `entry(...).or_insert(0) += 1`.
- [x] Логика `last_attempt_ms_by_class` / `last_result_by_class` в `record_transport_attempt` без изменений; tx-path/guards, routes, response fields, error messages не затронуты.

## Done Criteria (slice #14)

- [x] Transport/churn dial counters переиспользуют `increment_string_u64_bucket(...)` без семантического drift по ключам.
- [x] Добавлен pre-review scope proof блок в review report для slice #14.
- [x] В task artifact добавлен `review_evidence_manifest_slice14`.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #14)

- [x] Не менять tx-path/tx guards.
- [x] Не менять HTTP routes/response fields/error messages.
- [x] Не добавлять новые API поля.

## Slice #15 Scope (tooling, manifest-driven)

- [x] В `tools/slice-artifacts.ps1` добавлен режим `patch-manifest-numstat`: точечная замена `scoped_diff_stat` и `generated_at` в `review_evidence_manifest_sliceN` по `git diff --numstat HEAD` без `ConvertTo-Json` всего task-файла (UTF-8 без BOM, кириллица в заметках не пересобирается).
- [x] В `tools/README-slice-artifacts.md` описан новый режим и когда предпочитать его вместо `fill-diff`.
- [x] Изменения ограничены tooling + sprint-6 артефакты/manifest slice #15; `crates/pwmd` не менялся.

## Done Criteria (slice #15)

- [x] Raw UTF-8 read/write: только целевой manifest-блок обновляется парсером границ JSON-массива `scoped_diff_stat`.
- [x] DryRun режима выполняется без ошибок на рабочем дереве slice #15.
- [x] `cargo test -p pwmd` (регрессия; Rust без изменений).

## Non-goals (slice #15)

- [x] Не менять поведение `fill-diff` и `init` кроме расширения `ValidateSet` режимов.
- [x] Не трогать `crates/pwmd/src/lib.rs` и сетевую семантику.

## Slice #16 Scope (optimization-only, manifest-driven)

- [x] В `crates/pwmd/src/lib.rs` добавлен private helper `transport_outbound_slot(...)` — один `match` по `PeerClass` возвращает `(&mut u32, u32)` для счётчика scheduled и лимита outbound (`native_outbound_target` / `foreign_outbound_target`).
- [x] `run_transport_tick_with(...)` переведён на helper вместо двух последовательных `match class` на те же поля policy и те же `scheduled_native` / `scheduled_foreign`.
- [x] Семантика лимитов и инкремента scheduled без изменений; tx-path/guards, HTTP routes, response fields, error messages не затронуты.

## Done Criteria (slice #16)

- [x] Дублирование ветвления по классу peer в transport tick устранено без семантического drift.
- [x] Добавлен pre-review scope proof блок в review report для slice #16.
- [x] В task artifact добавлен `review_evidence_manifest_slice16`.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #16)

- [x] Не менять transport scheduling/backoff семантику вне выбора пары (counter, limit).
- [x] Не менять tx-path/tx guards.
- [x] Не менять HTTP routes/response fields/error messages.
- [x] Не добавлять новые API поля.

## Slice #17 Scope (optimization-only, manifest-driven)

- [x] В `crates/pwmd/src/lib.rs` добавлен private helper `dial_attempt_class_key(Option<&PeerClass>) -> String` для единого формирования строки класса в real seed dial path (`class_label` / `unknown` как у `ClassLabel::Unknown`).
- [x] Ветка после `attempt_seed_connect` в `run_real_transport_tick` переведена на helper вместо `map(ClassLabel::from_peer_class).to_string()` + `unwrap_or(ClassLabel::Unknown)`.
- [x] Семантика ключей для `record_transport_attempt` без изменений; tx-path/guards, routes, response fields, error messages не затронуты.

## Done Criteria (slice #17)

- [x] Дублирование формирования class-key для seed dial устранено без семантического drift по label-строкам.
- [x] Добавлен pre-review scope proof блок в review report для slice #17.
- [x] В task artifact добавлен `review_evidence_manifest_slice17`.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #17)

- [x] Не менять `attempt_seed_connect` контракт возврата и сетевую семантику handshake.
- [x] Не менять tx-path/tx guards.
- [x] Не менять HTTP routes/response fields/error messages.
- [x] Не добавлять новые API поля.

## Slice #18 Scope (optimization-only, manifest-driven)

- [x] В `crates/pwmd/src/lib.rs` добавлен private helper `enqueue_seed_by_last_peer_class(...)` — один `match` по `Option<PeerClass>` раскладывает seed в `native` / `foreign` / `unknown` очереди как раньше.
- [x] Цикл подготовки `due` в `run_real_transport_tick` вызывает helper вместо inline `match rank`.
- [x] Порядок `due.extend(native); due.extend(unknown); due.extend(foreign)` без изменений; tx-path/guards, routes, response fields, error messages не затронуты.

## Done Criteria (slice #18)

- [x] Дублирование ветвления по последнему известному классу peer для seed ordering устранено без семантического drift.
- [x] Добавлен pre-review scope proof блок в review report для slice #18.
- [x] В task artifact добавлен `review_evidence_manifest_slice18`.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #18)

- [x] Не менять seed rotation, backoff skip логику и бюджет попыток.
- [x] Не менять tx-path/tx guards.
- [x] Не менять HTTP routes/response fields/error messages.
- [x] Не добавлять новые API поля.

## Slice #19 Scope (optimization-only, batched micro-refactors)

- [x] В `crates/pwmd/src/lib.rs` добавлен `apply_transport_peer_result(...)`: общий apply outcome для `run_transport_tick_with` (`attempts`/`next_due_ms` при Success/RetryableFail).
- [x] Добавлен `seed_peer_state_mut(...)` для повторяющегося доступа к `hs.transport.seed_peers.entry(seed_key).or_default()` в `run_real_transport_tick`.
- [x] Добавлен `update_known_peer_status(...)` для единого обновления `PeerStatus` и `last_seen_ms` (при необходимости) по `last_node_id`.
- [x] Добавлен `retryable_connect_outcome()` и применён в ранних retryable-return ветках `attempt_seed_connect(...)`.
- [x] Семантика scheduler/backoff/churn и API/tx contracts сохранена.

## Done Criteria (slice #19)

- [x] Батч из 4 связанных micro-DRY правок выполнен в одном slice.
- [x] Добавлен pre-review scope proof блок в review report для slice #19.
- [x] В task artifact добавлен `review_evidence_manifest_slice19`.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #19)

- [x] Не менять transport scheduling/backoff/churn семантику.
- [x] Не менять tx-path/tx guards.
- [x] Не менять HTTP routes/response fields/error messages.
- [x] Не добавлять новые API поля.

## Slice #20 Scope (optimization-only, batched micro-refactors)

- [x] В `crates/pwmd/src/lib.rs` добавлен `rotate_seed_order(...)` для централизации построения seed-order от `seed_rotation_cursor`.
- [x] Добавлены `update_seed_peer_after_attempt(...)` и `set_seed_peer_next_due(...)` для повторяющихся обновлений `seed_peers` state после dial-attempt.
- [x] Добавлен `apply_reconnect_streak_tick(...)` для единого обновления reconnect streak/unstable/stable счетчиков в real transport tick.
- [x] Call-site в `run_real_transport_tick(...)` переведены на helper-вызовы без изменения ветвлений/порогов.
- [x] Семантика scheduler/backoff/churn/status transitions и API/tx contracts сохранена.

## Done Criteria (slice #20)

- [x] Батч из 4 связанных micro-DRY правок выполнен в одном slice.
- [x] Добавлен pre-review scope proof блок в review report для slice #20.
- [x] В task artifact добавлен `review_evidence_manifest_slice20`.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #20)

- [x] Не менять transport scheduling/backoff/churn семантику.
- [x] Не менять tx-path/tx guards.
- [x] Не менять HTTP routes/response fields/error messages.
- [x] Не добавлять новые API поля.

## Slice #21 Scope (optimization-only, batched micro-refactors)

- [x] В `crates/pwmd/src/lib.rs` добавлены `soak_counter_cap(...)` и `refresh_real_tick_state(...)` для pre-tick bookkeeping и cap-логики.
- [x] Добавлены `collect_due_seed_attempts(...)`, `apply_seed_attempt_result(...)` и `finalize_real_tick(...)` для централизации due-collection, per-attempt apply и post-tick runaway/streak финализации.
- [x] `run_real_transport_tick(...)` переведен на helper-вызовы без изменения порогов/ветвлений и без изменения scheduler/backoff/churn semantics.
- [x] API/tx/route/response/error contracts сохранены.

## Done Criteria (slice #21)

- [x] Батч из 4 связанных micro-DRY правок выполнен в одном slice.
- [x] Добавлен pre-review scope proof блок в review report для slice #21.
- [x] В task artifact добавлен `review_evidence_manifest_slice21`.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #21)

- [x] Не менять transport scheduling/backoff/churn семантику.
- [x] Не менять tx-path/tx guards.
- [x] Не менять HTTP routes/response fields/error messages.
- [x] Не добавлять новые API поля.

## Slice #22 Scope (optimization-only, batched micro-refactors)

- [x] В `crates/pwmd/src/lib.rs` локально упрощён `refresh_native_health(...)`: повторные обращения к `hs.transport.snapshot` заменены на локальную ссылку.
- [x] В `record_transport_attempt(...)` удалён промежуточный key-helper path: запись в `last_attempt_ms_by_class` / `last_result_by_class` идёт через прямой `class_key.to_string()`, семантика ключей неизменна.
- [x] Удалён неиспользуемый passthrough helper `compose_class_state_key(...)`.
- [x] В `collect_due_seed_attempts(...)` убрано повторное вычисление `due.len() as u32` через локальный `due_len` (без изменения budget логики).

## Done Criteria (slice #22)

- [x] Выполнен batched optimization-only cleanup без semantic drift.
- [x] Добавлен pre-review scope proof блок в review report для slice #22.
- [x] В task artifact добавлен `review_evidence_manifest_slice22`.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #22)

- [x] Не менять transport scheduling/backoff/churn семантику.
- [x] Не менять tx-path/tx guards.
- [x] Не менять HTTP routes/response fields/error messages.
- [x] Не добавлять новые API поля.

## Slice #23 Scope (optimization-only, batched micro-refactors)

- [x] В `crates/pwmd/src/lib.rs` `class_label(...)` переведён на прямой `match` по `PeerClass`; удалён неиспользуемый `ClassLabel::from_peer_class`.
- [x] `is_native_for_local(...)` упрощён до прямого сравнения `peer_domain_hi == local_domain_hi` (эквивалент текущему правилу классификации).
- [x] `enqueue_seed_by_last_peer_class(...)` принимает `Option<&PeerClass>`; в `collect_due_seed_attempts(...)` убран лишний `clone()` класса.
- [x] Семантика scheduler/backoff/churn/status transitions и API/tx contracts сохранена.

## Done Criteria (slice #23)

- [x] Выполнен batched optimization-only cleanup без semantic drift.
- [x] Добавлен pre-review scope proof блок в review report для slice #23.
- [x] В task artifact добавлен `review_evidence_manifest_slice23`.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #23)

- [x] Не менять transport scheduling/backoff/churn семантику.
- [x] Не менять tx-path/tx guards.
- [x] Не менять HTTP routes/response fields/error messages.
- [x] Не добавлять новые API поля.

## Slice #24 Scope (optimization-only, batched micro-refactors)

- [x] В `crates/pwmd/src/lib.rs` в `prioritize_peer_candidates(...)` вынесен helper `peer_priority_rank(...)` для устранения дублирования ранжирования.
- [x] В `refresh_native_health(...)` повторная проверка порога сведена к локальному `under_native_min` без изменения переходов состояния.
- [x] В `compose_class_result_key(...)` сборка ключа оптимизирована через `String::with_capacity` + `push_str/push` с сохранением формата `"{class}:{result}"`.
- [x] В `increment_string_u64_bucket(...)` добавлен fast-path (`key: &str`, borrowed lookup), вызовы в reject/class/transport/churn счётчиках синхронизированы без semantic drift.

## Done Criteria (slice #24)

- [x] Выполнен batched optimization-only cleanup без semantic drift.
- [x] Добавлен pre-review scope proof блок в review report для slice #24.
- [x] В task artifact добавлен `review_evidence_manifest_slice24`.
- [x] `cargo fmt` выполнен успешно.
- [x] `cargo check -p pwmd` выполнен успешно.

## Non-goals (slice #24)

- [x] Не менять transport scheduling/backoff/churn семантику.
- [x] Не менять tx-path/tx guards.
- [x] Не менять HTTP routes/response fields/error messages.
- [x] Не добавлять новые API поля.
## Оркестрация Sprint 6 (актуальная политика темпа)

- Дальнейшие micro-оптимизации в `pwmd` **батчить по 3–4 правки на один slice** (один тестовый прогон + один коммит по manifest), чтобы не раздувать конвейер.
- **Декомпозиция `lib.rs` по модулям** — **отдельный спринт**, не входит в обязанности текущего узкого optimization conveyor (см. `tools/README-slice-artifacts.md`, раздел *Sprint 6 orchestration*).
