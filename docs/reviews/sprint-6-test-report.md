# Sprint 6 Test Report (optimization slice #1)

Дата: 2026-04-24  
Исполнитель: `pwm-testing` (independent verification)

## Verdict

**PASS**

Slice #1 (`behavior-preserving refactor` transport/churn/state helper-unification в `pwmd`) подтвержден: полный `pwmd` прогон зелёный, helper-чувствительные сценарии по transport/churn counters и parity stub/real path сохраняют ожидаемое поведение, регрессий по tx-path/no-range/dev endpoints не выявлено.

## Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`).

## Проверка helper-sensitive сценариев (slice #1)

- **transport snapshot counters consistency**:
  - `transport_scheduler_orders_native_and_respects_backoff`,
  - `transport_retry_backoff_transitions_follow_envelope`,
  - `transport_degraded_state_requires_persistent_underflow_ticks`,
  - `v1_dev_peers_exposes_transport_snapshot`.
- **churn counters consistency**:
  - `real_transport_reconnect_sets_retrying_then_disconnected_with_bounded_cooldown`,
  - `real_transport_runaway_guard_stops_then_resumes_attempts`,
  - `real_transport_soak_rollups_are_bounded_and_periodic`.
- **stub vs real transport path parity (key metrics)**:
  - stub path покрыт через transport scheduler/snapshot tests (`transport_*`, `v1_dev_peers_exposes_transport_snapshot`);
  - real path покрыт через `real_transport_tick_connects_seed_and_accepts_handshake`, `real_transport_tick_rejects_bad_signature_and_tracks_reason`, `real_transport_tick_respects_retry_backoff_on_connect_timeout`, `real_transport_tick_uses_deterministic_seed_rotation_with_budget`;
  - итог: counters/results paths согласованы, drift между helper-вызовами на stub/real не выявлен.

## Regression check

- **tx-path invariants**: регрессий не выявлено; зелёные `v1_tx_accepts_signed_init`, `v1_tx_accepts_regulatory_lo_zero_init`, `v1_tx_rejects_domain_mismatch`, `v1_tx_rejects_wrong_shard_for_sender_domain_hi`, `v1_tx_rejects_cross_shard_transfer_on_local_path`, prefilter guards (`reserve/witness/unknown`) и body-limit guard.
- **no-range heuristics invariant**: сохранён; зелёные `policy_classification_uses_only_domain_equality_no_ranges`, `policy_prioritizes_native_first_without_range_heuristics`.
- **dev endpoints compatibility**: совместимость сохранена; зелёные `v1_dev_peers_exposes_transport_snapshot`, `v1_peer_hello_accepts_and_classifies_native`, `v1_peer_hello_classifies_foreign_and_exposes_reject_counters`.

## Residual risks

- Блокирующих проблем не найдено.
- Residual risk: parity оценивалась по automated test harness; перед merge/closeout всё ещё полезен короткий smoke на реальном multi-node окружении с churn bursts.

---

## Slice #2 Test Gate (strict optimization-only)

Дата: 2026-04-24  
Исполнитель: `pwm-testing` (independent verification)

### Verdict

**PASS**

Slice #2 (`shared native-live/degraded helper extraction` в `pwmd`) прошел testing gate в strict optimization-only режиме: полный `cargo test -p pwmd` зеленый, helper extraction parity подтвержден, регрессий по tx-path/no-range/dev endpoints не выявлено.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### Helper extraction parity (slice #2)

- **policy/degraded calculations behavior unchanged**:
  - `policy_native_degraded_state_toggles_by_native_min_live`,
  - `policy_classification_uses_only_domain_equality_no_ranges`,
  - `policy_prioritizes_native_first_without_range_heuristics`.
- **transport degraded/underflow semantics unchanged**:
  - `transport_degraded_state_requires_persistent_underflow_ticks`,
  - `transport_retry_backoff_transitions_follow_envelope`,
  - `transport_scheduler_orders_native_and_respects_backoff`.
- **dev readback consistency unchanged**:
  - `v1_dev_peers_exposes_transport_snapshot`,
  - `v1_peer_hello_accepts_and_classifies_native`,
  - `v1_peer_hello_classifies_foreign_and_exposes_reject_counters`.

### Regression check

- **tx-path invariants**: инварианты сохранены; зеленые `v1_tx_accepts_signed_init`, `v1_tx_accepts_regulatory_lo_zero_init`, `v1_tx_rejects_domain_mismatch`, `v1_tx_rejects_wrong_shard_for_sender_domain_hi`, `v1_tx_rejects_cross_shard_transfer_on_local_path`, prefilter guards (`reserve/witness/unknown`) и body-limit guard.
- **no-range heuristics**: инвариант сохранен; `policy_classification_uses_only_domain_equality_no_ranges` и `policy_prioritizes_native_first_without_range_heuristics` зеленые.
- **dev endpoints compatibility**: совместимость сохранена; `v1_dev_peers_exposes_transport_snapshot`, `v1_peer_hello_accepts_and_classifies_native`, `v1_peer_hello_classifies_foreign_and_exposes_reject_counters` зеленые.

### Residual risks

- Блокирующих проблем не выявлено.
- Остаточный риск: проверка выполнена на automated harness; для полного closeout по-прежнему полезен короткий multi-node smoke с churn bursts.

---

## Slice #3 Test Gate (backoff calculator unification)

Дата: 2026-04-24  
Исполнитель: `pwm-testing` (independent verification)

### Verdict

**PASS**

Slice #3 (`backoff/retry delay calculator unification` в `pwmd`) прошел testing gate: полный `cargo test -p pwmd` зеленый, parity значений/поведения delay подтвержден, регрессий по tx-path/no-range/dev endpoints не обнаружено.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### Backoff parity checks (slice #3)

- **delay calculator semantics unchanged**:
  - единый helper `compute_backoff_delay_ms(...)` использует прежнюю формулу: `shift = saturating_sub(1).min(20)`, `exp = 1 << shift`, `delay = base * exp` с saturating arithmetic и cap `min(max)`;
  - `backoff_delay_ms(...)` и `retry_delay_ms(...)` являются обертками к той же формуле, drift между transport/retry path отсутствует.
- **observed delay behavior unchanged (tests)**:
  - `transport_retry_backoff_transitions_follow_envelope`: `next_due_ms` сохраняется как `2000 -> 4000 -> 8000` при последовательных retry (для foreign envelope);
  - `real_transport_tick_respects_retry_backoff_on_connect_timeout`: при `retry_base_ms=400` первый due остается в ожидаемом окне `[1400, 1500]`;
  - `policy_backoff_envelopes_differ_by_class`: native/foreign envelopes и выбор policy counters сохраняют прежний контракт.

### Regression check

- **tx-path invariants**: регрессий не выявлено; зеленые `v1_tx_accepts_signed_init`, `v1_tx_accepts_regulatory_lo_zero_init`, `v1_tx_rejects_domain_mismatch`, `v1_tx_rejects_wrong_shard_for_sender_domain_hi`, `v1_tx_rejects_cross_shard_transfer_on_local_path`, prefilter guards (`reserve/witness/unknown`) и body-limit guard.
- **no-range heuristics**: инвариант сохранен; зеленые `policy_classification_uses_only_domain_equality_no_ranges`, `policy_prioritizes_native_first_without_range_heuristics`.
- **dev endpoints compatibility**: совместимость сохранена; зеленые `v1_dev_peers_exposes_transport_snapshot`, `v1_peer_hello_accepts_and_classifies_native`, `v1_peer_hello_classifies_foreign_and_exposes_reject_counters`.

### Residual risks

- Блокирующих проблем не выявлено.
- Остаточный риск: parity подтвержден на unit/integration harness; перед final closeout полезен короткий multi-node smoke с churn bursts.

---

## Slice #4 Test Gate (typed class labels refactor)

Дата: 2026-04-24  
Исполнитель: `pwm-testing` (independent verification)

### Verdict

**PASS**

Slice #4 (`typed class labels refactor` в `pwmd`) прошел testing gate: полный `cargo test -p pwmd` зеленый, parity по label keys/metrics (`native`/`foreign`/`unknown`) подтвержден, регрессий по tx-path/no-range/dev endpoints не обнаружено.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### Label parity checks (slice #4)

- **label key stability preserved**:
  - `handshake::tests::reason_labels_are_stable` зеленый; текстовые label-представления остаются стабильными;
  - `v1_peer_hello_accepts_and_classifies_native`, `v1_peer_hello_classifies_foreign_and_exposes_reject_counters` зеленые; classification-path сохраняет class labels для `native`/`foreign`;
  - fallback unknown-path сохраняется через прежние guard/reject сценарии без изменения текстов ключей.
- **metrics/counters semantics unchanged**:
  - `policy_backoff_envelopes_differ_by_class`, `policy_prioritizes_native_first_without_range_heuristics` зеленые;
  - `transport_scheduler_orders_native_and_respects_backoff`, `transport_retry_backoff_transitions_follow_envelope`, `transport_degraded_state_requires_persistent_underflow_ticks` зеленые;
  - итог: typed mapping не изменил счетчики/веса/политики по классам.

### Regression check

- **tx-path invariants**: регрессий не выявлено; зеленые `v1_tx_accepts_signed_init`, `v1_tx_accepts_regulatory_lo_zero_init`, `v1_tx_rejects_domain_mismatch`, `v1_tx_rejects_wrong_shard_for_sender_domain_hi`, `v1_tx_rejects_cross_shard_transfer_on_local_path`, prefilter guards (`reserve/witness/unknown`) и body-limit guard.
- **no-range heuristics**: инвариант сохранен; зеленые `policy_classification_uses_only_domain_equality_no_ranges`, `policy_prioritizes_native_first_without_range_heuristics`.
- **dev endpoints compatibility**: совместимость сохранена; зеленые `v1_dev_peers_exposes_transport_snapshot`, `v1_peer_hello_accepts_and_classifies_native`, `v1_peer_hello_classifies_foreign_and_exposes_reject_counters`.

### Residual risks

- Блокирующих проблем не выявлено.
- Остаточный риск: parity подтвержден на unit/integration harness; для полного closeout полезен короткий multi-node smoke с churn bursts.

---

## Slice #10 Test Gate (liveish status predicate centralization)

Дата: 2026-04-24  
Исполнитель: `pwm-testing` (independent verification)

### Verdict

**PASS**

Slice #10 (`is_peer_liveish(...)` helper extraction в `pwmd`) прошел testing gate: полный `cargo test -p pwmd` зеленый, parity liveish-фильтра подтверждена (те же статусы `Accepted | Connected | Retrying`), регрессий по покрытым тестам не выявлено.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### Liveish filter parity checks (slice #10)

- **shared predicate semantics unchanged**:
  - helper `is_peer_liveish(status)` использует тот же `matches!`-набор: `PeerStatus::Accepted | PeerStatus::Connected | PeerStatus::Retrying`;
  - `prioritize_peer_candidates(...)`, `count_native_live_peers(...)` и `v1_dev_peers(...)` переведены на helper без расширения/сужения фильтра.
- **covered behavior remains stable**:
  - `transport_scheduler_orders_native_and_respects_backoff`, `transport_retry_backoff_transitions_follow_envelope`, `transport_degraded_state_requires_persistent_underflow_ticks` зеленые;
  - `policy_native_degraded_state_toggles_by_native_min_live`, `policy_prioritizes_native_first_without_range_heuristics` зеленые;
  - `v1_dev_peers_exposes_transport_snapshot`, `v1_peer_hello_accepts_and_classifies_native`, `v1_peer_hello_classifies_foreign_and_exposes_reject_counters` зеленые.

### Regression check

- **tx-path invariants**: регрессий не выявлено; зеленые `v1_tx_accepts_signed_init`, `v1_tx_accepts_regulatory_lo_zero_init`, `v1_tx_rejects_domain_mismatch`, `v1_tx_rejects_wrong_shard_for_sender_domain_hi`, `v1_tx_rejects_cross_shard_transfer_on_local_path`, prefilter guards (`reserve/witness/unknown`) и body-limit guard.
- **no-range heuristics**: инвариант сохранен; зеленые `policy_classification_uses_only_domain_equality_no_ranges`, `policy_prioritizes_native_first_without_range_heuristics`.
- **dev endpoints compatibility**: совместимость сохранена; зеленые `v1_dev_peers_exposes_transport_snapshot`, `v1_peer_hello_accepts_and_classifies_native`, `v1_peer_hello_classifies_foreign_and_exposes_reject_counters`.

### Residual risks

- Блокирующих проблем не выявлено.
- Остаточный риск: parity подтверждена automated harness и codepath inspection; перед final closeout полезен короткий multi-node smoke с churn bursts.

---

## Slice #11 Test Gate (native classification predicate centralization)

Дата: 2026-04-24  
Исполнитель: `pwm-testing` (independent verification)

### Verdict

**PASS**

Slice #11 (`is_native_for_local(...)` helper extraction в `pwmd`) прошёл testing gate: полный `cargo test -p pwmd` зелёный, parity native-классификации подтверждена (эквивалент прежнему `classify_peer(...) == PeerClass::Native`), регрессий по tx-path/no-range/dev endpoints не выявлено.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### Native classification parity checks (slice #11)

- **predicate equivalence preserved**:
  - helper `is_native_for_local(local_domain_hi, peer_domain_hi)` возвращает тот же boolean, что и `classify_peer(local_domain_hi, peer_domain_hi) == PeerClass::Native`;
  - `prioritize_peer_candidates(...)` и `count_native_live_peers(...)` используют helper без изменения tie-break ordering и native-live counting semantics.
- **covered behavior remains stable**:
  - `policy_native_degraded_state_toggles_by_native_min_live`, `policy_prioritizes_native_first_without_range_heuristics` зелёные;
  - `transport_scheduler_orders_native_and_respects_backoff`, `transport_retry_backoff_transitions_follow_envelope`, `transport_degraded_state_requires_persistent_underflow_ticks` зелёные;
  - `v1_dev_peers_exposes_transport_snapshot`, `v1_peer_hello_accepts_and_classifies_native`, `v1_peer_hello_classifies_foreign_and_exposes_reject_counters` зелёные.

### Regression check

- **tx-path invariants**: регрессий не выявлено; зелёные `v1_tx_accepts_signed_init`, `v1_tx_accepts_regulatory_lo_zero_init`, `v1_tx_rejects_domain_mismatch`, `v1_tx_rejects_wrong_shard_for_sender_domain_hi`, `v1_tx_rejects_cross_shard_transfer_on_local_path`, prefilter guards (`reserve/witness/unknown`) и body-limit guard.
- **no-range heuristics**: инвариант сохранён; зелёные `policy_classification_uses_only_domain_equality_no_ranges`, `policy_prioritizes_native_first_without_range_heuristics`.
- **dev endpoints compatibility**: совместимость сохранена; зелёные `v1_dev_peers_exposes_transport_snapshot`, `v1_peer_hello_accepts_and_classifies_native`, `v1_peer_hello_classifies_foreign_and_exposes_reject_counters`.

### Residual risks

- Блокирующих проблем не выявлено.
- Остаточный риск: parity подтверждена automated harness; для полного closeout полезен короткий multi-node smoke с churn bursts.

---

## Slice #12 Test Gate (handshake-scoped classify_peer wrapper)

Дата: 2026-04-24  
Исполнитель: `pwm-testing` (оркестратор: независимый прогон после coding)

### Verdict

**PASS**

Slice #12 (`classify_peer_for_hs(...)` thin wrapper в `pwmd`) прошёл testing gate: полный `cargo test -p pwmd` зелёный, parity классификации через wrapper подтверждена, регрессий по tx-path/no-range/dev endpoints не выявлено.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### Peer classification parity checks (slice #12)

- **wrapper semantics unchanged**:
  - `classify_peer_for_hs(hs, peer_domain_hi)` делегирует в `classify_peer(hs.local_domain_hi, peer_domain_hi)` без изменения правила domain equality;
  - production call-sites: `run_transport_tick_with(...)` и `process_incoming_peer_hello(...)`.
- **covered behavior remains stable**:
  - `transport_scheduler_orders_native_and_respects_backoff`, `transport_retry_backoff_transitions_follow_envelope`, `transport_degraded_state_requires_persistent_underflow_ticks` зелёные;
  - `policy_native_degraded_state_toggles_by_native_min_live`, `policy_prioritizes_native_first_without_range_heuristics` зелёные;
  - `v1_peer_hello_accepts_and_classifies_native`, `v1_peer_hello_classifies_foreign_and_exposes_reject_counters`, `v1_dev_peers_exposes_transport_snapshot` зелёные.

### Regression check

- **tx-path invariants**: регрессий не выявлено; зелёные `v1_tx_accepts_signed_init`, `v1_tx_accepts_regulatory_lo_zero_init`, `v1_tx_rejects_domain_mismatch`, `v1_tx_rejects_wrong_shard_for_sender_domain_hi`, `v1_tx_rejects_cross_shard_transfer_on_local_path`, prefilter guards (`reserve/witness/unknown`) и body-limit guard.
- **no-range heuristics**: инвариант сохранён; зелёные `policy_classification_uses_only_domain_equality_no_ranges`, `policy_prioritizes_native_first_without_range_heuristics`.
- **dev endpoints compatibility**: совместимость сохранена; зелёные `v1_dev_peers_exposes_transport_snapshot`, `v1_peer_hello_accepts_and_classifies_native`, `v1_peer_hello_classifies_foreign_and_exposes_reject_counters`.

### Residual risks

- Блокирующих проблем не выявлено.
- Остаточный риск: parity подтверждена automated harness; для полного closeout полезен короткий multi-node smoke с churn bursts.

---

## Slice #13 Test Gate (string-keyed u64 bucket helper)

Дата: 2026-04-24  
Исполнитель: `pwm-testing` (оркестратор: независимый прогон после coding)

### Verdict

**PASS**

Slice #13 (`increment_string_u64_bucket(...)` для `increment_reject_reason_total` / `increment_class_bucket` в `pwmd`) прошёл testing gate: полный `cargo test -p pwmd` зелёный, parity string-keyed счётчиков подтверждена, регрессий по tx-path/no-range/dev endpoints не выявлено.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### String-keyed counter parity checks (slice #13)

- **единый паттерн `HashMap<String, u64>` + `entry/or_insert += 1`**:
  - `increment_reject_reason_total(...)` по-прежнему инкрементирует `reject_reason_total[reason_label.to_string()]`;
  - `increment_class_bucket(...)` по-прежнему инкрементирует map по ключу `class_label(class).to_string()` (в т.ч. для `class_accept_total` / `connected_by_class` через существующие call-site’ы).
- **covered behavior remains stable**:
  - `v1_peer_hello_classifies_foreign_and_exposes_reject_counters`, `real_transport_tick_rejects_bad_signature_and_tracks_reason` зелёные (reject reason counters);
  - `v1_peer_hello_accepts_and_classifies_native`, `v1_dev_peers_exposes_transport_snapshot` зелёные (class buckets / dev readback).

### Regression check

- **tx-path invariants**: регрессий не выявлено; зелёные `v1_tx_accepts_signed_init`, `v1_tx_accepts_regulatory_lo_zero_init`, `v1_tx_rejects_domain_mismatch`, `v1_tx_rejects_wrong_shard_for_sender_domain_hi`, `v1_tx_rejects_cross_shard_transfer_on_local_path`, prefilter guards (`reserve/witness/unknown`) и body-limit guard.
- **no-range heuristics**: инвариант сохранён; зелёные `policy_classification_uses_only_domain_equality_no_ranges`, `policy_prioritizes_native_first_without_range_heuristics`.
- **dev endpoints compatibility**: совместимость сохранена; зелёные `v1_dev_peers_exposes_transport_snapshot`, `v1_peer_hello_accepts_and_classifies_native`, `v1_peer_hello_classifies_foreign_and_exposes_reject_counters`.

### Residual risks

- Блокирующих проблем не выявлено.
- Остаточный риск: parity подтверждена automated harness; для полного closeout полезен короткий multi-node smoke с churn bursts.

---

## Slice #14 Test Gate (transport/churn dial counters → `increment_string_u64_bucket`)

Дата: 2026-04-24  
Исполнитель: `pwm-testing` (оркестратор: прогон после coding)

### Verdict

**PASS**

Slice #14 (делегирование `dial_attempt_by_class_result` / `seed_attempt_by_result` в `increment_string_u64_bucket` в `pwmd`) прошёл testing gate: полный `cargo test -p pwmd` зелёный, parity ключей и инкремента подтверждена, регрессий по tx-path/no-range/dev endpoints не выявлено.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### Parity checks (slice #14)

- **`dial_attempt_by_class_result`**: ключи по-прежнему из `compose_class_result_key(class_key, result)`; инкремент через `increment_string_u64_bucket` эквивалентен прежнему `entry(key).or_insert(0) += 1`.
- **`seed_attempt_by_result`**: ключи `result.as_label().to_string()` без изменений; тот же helper-паттерн.

### Regression check

- **tx-path invariants**: регрессий не выявлено; зелёные `v1_tx_accepts_signed_init`, `v1_tx_accepts_regulatory_lo_zero_init`, `v1_tx_rejects_domain_mismatch`, `v1_tx_rejects_wrong_shard_for_sender_domain_hi`, `v1_tx_rejects_cross_shard_transfer_on_local_path`, prefilter guards (`reserve/witness/unknown`) и body-limit guard.
- **no-range heuristics**: инвариант сохранён; зелёные `policy_classification_uses_only_domain_equality_no_ranges`, `policy_prioritizes_native_first_without_range_heuristics`.
- **dev endpoints compatibility**: совместимость сохранена; зелёные `v1_dev_peers_exposes_transport_snapshot`, `v1_peer_hello_accepts_and_classifies_native`, `v1_peer_hello_classifies_foreign_and_exposes_reject_counters`.

### Residual risks

- Блокирующих проблем не выявлено.
- Остаточный риск: parity подтверждена automated harness; для полного closeout полезен короткий multi-node smoke с churn bursts.

---

## Slice #15 Test Gate (slice-artifacts `patch-manifest-numstat`)

Дата: 2026-04-25  
Исполнитель: `pwm-testing` (оркестратор)

### Verdict

**PASS**

Slice #15 — tooling-only: добавлен режим `patch-manifest-numstat` для точечного обновления `scoped_diff_stat` без полной пересборки task JSON. Регрессия `pwmd` не затронута исходниками slice #15.

### Команды и результаты

- `pwsh -NoProfile -File tools/slice-artifacts.ps1 -SliceNumber 15 -Mode patch-manifest-numstat -DryRun` -> PASS (план печатается, файлы не пишутся).
- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### Tooling checks (slice #15)

- **raw JSON patch path**: DryRun проходит разбор границ `scoped_diff_stat` для `review_evidence_manifest_slice15` без `ConvertTo-Json` всего файла.
- **Rust parity**: исходники `crates/pwmd` не менялись; полный тестовый прогон зелёный как контроль регрессии.

### Regression check

- **tx-path / transport semantics**: не затронуты (нет изменений в `pwmd`).
- **task JSON кириллица**: не пересобиралась целиком в рамках slice #15 closeout (цель режима — избежать шага, который ломал UTF-8 в прошлом).

### Residual risks

- Низкий риск: raw-патчер предполагает стабильную структуру JSON вокруг `scoped_diff_stat`; при ручном редактировании формата массива нужна осторожность.

---

## Slice #16 Test Gate (`transport_outbound_slot` in transport tick)

Дата: 2026-04-25  
Исполнитель: `pwm-testing` (оркестратор)

### Verdict

**PASS**

Slice #16 — optimization-only: единый helper для пары (scheduled counter, outbound limit) в `run_transport_tick_with`; полный прогон `pwmd` зелёный, регрессий по transport scheduler / backoff / degraded не выявлено.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### Parity checks (slice #16)

- **outbound limits**: `native_outbound_target` / `foreign_outbound_target` читаются из того же `PeerPolicySnapshot`, что и до рефакторинга; ветвление по `PeerClass` перенесено в `transport_outbound_slot` без изменения значений.
- **scheduled counters**: `scheduled_native` / `scheduled_foreign` инкрементируются через тот же `&mut u32`, что и ранее; порядок проверки `*scheduled >= limit` сохранён.

### Regression check

- **tx-path invariants**: регрессий не выявлено; зелёные `v1_tx_*`, prefilter guards, body-limit guard.
- **no-range heuristics**: инвариант сохранён; зелёные `policy_classification_uses_only_domain_equality_no_ranges`, `policy_prioritizes_native_first_without_range_heuristics`.
- **dev endpoints / transport**: зелёные `v1_dev_peers_exposes_transport_snapshot`, `transport_scheduler_orders_native_and_respects_backoff`, `transport_retry_backoff_transitions_follow_envelope`, `transport_degraded_state_requires_persistent_underflow_ticks`, real transport harness tests.

### Residual risks

- Блокирующих проблем не выявлено.

---

## Slice #17 Test Gate (`dial_attempt_class_key` for seed dial)

Дата: 2026-04-25  
Исполнитель: `pwm-testing` (оркестратор)

### Verdict

**PASS**

Slice #17 — optimization-only: единый helper для строки класса при записи transport attempt после seed connect; полный прогон `pwmd` зелёный.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### Parity checks (slice #17)

- **class key strings**: для `Some(Native|Foreign)` — `class_label` → те же строки, что и через `ClassLabel::from_peer_class(...).to_string()`; для `None` — `unknown` как у `ClassLabel::Unknown`.

### Regression check

- **tx-path / no-range / dev+transport harness**: без регрессий; зелёные real transport и snapshot/dev тесты из прогона slice #17.

### Residual risks

- Блокирующих проблем не выявлено.

---

## Slice #18 Test Gate (`enqueue_seed_by_last_peer_class`)

Дата: 2026-04-25  
Исполнитель: `pwm-testing` (оркестратор)

### Verdict

**PASS**

Slice #18 — optimization-only: helper для раскладки seed по последнему классу peer перед `extend(native|unknown|foreign)`; полный прогон `pwmd` зелёный.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### Parity checks (slice #18)

- **queues**: `Some(Native|Foreign)` и `None` маршрутизируются в те же векторы, что и до рефакторинга; порядок `due.extend` не менялся.

### Regression check

- **real transport / scheduler / snapshot**: зелёные harness-тесты из полного прогона; tx-path и no-range инварианты без регрессий.

### Residual risks

- Блокирующих проблем не выявлено.

---

## Slice #19 Test Gate (batched transport/seed helper extraction)

Дата: 2026-04-25  
Исполнитель: `pwm-testing` (оркестратор)

### Verdict

**PASS**

Slice #19 — optimization-only batched micro-refactor (4 helper extraction) в transport/seed зоне; полный прогон `pwmd` зелёный.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### Parity checks (slice #19)

- **transport tick apply result**: `apply_transport_peer_result` повторяет прежние ветки Success/RetryableFail для attempts/next_due_ms.
- **seed peer state access**: `seed_peer_state_mut` не меняет ключи/карты, только централизует `entry(...).or_default()`.
- **peer status transitions**: `update_known_peer_status` сохраняет прежние переходы Connected/Retrying/Disconnected и условное обновление `last_seen_ms`.
- **retryable connect outcome**: ранние выходы `attempt_seed_connect` возвращают тот же tuple `(RetryableFail, None, None)`.

### Regression check

- scheduler/backoff/degraded, real reconnect/status transitions и dev snapshot visibility — без регрессий по полному тестовому прогону.

### Residual risks

- Блокирующих проблем не выявлено; риск ограничен покрытием существующего набора тестов.

---
## Slice #20 Test Gate (batched real-transport helper extraction)

Дата: 2026-04-25  
Исполнитель: `pwm-testing` (оркестратор)

### Verdict

**PASS**

Slice #20 — optimization-only batched micro-refactor (4 helper extraction) в real transport tick paths; полный прогон `pwmd` зелёный.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### Parity checks (slice #20)

- **seed rotation order**: `rotate_seed_order(...)` сохраняет прежнюю ротацию по `seed_rotation_cursor`.
- **seed peer state updates**: `update_seed_peer_after_attempt(...)` и `set_seed_peer_next_due(...)` сохраняют прежние значения `attempts/last_node_id/next_due_ms`.
- **reconnect streak counters**: `apply_reconnect_streak_tick(...)` сохраняет прежние условия и обновления stable/unstable counters.
- **status transitions**: real reconnect/status paths остаются эквивалентными прежним веткам.

### Regression check

- scheduler/backoff/degraded, real reconnect/status transitions и dev snapshot visibility — без регрессий по полному тестовому прогону.

### Residual risks

- Блокирующих проблем не выявлено; риск ограничен покрытием существующего набора тестов.

---

## Slice #21 Test Gate (batched real-transport orchestration helpers)

Дата: 2026-04-25  
Исполнитель: `pwm-testing` (оркестратор)

### Verdict

**PASS**

Slice #21 — optimization-only batched helper extraction в real transport tick orchestration; полный прогон `pwmd` зелёный.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### Parity checks (slice #21)

- **pre-tick bookkeeping**: `soak_counter_cap(...)` / `refresh_real_tick_state(...)` сохраняют прежние условия и значения snapshot/guard state.
- **due collection**: `collect_due_seed_attempts(...)` сохраняет rotation/class ranking/budget и backoff-skip semantics.
- **attempt apply**: `apply_seed_attempt_result(...)` сохраняет transitions/counters/cooldown logic для Success/RetryableFail.
- **post-tick finalize**: `finalize_real_tick(...)` сохраняет прежнюю runaway/streak semantics.

### Regression check

- scheduler/backoff/degraded, real reconnect/status transitions и dev snapshot visibility — без регрессий по полному тестовому прогону.

### Residual risks

- Блокирующих проблем не выявлено; риск ограничен покрытием существующего набора тестов.

---

## Slice #22 Test Gate (transport/state cleanup batch)

Дата: 2026-04-25  
Исполнитель: `pwm-testing` (оркестратор)

### Verdict

**PASS**

Slice #22 — optimization-only cleanup batch в transport/state helper зоне; полный прогон `pwmd` зелёный.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### Parity checks (slice #22)

- **scheduler/backoff/degraded**: без изменений по тестовому поведению.
- **real reconnect/status transitions**: без изменений по regression suite.
- **dev snapshot visibility**: без регрессий.

### Regression check

- Существующий полный `pwmd` suite (55 tests) прошёл без падений.

### Residual risks

- Блокирующих проблем не выявлено; риск ограничен покрытием текущего набора тестов.

---

## Slice #23 Test Gate (transport/classification cleanup batch)

Дата: 2026-04-25  
Исполнитель: `pwm-testing` (оркестратор)

### Verdict

**PASS**

Slice #23 — optimization-only cleanup batch в transport/classification helper зоне; полный прогон `pwmd` зелёный.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### Parity checks (slice #23)

- **scheduler/backoff/degraded**: без изменений по тестовому поведению.
- **real reconnect/status transitions**: без изменений по regression suite.
- **dev snapshot visibility**: без регрессий.

### Regression check

- Существующий полный `pwmd` suite (55 tests) прошёл без падений.

### Residual risks

- Блокирующих проблем не выявлено; риск ограничен покрытием текущего набора тестов.

---

## Slice #24 Test Gate (transport/state micro-optimization batch)

Дата: 2026-04-25  
Исполнитель: `pwm-testing` (оркестратор)

### Verdict

**PASS**

Slice #24 — optimization-only cleanup batch в transport/state helper зоне; полный прогон `pwmd` зелёный.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### Parity checks (slice #24)

- **scheduler/backoff/degraded**: без изменений по тестовому поведению.
- **real reconnect/status transitions**: без изменений по regression suite.
- **dev snapshot visibility**: без регрессий.

### Regression check

- Существующий полный `pwmd` suite (55 tests) прошёл без падений.

### Residual risks

- Блокирующих проблем не выявлено; риск ограничен покрытием текущего набора тестов и отсутствием отдельного perf-baseline.

---

## Slice #8 Test Gate (class_accept_total helper centralization)

Дата: 2026-04-24  
Исполнитель: `pwm-testing` (independent verification)

### Verdict

**PASS**

Slice #8 (`class_accept_total` helper extraction в `pwmd`) прошел testing gate: полный `cargo test -p pwmd` зеленый, parity обновления `class_accept_total` после centralization в `increment_class_accept_total(...)` подтвержден, регрессий по tx-path/no-range/dev endpoints не выявлено.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### class_accept_total parity checks (slice #8)

- **single-point accept counter update preserved**:
  - helper `increment_class_accept_total(...)` инкрементирует `class_accept_total[class_label]` через `entry(...).or_insert(0) += 1` без изменения key/value семантики;
  - accept-path в `process_incoming_peer_hello(...)` по-прежнему выполняет `accepted_total += 1`, update class counter и `info`-лог в том же semantic порядке.
- **observed behavior unchanged in covered tests**:
  - `v1_peer_hello_accepts_and_classifies_native` зеленый: в `/v1/status` подтвержден `accepted_total == 1` и `class_accept_total["native"] == 1`;
  - `v1_peer_hello_classifies_foreign_and_exposes_reject_counters` зеленый: в `/v1/status` подтвержден `class_accept_total["foreign"] == 1` при сохранении reject counters semantics.

### Regression check

- **tx-path invariants**: регрессий не выявлено; зеленые `v1_tx_accepts_signed_init`, `v1_tx_accepts_regulatory_lo_zero_init`, `v1_tx_rejects_domain_mismatch`, `v1_tx_rejects_wrong_shard_for_sender_domain_hi`, `v1_tx_rejects_cross_shard_transfer_on_local_path`, prefilter guards (`reserve/witness/unknown`) и body-limit guard.
- **no-range heuristics**: инвариант сохранен; зеленые `policy_classification_uses_only_domain_equality_no_ranges`, `policy_prioritizes_native_first_without_range_heuristics`.
- **dev endpoints compatibility**: совместимость сохранена; зеленые `v1_dev_peers_exposes_transport_snapshot`, `v1_peer_hello_accepts_and_classifies_native`, `v1_peer_hello_classifies_foreign_and_exposes_reject_counters`.

### Residual risks

- Блокирующих проблем не выявлено.
- Остаточный риск: parity подтвержден на unit/integration harness и status-endpoint assertions; для полного closeout полезен короткий multi-node smoke с churn bursts.

---

## Slice #9 Test Gate (increment_class_bucket shared for accept + dev aggregation)

Дата: 2026-04-24  
Исполнитель: `pwm-testing` (independent verification)

### Verdict

**PASS**

Slice #9 (`increment_class_bucket(...)` для `class_accept_total` и `connected_by_class` в `pwmd`) прошёл testing gate: полный `cargo test -p pwmd` зелёный, parity per-class string-key counters без смены ключей/фильтра статусов, регрессий по tx-path/no-range/dev endpoints не выявлено.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### Class-bucket parity checks (slice #9)

- **единый паттерн инкремента `HashMap<String, u64>` по `class_label(class)`**:
  - `increment_class_accept_total(...)` делегирует в `increment_class_bucket(&mut metrics.class_accept_total, class)` — та же `entry(...).or_insert(0) += 1` семантика, что и до рефакторинга для accept counters;
  - `v1_dev_peers(...)` агрегирует `connected_by_class` через тот же helper при неизменном фильтре `Accepted | Connected | Retrying`.
- **observed behavior unchanged in covered tests**:
  - `v1_peer_hello_accepts_and_classifies_native`, `v1_peer_hello_classifies_foreign_and_exposes_reject_counters` зелёные: `class_accept_total` в `/v1/status` согласован с прежними assertions;
  - `v1_dev_peers_exposes_transport_snapshot` зелёный: dev readback и `connected_by_class` остаются совместимыми с transport snapshot harness.

### Regression check

- **tx-path invariants**: регрессий не выявлено; зелёные `v1_tx_accepts_signed_init`, `v1_tx_accepts_regulatory_lo_zero_init`, `v1_tx_rejects_domain_mismatch`, `v1_tx_rejects_wrong_shard_for_sender_domain_hi`, `v1_tx_rejects_cross_shard_transfer_on_local_path`, prefilter guards (`reserve/witness/unknown`) и body-limit guard.
- **no-range heuristics**: инвариант сохранён; зелёные `policy_classification_uses_only_domain_equality_no_ranges`, `policy_prioritizes_native_first_without_range_heuristics`.
- **dev endpoints compatibility**: совместимость сохранена; зелёные `v1_dev_peers_exposes_transport_snapshot`, `v1_peer_hello_accepts_and_classifies_native`, `v1_peer_hello_classifies_foreign_and_exposes_reject_counters`.

### Residual risks

- Блокирующих проблем не выявлено.
- Остаточный риск: parity подтверждена automated harness; для полного closeout полезен короткий multi-node smoke с churn bursts.

---

## Slice #7 Test Gate (reject-reason counter helper centralization)

Дата: 2026-04-24  
Исполнитель: `pwm-testing` (independent verification)

### Verdict

**PASS**

Slice #7 (`reject_reason_total` helper extraction в `pwmd`) прошел testing gate: полный `cargo test -p pwmd` зеленый, parity обновления `reject_reason_total` после centralization в `increment_reject_reason_total(...)` подтвержден, регрессий по tx-path/no-range/dev endpoints не выявлено.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### Reject reason parity checks (slice #7)

- **single-point reject counter update preserved**:
  - helper `increment_reject_reason_total(...)` инкрементирует `reject_reason_total[reason_label]` через `entry(...).or_insert(0) += 1` без изменения key/value семантики;
  - reject-path в `process_incoming_peer_hello(...)` по-прежнему выполняет `rejected_total += 1`, update reason counter и `warn`-лог в том же semantic порядке.
- **observed behavior unchanged in covered tests**:
  - `v1_peer_hello_classifies_foreign_and_exposes_reject_counters` зеленый: `rejected_total == 1` и `reject_reason_total["bad_signature"] == 1`;
  - `real_transport_tick_rejects_bad_signature_and_tracks_reason` зеленый: в runtime metrics подтвержден `reject_reason_total["bad_signature"] == 1`;
  - `v1_peer_hello_rejects_bad_signature_replay_network_genesis_and_malformed` зеленый: reject reasons по причинам валидации (bad_signature/replay_nonce/network_mismatch/genesis_mismatch/malformed) остаются совместимыми.

### Regression check

- **tx-path invariants**: регрессий не выявлено; зеленые `v1_tx_accepts_signed_init`, `v1_tx_accepts_regulatory_lo_zero_init`, `v1_tx_rejects_domain_mismatch`, `v1_tx_rejects_wrong_shard_for_sender_domain_hi`, `v1_tx_rejects_cross_shard_transfer_on_local_path`, prefilter guards (`reserve/witness/unknown`) и body-limit guard.
- **no-range heuristics**: инвариант сохранен; зеленые `policy_classification_uses_only_domain_equality_no_ranges`, `policy_prioritizes_native_first_without_range_heuristics`.
- **dev endpoints compatibility**: совместимость сохранена; зеленые `v1_dev_peers_exposes_transport_snapshot`, `v1_peer_hello_accepts_and_classifies_native`, `v1_peer_hello_classifies_foreign_and_exposes_reject_counters`.

### Residual risks

- Блокирующих проблем не выявлено.
- Остаточный риск: parity подтвержден на unit/integration harness и codepath inspection; для полного closeout полезен короткий multi-node smoke с churn bursts.

---

## Slice #6 Test Gate (class-state key helper centralization)

Дата: 2026-04-24  
Исполнитель: `pwm-testing` (independent verification)

### Verdict

**PASS**

Slice #6 (`class-state key helper centralization` в `pwmd`) прошел testing gate: полный `cargo test -p pwmd` зеленый, parity ключей class-state maps (`last_attempt_ms_by_class`/`last_result_by_class`) после helper centralization подтвержден, регрессий по tx-path/no-range/dev endpoints не выявлено.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### Class-state key parity checks (slice #6)

- **single-point key composition preserved for both maps**:
  - helper `compose_class_state_key(...)` используется как единая точка формирования ключа класса для обоих insert-path в `record_transport_attempt(...)`;
  - `last_attempt_ms_by_class` и `last_result_by_class` заполняются одним и тем же `class_state_key` (без split key logic между map paths);
  - helper возвращает `class_key.to_string()`, что сохраняет прежний wire/key контракт (`native`/`foreign`/`unknown`) для state maps.
- **observed behavior remains stable in gate run**:
  - `v1_dev_peers_exposes_transport_snapshot` зеленый; transport snapshot остаётся согласованным с class/result counters после tick;
  - `real_transport_tick_connects_seed_and_accepts_handshake` и `real_transport_tick_rejects_bad_signature_and_tracks_reason` зеленые; успешный/retryable transport paths сохраняют прежнюю state-update семантику;
  - `transport_scheduler_orders_native_and_respects_backoff`, `transport_retry_backoff_transitions_follow_envelope`, `transport_degraded_state_requires_persistent_underflow_ticks` зеленые; scheduler/backoff/degraded поведение не изменилось.

### Regression check

- **tx-path invariants**: регрессий не выявлено; зеленые `v1_tx_accepts_signed_init`, `v1_tx_accepts_regulatory_lo_zero_init`, `v1_tx_rejects_domain_mismatch`, `v1_tx_rejects_wrong_shard_for_sender_domain_hi`, `v1_tx_rejects_cross_shard_transfer_on_local_path`, prefilter guards (`reserve/witness/unknown`) и body-limit guard.
- **no-range heuristics**: инвариант сохранен; зеленые `policy_classification_uses_only_domain_equality_no_ranges`, `policy_prioritizes_native_first_without_range_heuristics`.
- **dev endpoints compatibility**: совместимость сохранена; зеленые `v1_dev_peers_exposes_transport_snapshot`, `v1_peer_hello_accepts_and_classifies_native`, `v1_peer_hello_classifies_foreign_and_exposes_reject_counters`.

### Residual risks

- Блокирующих проблем не выявлено.
- Остаточный риск: parity подтвержден на unit/integration harness и codepath inspection; для полного closeout полезен короткий multi-node smoke с churn bursts.

---

## Slice #5 Test Gate (class/result key helper extraction)

Дата: 2026-04-24  
Исполнитель: `pwm-testing` (independent verification)

### Verdict

**PASS**

Slice #5 (`transport key composition helper extraction` в `pwmd`) прошел testing gate: полный `cargo test -p pwmd` зеленый, parity ключей dial counters (`<class>:<result>`) после helper extraction подтвержден, регрессий по tx-path/no-range/dev endpoints не выявлено.

### Команды и результаты

- `cargo test -p pwmd` -> PASS (`55 passed; 0 failed`; `returncode=0`).

### Dial counter key parity checks (slice #5)

- **key format stability preserved (`<class>:<result>`)**:
  - helper `compose_class_result_key(...)` формирует ключ как `format!("{}:{}", class_key, result.as_label())` без изменения контрактного формата;
  - `v1_dev_peers_exposes_transport_snapshot` зеленый: подтверждены ключи `native:success` и `foreign:retryable_fail` в `dial_attempt_by_class_result`;
  - `real_transport_tick_connects_seed_and_accepts_handshake` зеленый: подтвержден ключ `native:success`;
  - `real_transport_tick_rejects_bad_signature_and_tracks_reason` и `real_transport_runaway_guard_stops_then_resumes_attempts` зеленые: подтвержден ключ `unknown:retryable_fail`.
- **counter semantics unchanged**:
  - `transport_scheduler_orders_native_and_respects_backoff`,
  - `transport_retry_backoff_transitions_follow_envelope`,
  - `transport_degraded_state_requires_persistent_underflow_ticks`,
  - `real_transport_reconnect_sets_retrying_then_disconnected_with_bounded_cooldown`.

### Regression check

- **tx-path invariants**: регрессий не выявлено; зеленые `v1_tx_accepts_signed_init`, `v1_tx_accepts_regulatory_lo_zero_init`, `v1_tx_rejects_domain_mismatch`, `v1_tx_rejects_wrong_shard_for_sender_domain_hi`, `v1_tx_rejects_cross_shard_transfer_on_local_path`, prefilter guards (`reserve/witness/unknown`) и body-limit guard.
- **no-range heuristics**: инвариант сохранен; зеленые `policy_classification_uses_only_domain_equality_no_ranges`, `policy_prioritizes_native_first_without_range_heuristics`.
- **dev endpoints compatibility**: совместимость сохранена; зеленые `v1_dev_peers_exposes_transport_snapshot`, `v1_peer_hello_accepts_and_classifies_native`, `v1_peer_hello_classifies_foreign_and_exposes_reject_counters`.

### Residual risks

- Блокирующих проблем не выявлено.
- Остаточный риск: parity подтвержден на unit/integration harness; для полного closeout полезен короткий multi-node smoke с churn bursts.
