# Review: V6-7 emergency activation_target + evacuation + pwm-cli prepared activation

**Slice:** `20260607-v6-sprint7-emergency-sweep-coding`  
**Branch:** `v6/20260607-v6-sprint7-emergency-sweep-coding`  
**Commit:** `85241e9`  
**Reviewer:** pwm-review  
**Date:** 2026-06-07

## 1. Scope recap

Слайс закрывает **MVP v6 Sprint V6-7** ([`docs/plans/mvp_v6.md`](../plans/mvp_v6.md)): runtime apply для **ADR 0011** — fee-free `ActivatePolicy`, обязательный `activation_target` для `routing.emergency_redirect`, эвакуация spendable `balance_pwm` на rescue в том же `apply_tx`, стабильные `E_POLICY_ACTIVATION_*`, CLI-подготовка signed activation при `tx-init`.

**Затронутые файлы (commit `85241e9`):**

- `crates/pwm-core/src/state.rs`, `crates/pwm-core/src/tx.rs`
- `crates/pwm-cli/src/cli_cmd.rs`, `cli_dispatch.rs`, `cmd_tx.rs`, `tests/mod.rs`

**Вне diff, но релевантно:** `crates/pwm-core/src/reject_wire.rs` уже мапит четыре V6-кода активации на wire.

## 2. Requirements fit

| Требование (ADR 0011 / RFC6 / ticket) | Статус | Комментарий |
|--------------------------------------|--------|-------------|
| `ActivatePolicy` fee MUST be 0 | **OK** | `validate_tx_shape` → `PolicyActivationFeeMustBeZero`; Set/Deactivate по-прежнему требуют fee > 0 |
| Emergency: `activation_target` required | **OK** | Shape + `validate_pol_action` |
| `activation_target == rescue_address` | **OK** | `PolicyActivationTargetMismatch` |
| Non-emergency + non-null target | **OK** | `PolicyActivationTargetNotAllowed` |
| Same-shard only (cross-shard reject) | **OK** | `same_hi_domain` → `PolicyRoutingDenied` (не отдельный activation-код — допустимо по ADR) |
| Rescue cosign before apply | **OK** | `validate_pol_action` + `has_role_cosign(Rescue)` |
| One `apply_tx`: activate + finalize + sweep balance | **OK** | `apply_policy_action` затем inline credit target / zero sender |
| No `ProtocolSweep` tx type | **OK** | Новый тип не введён |
| Reject codes `E_POLICY_ACTIVATION_*` | **OK** | `reject_wire.rs` + unit-тесты маппинга |
| `pwm-cli --activation-target`, default fee 0 | **OK** | `tx-policy-activate` |
| `--save-activation-tx` + `--activation-tx` roundtrip | **Частично** | Файловый путь реализован; см. nit по RFC 10 wallet |
| `tx-init` prepared activation | **Частично** | `build_init_activation` + rescue cosign из wallet при совпадении id; **нет** записи в wallet YAML |
| Reuse Transfer debit/credit helpers | **Частично** | Семантика debit/credit совпадает, но логика **дублирована** в ветке `Policy`, без общего helper и без `touch_acct_mrks` на получателе |
| `emergency_activation_*` tests | **OK** | sweep, fee, target required/mismatch, cross-shard |
| Signing / JSON round-trip `activation_target` | **OK** | `pol_activate_target_json_roundtrip`, `pol_activate_target_signing_diff`, `prepared_activation_roundtrip` |

**Пробелы (не блокеры для core apply, но traceability):**

1. **RFC 10 wallet schema** (`prepared_policy_activation` в `accounts[]`) не реализован — только standalone JSON через `--save-activation-tx`. Тикет формулирует «wallet **and/or** файл»; для полного соответствия RFC 10 addendum §4 (default = wallet) нужен follow-up.
2. **CONSERVATION / pending outgoing** (ADR 0011 → ADR 0009) при эвакуации не проверяется — ожидаемо отложено на **V6-8** per `mvp_v6.md`; зафиксировать в E2E V6-10.
3. **`maybe_rescue_cosign`** молча пропускает cosign, если rescue нет в wallet — prepared tx может уйти без rescue-подписи и отклониться нодой (`PolicyEmergencyCosignRequired`). Документировать в operator runbook / TUI copy (ADR consequences).

## 3. Style and module shape

- **`check_entity_name_segments.py`** на заявленных путях: **violations = []** (prod ≤4, test ≤5).
- Идентификаторы в норме: `emergency_act_target`, `validate_pol_action`, `build_init_activation`, тесты `emergency_activation_*`.
- Module banners `//!` на `state.rs`, `cmd_tx.rs` — на месте.
- Новая логика в `apply_tx` Policy-ветке компактна (~20 строк); не раздувает façade.
- **Замечание (low):** эвакуация использует `expect("activation target validated")` после `validate_pol_action` — паника теоретически недостижима при согласованном state; приемлемо для текущего стиля crate.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice). Изменения касаются apply-path, CLI JSON signed tx (существующий envelope) и shape-validation; `PeerWireMsg` / sync wire не тронуты. Поле `fee` в Policy остаётся `u128` в уже принятой serde-модели signed tx.

## 4. Safety

- **Cosign gate:** неверный rescue cosign не мутирует state (тест `policy_emerg_act_bad_cosign` + rollback assertions) — хорошо.
- **Fee waiver:** только для `ActivatePolicy`; злоупотребление activation_target на non-emergency отсекается на shape.
- **Cross-shard activation_target:** отклоняется до мутации баланса.
- **Finalize ordering:** `finalized = true` выставляется до sweep — повторная активация следует V4 irreversibility.
- **Trust boundary CLI:** `load_signed_tx` / `save_signed_tx` — локальные пути оператора; без новых сетевых поверхностей.
- **Эвакуация всего `balance_pwm`:** staked не трогается (соответствует ADR «spendable balance_pwm»). Риск double-spend через pending conservation — вне scope V6-7.
- **Hardcoded nonce=1** в `build_init_activation` корректен для сценария «init (nonce 0) затем activation» в одном `tx-init` run.

## 5. Tests

**Покрыто:**

- Core: `emergency_activation_sweep_ok`, `fee_reject`, `target_required`, `target_mismatch`, `cross_reject`; существующие emergency cosign/finalize тесты обновлены под `activation_target`.
- `tx.rs`: fee rules per action arm, `pol_act_tgt_non_emerg`, signing/json round-trips.
- CLI: parse `--save-activation-tx`, `--activation-tx` без wallet, `--activation-target`; `prepared_activation_roundtrip` (unit в `cmd_tx`).

**Пробелы (для pwm-testing / follow-up):**

- Нет integration-теста полного `run_tx_init` + `--save-activation-tx` с emergency policy (только parse-тест с `sender_filter:dormant`).
- Нет теста wallet-persist `prepared_policy_activation` (схема не реализована).
- Нет теста CONSERVATION + emergency sweep (V6-8).
- Локальный `cargo test` в среде ревьюера не собрался (`dlltool.exe` / Windows toolchain) — полагаться на отчёт coding + прогон pwm-testing.

## 6. Verdict

**APPROVE_WITH_NITS**

Приоритетные nits (добивка без решения владельца по продукту):

1. **Medium:** RFC 10 — persist prepared activation в wallet `accounts[]`, не только файл (или явно задокументировать file-only defer в RFC/ticket).
2. **Low:** Вынести или явно вызвать transfer-эквивалент debit/credit (в т.ч. `touch_acct_mrks` на target) вместо inline sweep в Policy-ветке — ticket AC «Reuse Transfer helpers».
3. **Low:** CLI: предупреждение, если `build_init_activation` не смог добавить rescue cosign.
4. **Low:** Integration-тест `tx-init` + emergency dormant + save file (pwm-testing).

Блокеров по ADR 0011 core apply, reject codes и отсутствию `ProtocolSweep` не выявлено.

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts: docs/reviews/v6-sprint7-emergency-sweep-coding-review-20260607.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 28000
  confidence: medium
```

**Verdict (one-liner):** `APPROVE_WITH_NITS` — core ADR 0011 apply + rejects + CLI file prepared activation OK; nits: RFC10 wallet persist, transfer-helper reuse, rescue-cosign warn.
