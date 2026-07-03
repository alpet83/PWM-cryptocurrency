# Review: V6-2 slice 2 — ActivatePolicy `activation_target` wire

**Ticket:** `tasks/20260605-v6-s2-slice2-activate-policy-wire.json`  
**Worktree:** `P:/opt/docker/pwm-protocol-worktrees/v6-sprint2-core-model`  
**Branch:** `v6/20260605-v6-sprint2-core-model`  
**Reviewer:** pwm-review  
**Date:** 2026-06-05  
**Spec:** ADR 0011, `docs/rfc/addenda/v6-rfc6-activate-policy-activation-target.md`  
**Scope claim:** wire/signing/serde only — no fee=0 enforcement, no apply semantics

---

## 1. Scope recap

Слайс расширяет `PolicyAction::ActivatePolicy` полем `activation_target: Option<AccountId>` по ADR 0011 и RFC 6 addendum (V6). Заявленные артефакты:

| File | Change |
|------|--------|
| `crates/pwm-core/src/tx.rs` | enum field, signing bytes, JSON serde, unit tests |
| `crates/pwm-core/src/state.rs` | pattern-only `..` на match arms + `activation_target: None` в тестах |
| `crates/pwm-cli/src/cmd_tx.rs` | compile fix: `activation_target: None` |
| `crates/pwmd/src/snapshot/types.rs` | `PolicyActionV2` hex mapping to/from core |

Acceptance из тикета: optional field с omitted/null для legacy JSON; signing включает target when `Some`; JSON round-trip tests; `cargo check --workspace`. Явно **вне scope**: fee=0 validation, apply/evacuation, CLI `--activation-target` (V6-7).

---

## 2. Requirements fit

**Met (wire slice):**

- `ActivatePolicy { policy_id, activation_target: Option<AccountId> }` с `#[serde(default, skip_serializing_if = "Option::is_none")]` — omitted/null → `None`.
- Signing: `push_policy_action_signing` вызывает `push_opt_account_id` (tag `0` = absent, `1` + 32 bytes = present) — согласовано с `init_v4.rescue_address`.
- Legacy JSON без поля десериализуется (`pol_act_tgt_json_legacy`).
- Round-trip с `Some(tgt)` и signing diff None vs Some (`pol_activate_target_json_roundtrip`, `pol_activate_target_signing_diff`).
- Snapshot v2: hex encode/decode через `policy_action_to_v2` / `policy_action_from_v2`, `#[serde(default)]` на optional hex string.
- `cargo check --workspace` — OK (pre-existing pwmd warnings only).
- `state.rs` / apply не читает `activation_target` — корректно для wire-only.
- `validate_tx_shape` по-прежнему требует `fee > 0` для всех Policy tx — **ожидаемо** (fee=0 enforcement отложен).

**Deferred by design (not gaps for this slice):**

- ADR 0011 fee=0, target required/mismatch/not-allowed rejects.
- Apply-time evacuation, `finalized`, emergency cosign gates with target binding.
- `pwm-cli tx-policy-activate --activation-target`.

**Minor coverage gap (nit):**

- ADR 0011 Consequences упоминает «Snapshot/policy tx serde tests in V6-2»; dedicated unit test для snapshot v2 hex round-trip `activation_target` отсутствует. Core JSON покрыт в `pwm-core`; mapping в `types.rs` тривиален и зеркалит `rescue_address`, но явного regression test нет.

**Signing backward compatibility (nit, document):**

- Pre-slice signing для `ActivatePolicy` был `[tag=1][policy_id]`. Post-slice даже при `activation_target: None` добавляется `[0]` через `push_opt_account_id`. Подписи старых ActivatePolicy tx **не совпадут** с новым canonical message. Для V6 devnet/testnet это согласуется с additive wire в ADR; normative описание signing layout в RFC 7 пока не обновлено — рекомендуется зафиксировать в follow-up spec slice.

---

## 3. Style and module shape

- `python scripts/check_entity_name_segments.py` на всех четырёх путях: **violations: []** (prod ≤4, test ≤5).
- Reuse `push_opt_account_id` вместо дублирования — хорошо.
- Module banners (`//!`) уже были; slice не ухудшает.
- Test names `pol_activate_target_*`, `pol_act_tgt_json_legacy` — в budget (≤5 segments).
- Diff минимален; state/cli правки — mechanical pattern updates.

### Wire JSON / u128

**Scope:** yes — `SignedTx` / `PolicyAction` участвуют в block/snapshot JSON, которым обмениваются узлы. Новое поле — `AccountId` (`[u8; 32]`, JSON byte array в tx wire; hex string в snapshot v2). **Новых `u128` полей нет.** Policy `fee` по-прежнему через `ser_json_u128` (decimal string) — без регрессии.

**u128 check:** not applicable for the new field. Existing policy fee encoding unchanged and tested (`policy_tx_json_fee_str`).

**Protocol semver:** additive optional JSON field + signing extension; `PWM_PROTOCOL_VERSION` bump не требуется для handshake (no `NodeHello` / `PeerWireMsg` change). Orchestrator may note «no wire compatibility impact» on peer handshake version.

---

## 4. Safety

- No new panics in hot paths; `push_opt_account_id` / signing helpers unchanged in error behavior.
- No apply-side trust boundary changes — target ignored at state layer (wire-only).
- `validate_tx_shape` не валидирует semantics `activation_target` — acceptable until V6-7; precludes premature reject/inject of emergency-only rules.
- Snapshot hex decode uses existing `hex_v2` with path context — consistent error surface.

---

## 5. Tests

**Present:**

- `pol_activate_target_json_roundtrip` — Some(target) JSON equality.
- `pol_act_tgt_json_legacy` — omitted field → None.
- `pol_activate_target_signing_diff` — None vs Some signing bytes differ.
- Existing `policy_signing_changes_by_action` updated with `activation_target: None`.
- State test fixtures compile with explicit `None`.

**Missing (low priority for wire slice):**

- Snapshot v2 `policy_action_to_v2` ↔ `from_v2` with `activation_target` hex.
- Test that legacy **signature** (pre-slice signing layout) is intentionally invalid post-upgrade — optional doc/test for V6 migration story.

**Not run by reviewer:** full `cargo test --workspace` (pwm-coding claimed PASS; spot-check `cargo test -p pwm-core pol_activate*` and `pol_act_tgt*` — green).

---

## 6. Verdict

**Approve with nits.**

Приоритет nits:

1. **Low:** добавить snapshot v2 round-trip test для `activation_target` hex (или явно перенести в следующий snapshot slice).
2. **Low:** зафиксировать в RFC 7 / signing spec, что V6 ActivatePolicy signing включает optional-account-id tag даже when JSON omits field.

Блокеров для конвейера `pwm-testing` на заявленном wire-only scope нет.

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260605-v6-s2-slice2-activate-policy-wire-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 18000
  confidence: medium
```

---

**Verdict:** APPROVE_WITH_NITS
