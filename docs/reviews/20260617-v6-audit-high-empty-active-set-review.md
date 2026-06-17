# Review: V6 audit HIGH-003 empty active validator set mid-chain

**Date:** 2026-06-17  
**Ticket:** `tasks/20260617-v6-audit-high-empty-active-set.json`  
**Audit ref:** `docs/reviews/20260616-v6-mvp-rust-code-audit-review.md` (HIGH-003)  
**Commit:** `2eba174` — `fix(v6): fail fast on empty midchain active set`  
**Slice:** `crates/pwmd/src/lifecycle.rs` (+ ticket)  
**Reviewer:** pwm-review

---

## 1. Scope recap

Закрытие **HIGH-003** из V6 pre-publication audit: при `recompute_active_idxs` → `[]` после genesis proposer seal loop крутился без actionable `fatal_protocol_blocker`, тогда как cold-start (`tip_h == 0`, `lead_h == 1`) уже выходил через `3019528` / `mk_pick_fatal_diag`.

Тикет выбрал путь **(A)** — расширить fatal-диагностику в pwmd lifecycle для proposer, без смены consensus rule в pwm-core. Ожидаемый тест: `epoch_empty_active_midchain_diag` (в коммите — `epoch_empty_active_midchain_diag`). Acceptance: unit/lib only, без live CY cluster.

---

## 2. Requirements fit

### Core fix

**Implemented.** Удалён gate в `mk_pick_fatal_diag`:

```text
if g.chain.tip_h() != 0 || lead_h != 1 { return None; }
```

Теперь fatal path срабатывает при **любом** `lead_h`, если:

1. ошибка содержит `PROD_PICK_EMPTY_ERR` (`"no active validators for current epoch"`);
2. `active_validator_indices` пуст;
3. роль proposer в seal loop доходит до `local_prod_for_h` → `Err` → `exit_fatal_pick` (строки ~1519–1521).

Сообщения `exit_fatal_pick` обобщены с genesis-only на mid-chain: hint про `min_validator_stake`, второй log line — «validator stake/config fix» вместо «genesis/config fix».

**Сценарий audit закрыт для live proposer:** mid-chain epoch rollover с пустым active set → process exit(1) с `fatal_protocol_blocker` и полями `tip_h`, `lead_h`, stakes — вместо бесконечного spin в `cluster_primary_wait` / `proposer_pick_failed` warn loop.

### Partial coverage (documented, non-blocking)

| Path | Behavior after fix |
|------|-------------------|
| Live proposer seal loop | Fatal exit via `mk_pick_fatal_diag` |
| Cold start (`prod_pick_fatal_start`) | По-прежнему fatal (регрессия не сломана) |
| Snapshot replay (`io.rs`, `repair.rs`) | `pick_prod_idx` → `Err` в тексте mismatch; **без** `exit_fatal_pick` |
| Attester role | Seal loop не вызывает `local_prod_for_h` для commit (RFC16 non-committer branch) |

Replay/repair offline — ошибка операции уместнее `exit(1)`; audit stall concern относился к live proposer. Альтернатива audit «consensus rule prevents empty set» в pwm-core **не** реализована — осознанный scope тикета.

### Acceptance criteria (ticket)

| Criterion | Status |
|-----------|--------|
| Proposer при empty `active_validator_indices` → actionable fatal | Met |
| `cargo test` pwm-core + pwmd targeted | Claimed in ticket notes (coding); review не перезапускал |
| pwm-review PASS | This report |
| pwm-testing PASS; no cluster | Pending orchestrator |
| No scope creep on wire/consensus | Met (pwmd-only diagnostic) |

---

## 3. Style and module shape

- Production symbols в diff: `mk_pick_fatal_diag`, `exit_fatal_pick` — без новых имён; тест `epoch_empty_active_midchain_diag` (5 сегментов) — в пределах test budget.
- `python scripts/check_entity_name_segments.py crates/pwmd/src/lifecycle.rs` → **violations: []**.
- Module banner `//!` на `lifecycle.rs` без изменений.

**Scope creep (nit):** в том же коммите — рефакторинг structured `tracing` полей в `run_with` для `DeploymentProfile::SingleSealer` / `MultiSealerExperimental` (~20 строк). Поведение логирования эквивалентно; не связано с HIGH-003. Рекомендация coding: в будущем выносить observability churn в отдельный коммит.

### Wire JSON / u128

Wire JSON / u128: not applicable (no peer wire / RFC wire contract in this slice).

---

## 4. Safety

- **Liveness / ops:** fail-fast `std::process::exit(1)` — намеренный контракт для misconfigured stake; лучше, чем silent stall. Детерминированно на всех proposer-нодах с тем же state.
- **False positive:** fatal только при пустом `active_validator_indices` **и** matching error string; другие `pick_prod_idx` failures по-прежнему warn + sleep loop.
- **Attester / follower:** не затронуты fatal path.
- **Panics / crypto / DoS:** новых рисков нет; diff не трогает wire, mempool, file trust boundaries.

**Observability nit:** поле лога всё ещё называется `max_genesis_validator_stake`, хотя mid-chain это max staked among validator accounts — misleading label, pre-existing name, не блокер.

---

## 5. Tests

**Changed:**

- `prod_pick_nonfatal_after_start` **заменён** на `epoch_empty_active_midchain_diag` — корректно: старый тест фиксировал баг (ожидал `diag.is_none()` при `tip_h=3`).
- Новый тест: `set_canon_h(3)`, `active_validator_indices.clear()`, `mk_pick_fatal_diag(&app, 4, PROD_PICK_EMPTY_ERR)` → `Some` с `tip_h=3`, `lead_h=4`, `max_val_stake=0`, `min_stake=2_000_000`.

**Retained:**

- `prod_pick_fatal_start` — cold-start regression.

**Gaps (non-blocking):**

- Нет теста end-to-end `exit_fatal_pick` (process exit — ожидаемо unit-test только helper).
- Нет теста, что non-empty-set `pick_prod_idx` error **не** fatal mid-chain (логика следует из guards; optional negative test).

**Review execution:** entity-name check only; полный `cargo test` не перезапускался (coding notes в тикете).

---

## 6. Verdict

**PASS_WITH_NITS**

Приоритетные nits (не блокируют pwm-testing):

1. **Bundled logging refactor** в `run_with` — вынести в отдельный коммит при будущих слайсах.
2. **Log field name** `max_genesis_validator_stake` — переименовать в neutral `max_validator_stake` при следующем touch lifecycle logging.
3. **Replay paths** остаются non-fatal — приемлемо для offline; при желании owner может добавить явное упоминание в runbook.

HIGH-003 по смыслу audit и acceptance criteria для live proposer закрыт.

---

## 7. Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts: docs/reviews/20260617-v6-audit-high-empty-active-set-review.md
token_usage:
  source: estimate
  input: 14000
  output: 3800
  total: 17800
  confidence: medium
```

Note: slice verdict **PASS_WITH_NITS**; participation `result: PASS` — review gate cleared for pwm-testing (nits non-blocking per orchestrator §Review nits).

---

## Git handoff for orchestrator

```powershell
# git-handoff
Set-Location 'REPO_ROOT'
git add 'docs/reviews/20260617-v6-audit-high-empty-active-set-review.md'
git add 'tasks/20260617-v6-audit-high-empty-active-set.json'
git commit -m 'docs(v6): HIGH-003 empty active set fix review'
```
