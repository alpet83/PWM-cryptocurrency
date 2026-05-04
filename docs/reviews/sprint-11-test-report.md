# Sprint 11 Test Report (Slice 3-5 coding-pass checks)

Дата: 2026-04-26  
Этап: Slice 3 mode-bound guard policy

## Executed Checks

- `cargo fmt --all` — PASS.
- `cargo check -p pwmd` — PASS.
- `cargo test -p pwmd v1_tx_rejects_wrong_shard_for_sender_domain_hi` — PASS (1 test).
  - Подтверждено: shard-enforced local guard действует в explicit mode.
- `cargo test -p pwmd v1_tx_allows_wrong_shard_in_relay_baseline_mode` — PASS (1 test).
  - Подтверждено: в relay baseline нет shard-enforced reject для wrong-shard sender.
- `cargo test -p pwmd v1_tx_rejects_unknown_recipient_prefilter_in_explicit_mode` — PASS (1 test).
  - Подтверждено: baseline recipient prefilter остается активным и в explicit mode (`400 BAD_REQUEST`).

## Verdict

- Slice 3 mode-bound guard policy checks — PASS:
  - baseline recipient prefilter подтвержден как always-on;
  - shard-enforced local guards подтверждены как explicit-mode only;
  - scope-ограничения соблюдены (без wire/API expansion, без unrelated refactor).

## Notes

- Прогон targeted и достаточный для Slice 3 coding-pass (без полного regression matrix; full testing остаётся за `pwm-testing`).

---

## Slice 4 Coding-pass Checks (storage namespace migration policy)

Дата: 2026-04-26  
Этап: Slice 4 storage namespace migration policy

### Executed Checks

- `cargo fmt --all` — PASS.
- `cargo check -p pwmd` — PASS.
- `cargo test -p pwmd storage_namespace_is_domain_target_with_alias_compat_mapping` — PASS (1 test).
  - Подтверждено: explicit mode использует domain target namespace (`domain-hi-0xNN`), alias mode сохраняет legacy mapping (`shard-a|shard-b`).
- `cargo test -p pwmd resolve_runtime_identity_uses_alias_mapping_for_shard` — PASS (1 test).
  - Подтверждено: compat alias path (`--shard`) остается детерминированным и без hard-break.

### Verdict

- Slice 4 namespace migration policy checks — PASS:
  - domain-based namespace target реализован для explicit identity mode;
  - legacy alias namespace mapping сохранен для compat сценария;
  - scope-ограничения соблюдены (без wire/API expansion, без unrelated refactor).

### Notes

- Прогон targeted и достаточный для Slice 4 coding-pass; полный regression matrix остается за `pwm-testing`.

---

## Slice 5 Coding-pass Checks (conformance docs/test baseline sync)

Дата: 2026-04-26  
Этап: Slice 5 conformance docs and test baseline

### Executed Checks

- `rg -n "Slice 5/6|Slice 6/6|Artifact gate|Scope gate" docs/reviews/sprint-11-checklist.md` — PASS.
  - Подтверждено: checklist синхронизирован, Slice 5 задачи отмечены выполненными, global scope/artifact gates закрыты.
- `rg -n "Slice 5/6 completed|SLICE 6 REVIEW/TEST PASS|Artifact gate" docs/reviews/sprint-11-status-note.md` — PASS.
  - Подтверждено: status-note отражает завершение Slice 5 и переход к Slice 6 review/testing фазе.
- `rg -n "Slice 5 coding-pass|Conformance evidence|Для Slice 6" docs/reviews/sprint-11-review-report.md` — PASS.
  - Подтверждено: review-report переведен на Slice 5 verdict и зафиксирован handoff на Slice 6.
- `rg -n "Sprint 11 migration track|relay-baseline mode by default|explicit domain mode|alias mode \\(`--shard A\\|B`\\)" README.md` — PASS.
  - Подтверждено: README синхронизирован с текущей migration policy и storage namespace behavior.

### Verdict

- Slice 5 conformance docs/test baseline checks — PASS:
  - operator docs и sprint-11 review artifacts синхронизированы с runtime policy после Slice 4;
  - README drift после Slice 4 устранен;
  - scope-ограничения соблюдены (без wire/API expansion, без unrelated refactor).

### Notes

- Прогон целевой и документарный; независимый verification pass по Slice 6 остается за `pwm-testing`/`pwm-review`.

---

## Independent Testing Pass (Slice 3)

Дата: 2026-04-26  
Роль: `pwm-testing` (independent verification after coding-pass)

### Executed Commands and Results

1. `cargo fmt --all`  
   **PASS** (`exit code 0`).

2. `cargo test -p pwmd v1_tx_rejects_wrong_shard_for_sender_domain_hi`  
   **PASS** (`exit code 0`, 1 test passed).

3. `cargo test -p pwmd v1_tx_allows_wrong_shard_in_relay_baseline_mode`  
   **PASS** (`exit code 0`, 1 test passed).

4. `cargo test -p pwmd v1_tx_rejects_reserve_recipient_prefilter`  
   **PASS** (`exit code 0`, selected tests passed):
   - `v1_tx_rejects_reserve_recipient_prefilter`
   - `v1_tx_rejects_reserve_recipient_prefilter_in_explicit_mode`

5. `cargo test -p pwmd v1_tx_rejects_witness_recipient_prefilter`  
   **PASS** (`exit code 0`, selected tests passed):
   - `v1_tx_rejects_witness_recipient_prefilter`
   - `v1_tx_rejects_witness_recipient_prefilter_in_explicit_mode`

6. `cargo test -p pwmd v1_tx_rejects_unknown_recipient_prefilter`  
   **PASS** (`exit code 0`, selected tests passed):
   - `v1_tx_rejects_unknown_recipient_prefilter`
   - `v1_tx_rejects_unknown_recipient_prefilter_in_explicit_mode`

7. `cargo test -p pwmd v1_tx_rejects_reserve_recipient_prefilter_in_explicit_mode`  
   **PASS** (`exit code 0`, 1 test passed).

8. `cargo test -p pwmd v1_tx_rejects_witness_recipient_prefilter_in_explicit_mode`  
   **PASS** (`exit code 0`, 1 test passed).

9. `cargo test -p pwmd v1_tx_rejects_unknown_recipient_prefilter_in_explicit_mode`  
   **PASS** (`exit code 0`, 1 test passed).

### Slice 3 Mandatory Checks Verdict

- Mode-bound shard guard in explicit mode — **PASS**.
- Absence of shard-enforced reject in relay baseline mode — **PASS**.
- Always-on recipient prefilter in explicit mode and relay baseline for invalid classes (`reserve`/`witness`/`unknown`) — **PASS**.

### Residual Risks

- Sanity scope is targeted; full cross-crate regression (`workspace cargo test`) was not run in this pass.
- Slice 3 verification focused on tx-guard policy only; broader integration matrix остаётся в зоне `pwm-testing`.

---

## Independent Testing Pass (Slice 4)

Дата: 2026-04-26  
Роль: `pwm-testing` (independent verification after coding-pass)

### Executed Commands and Results

1. `cargo fmt --all`  
   **PASS** (`exit code 0`).

2. `cargo test -p pwmd storage_namespace_is_domain_target_with_alias_compat_mapping`  
   **PASS** (`exit code 0`, 1 test passed).

3. `cargo test -p pwmd resolve_runtime_identity_uses_alias_mapping_for_shard`  
   **PASS** (`exit code 0`, 1 test passed).

4. `cargo test -p pwmd v1_status_reports_alias_state_namespace_for_shard`  
   **PASS** (`exit code 0`, 1 test passed).

5. `cargo test -p pwmd v1_status_reports_explicit_domain_state_namespace`  
   **PASS** (`exit code 0`, 1 test passed).

6. `cargo test -p pwmd v1_head_returns_tip_json`  
   **PASS** (`exit code 0`, 1 test passed).

7. `cargo test -p pwmd v1_status_reports_loading_and_head_returns_503`  
   **PASS** (`exit code 0`, 1 test passed).

8. `cargo test -p pwmd v1_status_reports_ready_degraded_after_snapshot_error`  
   **PASS** (`exit code 0`, 1 test passed).

### Slice 4 Mandatory Checks Verdict

- storage namespace policy:
  - explicit mode -> domain-based namespace target (`domain-hi-0xNN`) — **PASS**;
  - alias mode (`--shard` compat) -> legacy mapping (`shard-a|shard-b`) — **PASS**.
- backward compat path (`--shard` alias mapping) — **PASS**.
- базовые status/head пути, связанные с namespace reporting — **PASS**:
  - `/v1/status` сообщает ожидаемый `state_namespace` в alias и explicit режимах;
  - `/v1/head` сохраняет baseline поведение (`200 OK` when ready, `503` during loading).

### Residual Risks

- Прогон targeted и достаточный для Slice 4 policy-gates, но не заменяет полный `cargo test --workspace`.
- Проверка ограничена `pwmd`; межкрейтовые интеграционные эффекты не переоценивались в этом проходе.

---

## Independent Testing Pass (Slice 5)

Дата: 2026-04-26  
Роль: `pwm-testing` (independent verification after coding-pass)

### Scope

- Conformance/docs baseline pass для Sprint 11 Slice 5:
  - проверка согласованности `checklist/status/review/test + README` с policy Sprint 11;
  - короткий targeted smoke confirm для ключевых runtime claims.
- Ограничения соблюдены:
  - без product-code изменений;
  - sprint-10 артефакты не затрагивались.

### Executed Commands and Results

1. `rg -n "relay mode = default|shard-support = explicit domain config only|--shard = deprecated compat alias|slices 0..6" docs/reviews/sprint-11-checklist.md`  
   **PASS**.
   - Подтверждено: checklist фиксирует Sprint 11 policy и единую slice-структуру `0..6`.

2. `rg -n "SLICE 6 REVIEW/TEST PASS|deprecated compat alias|slices 0..6" docs/reviews/sprint-11-status-note.md`  
   **PASS**.
   - Подтверждено: status-note отражает post-Slice 5 состояние и переход к Slice 6 review/testing phase.

3. `rg -n "relay baseline по умолчанию|shard-enforced только в explicit domain mode|deprecated compat path|Conformance evidence" docs/reviews/sprint-11-review-report.md`  
   **PASS**.
   - Подтверждено: review-report согласован с migration policy и Slice 5 conformance verdict.

4. `rg -n "relay-baseline mode by default|shard-enforced behavior is activated only for explicit domain config|--shard remains deprecated compat alias|explicit domain mode ->|alias mode" README.md`  
   **PASS**.
   - Подтверждено: README консистентен с policy Sprint 11 и namespace mapping claims.

5. `cargo test -p pwmd v1_tx_allows_wrong_shard_in_relay_baseline_mode`  
   **PASS** (`exit code 0`, 1 test passed).
   - Smoke confirm: relay baseline не включает shard-enforced reject для wrong-shard sender.

6. `cargo test -p pwmd storage_namespace_is_domain_target_with_alias_compat_mapping`  
   **PASS** (`exit code 0`, 1 test passed).
   - Smoke confirm: explicit mode использует `domain-hi-0xNN`, alias mode сохраняет `shard-a|shard-b`.

### Slice 5 Independent Verdict

- **PASS**: docs/test baseline для Slice 5 согласован между `sprint-11-checklist`, `sprint-11-status-note`, `sprint-11-review-report`, `sprint-11-test-report` и `README.md`.
- **PASS**: targeted smoke confirm не выявил расхождений между doc claims и текущим runtime behavior по ключевым migration assertions.

### Residual Risks

- Проверка targeted; полный `cargo test --workspace` в рамках этого pass не выполнялся.
- Runtime smoke ограничен двумя policy-critical assertions (relay baseline guard semantics и namespace mapping).

---

## Slice 6 Closeout Prep (coding-pass side)

Дата: 2026-04-26  
Этап: финальный coding-pass check set (без расширения scope)

### Executed Checks

1. `rg -n "relay (mode|baseline).*(default|по умолчанию)|explicit domain.*shard-enforced|deprecated compat alias|domain.*namespace|alias.*compat|Slice 6|closeout|verdict" docs/reviews/sprint-11-checklist.md docs/reviews/sprint-11-status-note.md docs/reviews/sprint-11-review-report.md docs/reviews/sprint-11-test-report.md README.md`  
   **PASS**.
   - Подтверждено: policy формулировки консистентны в checklist/status/review/test и README:
     - relay baseline default;
     - explicit domain => shard-enforced;
     - storage namespace policy = domain target + alias compat mapping;
     - `--shard` остается deprecated compat alias.

2. `cargo fmt --all`  
   **PASS** (`exit code 0`).

3. `cargo check -p pwmd`  
   **PASS** (`exit code 0`).

4. `cargo test -p pwmd v1_tx_allows_wrong_shard_in_relay_baseline_mode`  
   **PASS** (`exit code 0`, 1 test passed).
   - Smoke confirm: relay baseline по-прежнему не включает shard-enforced reject.

5. `cargo test -p pwmd storage_namespace_is_domain_target_with_alias_compat_mapping`  
   **PASS** (`exit code 0`, 1 test passed).
   - Smoke confirm: explicit mode использует `domain-hi-0xNN`, alias mode сохраняет `shard-a|shard-b`.

### Slice 6 Coding-pass Closeout Verdict

- **PASS**: финальный coding-pass check set достаточный и не раздут.
- **PASS**: Sprint 11 coding-pass сторона готова к независимому final testing/review verdict.
- Ограничения соблюдены: без новых product features, sprint-10 артефакты не изменялись.

### Handoff

- `pwm-testing`: выполнить финальный независимый targeted+sanity pass и подтвердить отсутствие policy drift.
- `pwm-review`: проверить evidence-консистентность и вынести итоговый sprint closeout verdict.

---

## Independent Closeout Pass (Slice 6 final)

Дата: 2026-04-26  
Роль: `pwm-testing` (independent closeout verification)

### Scope

- Final independent targeted+sanity pass для Sprint 11 closeout verdict.
- Ограничения соблюдены:
  - без product-code изменений;
  - sprint-10 артефакты не затрагивались.

### Executed Commands and Results

1. `cargo test -p pwmd v1_tx_allows_wrong_shard_in_relay_baseline_mode`  
   **PASS** (`exit code 0`, 1 test passed).
2. `cargo test -p pwmd v1_tx_rejects_wrong_shard_for_sender_domain_hi`  
   **PASS** (`exit code 0`, 1 test passed).
3. `cargo test -p pwmd v1_tx_rejects_reserve_recipient_prefilter`  
   **PASS** (`exit code 0`, 2 tests passed: baseline + explicit mode variant).
4. `cargo test -p pwmd v1_tx_rejects_witness_recipient_prefilter`  
   **PASS** (`exit code 0`, 2 tests passed: baseline + explicit mode variant).
5. `cargo test -p pwmd v1_tx_rejects_unknown_recipient_prefilter`  
   **PASS** (`exit code 0`, 2 tests passed: baseline + explicit mode variant).
6. `cargo test -p pwmd storage_namespace_is_domain_target_with_alias_compat_mapping`  
   **PASS** (`exit code 0`, 1 test passed).
7. `cargo test -p pwmd resolve_runtime_identity_uses_alias_mapping_for_shard`  
   **PASS** (`exit code 0`, 1 test passed).
8. `cargo test -p pwmd v1_status_reports_explicit_domain_state_namespace`  
   **PASS** (`exit code 0`, 1 test passed).
9. `cargo test -p pwmd v1_head_returns_tip_json`  
   **PASS** (`exit code 0`, 1 test passed).
10. `cargo test -p pwmd v1_status_reports_loading_and_head_returns_503`  
    **PASS** (`exit code 0`, 1 test passed).

### Closeout Verdict

- Policy-critical runtime assertions — **PASS**:
  - relay baseline default behavior подтвержден;
  - explicit shard-enforced behavior подтвержден;
  - always-on recipient prefilter подтвержден;
  - storage namespace domain-target + alias compat mapping подтвержден.
- Quick `pwmd` sanity subset — **PASS** (status/head readiness paths без regression drift).
- Hang watchdog: **not triggered**.
- Cleanup: **yes** (после run-check все spawned процессы завершены, `alive=false`).

### Residual Risks

- Прогон целевой и sanity-уровня; полный `cargo test --workspace` не выполнялся в этом closeout pass.
- Sanity subset ограничен `pwmd`; межкрейтовая интеграция (`pwm-core`/`pwm-cli`) в этом раунде не переоценивалась.
