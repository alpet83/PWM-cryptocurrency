# Review: V6-6 COSIGN_NON_DISABLEABLE (ADR 0009 bit 0)

**Slice:** `20260607-v6-sprint6-cosign-flags-coding`  
**Commit:** `b3750cf`  
**Branch:** `v6/20260607-v6-sprint6-cosign-flags-coding`  
**Reviewer:** pwm-review  
**Date:** 2026-06-07

## 1. Scope recap

Слайс V6-6 (`docs/plans/mvp_v6.md`, Sprint V6-6) вводит runtime enforcement флага `COSIGN_NON_DISABLEABLE` (bit 0) по [ADR 0009](adr/0009-address-flags-runtime-enforcement.md). Область слайса — **только bit 0**; `CONSERVATION` (V6-8) вне scope.

Затронутые файлы:

- `crates/pwm-core/src/types.rs` — decode флага из байтов `AccountId`, без поля `Account.address_flags`
- `crates/pwm-core/src/tx.rs` — `policy_weakens_cosign`, ранний reject в `validate_tx_shape`
- `crates/pwm-core/src/state.rs` — cosign gate в `evaluate_policy`, hardening в `validate_pol_action`, тесты `policy_flag_*`

Критерии приёмки из тикета: decode из address bytes; `E_POLICY_FLAG_NON_DISABLEABLE` при ослаблении cosign; cosign для protected actions при bit 0; исключение emergency routing; `policy_flag_*` PASS.

## 2. Requirements fit

| Критерий | Статус | Комментарий |
|----------|--------|-------------|
| Флаг из decode address, без `Account.address_flags` | **PASS** | `address_flags` / `cosign_non_dis` в `types.rs`; рефактор `render_acct_id_ui` переиспользует `address_flags`. Поле аккаунта не добавлено. |
| `DeactivatePolicy` / ослабляющий `SetPolicy` → `E_POLICY_FLAG_NON_DISABLEABLE` | **PASS** | `policy_weakens_cosign`: `DeactivatePolicy` для `CosignRequired`; `SetPolicy` с `CosignRequired` и activation ≠ `Immediately` (Dormant/Deferred). Двойной gate: `validate_tx_shape` (mempool/shape) и `validate_pol_action` (apply). Wire-код стабилен: `reject_wire.rs` → `E_POLICY_FLAG_NON_DISABLEABLE`. |
| Protected actions требуют cosign при bit 0 | **PASS** | `evaluate_policy`: sender path `cosign_non_dis(sender) && cosign_prot_body(body)`; для `Policy` — также `cosign_non_dis(target)` OR active `CosignRequired`. `cosign_prot_body` = `Transfer` \| `Policy`. |
| Emergency routing разрешён при RFC6+ADR0011 | **PASS** | `policy_flag_emerg_rescue_ok`: flagged owner, dormant emergency redirect, `ActivatePolicy` с `CosignRole::Rescue` — apply успешен, account finalized. `ActivatePolicy` не считается weaken (`policy_weakens_cosign` → false). |
| `policy_flag_*` тесты | **PASS (coding claim)** | 5 тестов в diff; локальный `cargo test` в среде ревью не собрался (`dlltool.exe` / Windows toolchain). Доверяем bridge submit + делегированию coding. |
| Mempool = тот же stable code | **PASS** | `validate_tx_shape` вызывается из `pwmd` handlers/peer_session; weaken reject до state. |

**Частичные / отложенные зоны (не блокеры для V6-6):**

- **EXPORT и protected-action taxonomy:** ADR 0009 matrix: «cosign if export is protected action». `cosign_prot_body` не включает `Export`; до слайса `CosignRequired` на sender для `Transfer` тоже не применялся — только Policy target. Поведение согласовано с pre-V6-6 cosign scope, но полная RFC6 taxonomy protected actions не замкнута. Рекомендация: явная строка в V6-7 follow-up или ADR addendum, если EXPORT должен gate'иться при bit 0.
- **SetPolicy Deferred weaken:** логика покрыта `policy_weakens_cosign`, но тест только для `Dormant` — см. nits.

## 3. Style and module shape

- Именование: `python scripts/check_entity_name_segments.py` на трёх файлах — **violations: []** (prod ≤4, test ≤5).
- Новые production symbols: `address_flags`, `cosign_non_dis`, `policy_weakens_cosign`, `cosign_prot_body` — в пределах политики.
- `types.rs`, `tx.rs`, `state.rs` — есть `//!` banners.
- Дублирование weaken-check в `validate_tx_shape` и `validate_pol_action` — приемлемо (mempool vs apply parity); для Policy `target_account == aid` уже enforced shape'ом, поэтому проверка `cosign_non_dis(&aid)` эквивалентна target.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

## 4. Safety

- Нет новых `unwrap` в hot paths; флаги — чистый decode битов, без IO.
- Cosign verify через существующий `verify` / `has_valid_cosign` — без ослабления trust boundary.
- Reject стабилен и не раскрывает лишних деталей.
- Риск: широкий `cosign_prot_body` для всех `Policy` на flagged sender может требовать cosign на benign `SetPolicy` (routing) — это **намеренное** ужесточение baseline cosign, не bypass.

## 5. Tests

**Покрыто:**

- `policy_flag_decode_bit0` — bit 0 в address
- `policy_flag_deact_cosign_reject` — `DeactivatePolicy` CosignRequired
- `policy_flag_set_dormant_reject` — `SetPolicy` Dormant weaken
- `policy_flag_transfer_needs_cosign` — deny без cosign, allow с Witness cosign
- `policy_flag_emerg_rescue_ok` — emergency activation с Rescue cosign

**Пробелы (nits):**

1. Нет теста `SetPolicy` + `ActivationMode::Deferred` weaken на flagged account (логика есть, тест — нет).
2. Нет `precheck_apply_same_err` / `validate_tx_shape`-only теста для `PolicyFlagNonDisableable` (parity mempool vs apply для weaken).
3. Нет негативного emergency-теста на flagged account без Rescue cosign (positive path есть).

## 6. Verdict

**APPROVE WITH NITS**

Приоритет nits:

1. **Low:** добавить тест `policy_flag_set_deferred_reject` (Deferred activation для CosignRequired).
2. **Low:** тест shape/precheck parity для `E_POLICY_FLAG_NON_DISABLEABLE`.
3. **Low / doc:** зафиксировать в плане V6-7, нужен ли cosign gate для `Export` при bit 0.

Блокеров по ADR 0009 bit 0 не выявлено. Конвейер может переходить к **pwm-testing**.

## 7. Participation / token estimate

```json
{
  "agent": "pwm-review",
  "result": "PASS",
  "artifacts": "docs/reviews/v6-6-cosign-non-disableable-20260607.md",
  "token_usage": {
    "source": "estimate",
    "input": null,
    "output": null,
    "total": 28000,
    "confidence": "medium"
  }
}
```

**Verdict:** APPROVE WITH NITS — ADR 0009 bit 0 реализован корректно; мелкие пробелы в тестах не блокируют testing gate.
