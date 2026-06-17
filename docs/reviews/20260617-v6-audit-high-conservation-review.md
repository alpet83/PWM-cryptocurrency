# Review: V6 audit HIGH-001/002 conservation fix

**Date:** 2026-06-17  
**Ticket:** `tasks/20260617-v6-audit-high-conservation.json`  
**Audit ref:** `docs/reviews/20260616-v6-mvp-rust-code-audit-review.md` (HIGH-001, HIGH-002)  
**Slice:** `crates/pwm-core/src/state.rs` (+ unit tests in `#[cfg(test)]`)  
**Reviewer:** pwm-review

---

## 1. Scope recap

Закрытие двух high-находок V6 pre-publication audit в подсистеме CONSERVATION delayed transfer:

| Finding | Claim |
|---------|--------|
| **HIGH-001** | Конфликт `pending_conservation` с balance-affecting tx (`Export`, `Stake`, …) без резерва → silent drain loss |
| **HIGH-002** | `drain_conservation_at_height` молча отбрасывал failed row (`let _ = apply_due_conservation(...)`) |

Тикет выбрал простой контракт: **reject** конфликтующих tx при активной `pending_conservation` для sender (вместо balance reserve). Регрессии: `conservation_export_race_reject`, `conservation_stake_race_reject`, `conservation_drain_insufficient_requeue`. Emergency evac должен остаться корректным (ADR 0011 + cross-ref ADR 0009).

Затронут один production-файл; wire / snapshot schema не менялись.

---

## 2. Requirements fit

### HIGH-001 — reject conflicting spend

**Implemented.** Перед `match &tx.body` добавлены:

- `has_pending_conserv(sender)` — любая pending row для `computed_account_id()`;
- `pending_tx_conflict(body)` — `true` для всех balance/nonce-affecting вариантов, кроме:
  - обычного `Transfer` (второй conservation transfer по-прежнему режется в conservation arm, ~440–441);
  - `Policy::ActivatePolicy` только для `RoutingEmergencyRedirect` (`pending_pol_allowed`).

При конфликте → стабильный `TxError::ConservationPendingExists` (тот же код, что для второго conservation transfer).

**Сценарий audit закрыт:** enqueue conservation `Transfer` → `Export` или `Stake` с тем же nonce → reject до debit/nonce bump; баланс и nonce отправителя неизменны; row остаётся в очереди.

**Emergency path (ADR 0011 §Emergency routing binding; ADR 0009 §Interaction):** emergency activation **не** блокируется pre-check; в Policy arm по-прежнему `pending_conservation.retain(|row| row.sender != id)` + balance evac. Существующий тест `conservation_emergency_cancels_pending` подтверждает: activation проходит, pending очищается, drain на height 20 не кредитует получателя отменённого transfer.

**Gap (non-blocking):** нет отдельного регрессионного теста на `Unstake` / non-emergency `Policy` при pending — логика покрыта `_ => true` в `pending_tx_conflict`, но только Export и Stake зафиксированы в новых тестах.

### HIGH-002 — drain failure observability + retain

**Implemented.** `drain_conservation_at_height` заменил silent drop на:

```text
match apply_due_conservation(row.clone(), ...) {
  Ok(()) => {}
  Err(err) => { eprintln!(...); remaining.push(row); }
}
```

**Сценарий audit закрыт:** при `Insufficient` (симулировано обнулением balance между enqueue и drain) row остаётся в `pending_conservation`; повторный drain после восстановления balance успешно применяет transfer. Тест `conservation_drain_insufficient_requeue` покрывает полный цикл.

**Consensus:** requeue зависит только от детерминированного `apply_due_conservation` и порядка обхода `pending` — все ноды получают одинаковый `remaining`. `eprintln!` — side-effect на stderr, на state не влияет.

**Partial vs audit aspirational fix:** audit предлагал «structured log/metric» или evidence path; реализация — `eprintln!` с полями sender/nonce/execute_at/height/err. Это удовлетворяет acceptance «не silent» и audit minimum «never drop without logging reason», но не operator-grade structured logging в pwmd.

**Residual behavior:** перманентная ошибка drain (например, `NoAccount` после удаления аккаунта) → бесконечный requeue каждый seal height. Детерминированно и консенсусно, но без эскалации; улучшение относительно silent drop, не полное «evidence/reject path».

### Acceptance criteria (ticket)

| Criterion | Status |
|-----------|--------|
| HIGH-001 no silent race loss | Met (reject path + tests) |
| HIGH-002 retain/requeue + test | Met |
| `cargo test -p pwm-core --lib` conservation | Met (8 `conservation_*` tests, incl. 3 new) |
| Scope: pwm-core only | Met |

---

## 3. Style and module shape

- Новые production helpers: `pending_pol_allowed`, `pending_tx_conflict`, `has_pending_conserv` — все ≤4 snake_case сегмента.
- `python scripts/check_entity_name_segments.py crates/pwm-core/src/state.rs` → **violations: []**.
- Diff локализован: ~35 строк production, ~140 строк tests; без рефакторинга соседних arms.
- Module banner `//!` на `state.rs` уже был; slice не ухудшил façade shape.
- `row.clone()` в drain — небольшая аллокация; приемлемо для ясности match/requeue.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

---

## 4. Safety

- **Economic safety:** HIGH-001 устраняет класс «подписанный delayed transfer + тихая потеря»; reject явный и стабильный.
- **Emergency evac:** не регрессирует; cancel pending перед sweep сохранён.
- **DoS / queue growth:** requeue при transient failure ограничен размером `pending_conservation` (уже bounded политикой «один pending row на sender»). Permanent failure → один row requeued indefinitely — liveness/ops concern, не consensus split.
- **Panics:** новых `unwrap` в production path нет; drain по-прежнему использует `expect` на recipient после `require_recipient` (pre-existing).
- **Trust boundaries:** без изменений RPC/file paths.

---

## 5. Tests

**Added (3):**

- `conservation_export_race_reject` — Export blocked, balance/nonce/pending intact.
- `conservation_stake_race_reject` — Stake blocked, same assertions.
- `conservation_drain_insufficient_requeue` — fail drain → requeue → success on funded retry.

**Existing still relevant:**

- `conservation_pending_exists_reject` — second conservation transfer.
- `conservation_emergency_cancels_pending` — emergency + evac + cancelled drain.
- `conservation_delay_execute`, `conservation_incoming_not_delayed`, `chain::conservation_seal_drains`.

**Executed (review):** `CARGO_TARGET_DIR=F:/pwm-test/PWM-cryptocurrency cargo test -p pwm-core --lib conservation_` → **8 passed**.

**Optional follow-up (non-blocking):** тест на `Unstake` или `SetPolicy` reject при pending; интеграция drain-retry log с pwmd tracing вместо `eprintln!`.

---

## 6. Verdict

**PASS_WITH_NITS**

Приоритетные nits (не блокируют merge/testing):

1. **Observability:** `eprintln!` достаточен для «не silent», но слабее structured operator log в pwmd — зафиксировать в runbook или отдельный nit-тикет при soak.
2. **Test coverage:** Unstake/Import/non-emergency Policy reject опираются на общую ветку `_ => true` без именованных регрессий.
3. **Permanent drain failure:** requeue без cap/escalation — документировать как known ops edge (детерминированный stall одной row).

HIGH-001 и HIGH-002 по смыслу audit и acceptance criteria закрыты. Emergency path соответствует ADR 0011 (тикет ссылается на ADR 0009 — нормативный текст evac+CONSERVATION в ADR 0011 §Interaction). Scope creep отсутствует.

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260617-v6-audit-high-conservation-review.md
token_usage:
  source: estimate
  input: 18000
  output: 4500
  total: 22500
  confidence: medium
```

Note: slice verdict **PASS_WITH_NITS**; participation `result: PASS` reflects review gate cleared for pwm-testing (nits are non-blocking per orchestrator §Review nits).

---

## Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260617-v6-audit-high-conservation-review.md'
git add 'tasks/20260617-v6-audit-high-conservation.json'
git commit -m 'docs(v6): HIGH-001/002 conservation fix review'
```
