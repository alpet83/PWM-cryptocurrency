# Review: V6-5 Mode B cross-shard escrow

**Commit:** `937bb83` · **Branch:** `v6/20260607-v6-sprint5-mode-b-escrow-coding`  
**Ticket:** `tasks/20260607-v6-sprint5-mode-b-escrow-coding.json`  
**Normative:** `docs/rfc/addenda/v6-rfc9-mode-b-escrow.md`

---

## 1. Scope recap

Слайс V6-5 реализует Mode B escrow в **pwm-core**: при `EXPORT` — атомарный `CrossShardLock` и дебет spendable; при seal-tick — timeout refund; при `IMPORT` — release lock / credit; late import после refund — `E_EXPORT_LOCK_REFUNDED`.

| File | Role |
|------|------|
| `crates/pwm-core/src/state.rs` | Lock create, `refund_exp_locks`, IMPORT guards, lib tests |
| `crates/pwm-core/src/chain.rs` | Seal-tick refund, test `escrow_seal_refunds` |
| `crates/pwm-core/src/tx.rs` | `TxError::ExportLockRefunded` |
| `crates/pwm-core/src/reject_wire.rs` | Wire mapping `E_EXPORT_LOCK_REFUNDED` |
| `scripts/mode_b_escrow_smoke.cmd` | Smoke: `escrow_*` via `build_project.cmd` |

---

## 2. Requirements fit

| Criterion | Status | Notes |
|-----------|--------|-------|
| EXPORT: atomic lock + spendable debit | **Met** | Debit `amount+fee`, push `Locked`, `exported_registry` |
| Sender locked until finalize/refund | **Met** | Spendable reduced; repeat spend blocked |
| Seal-tick refund at `unlock_height` (RFC §5) | **Met** | `refund_exp_locks` до/после txs в `Chain::seal` |
| Timeout → Refunded + replay guard | **Met** | Credit refund account; `imported_set.insert` |
| Happy IMPORT → Released + credit | **Met** | `import_credits_target_happy_path` |
| Late IMPORT after refund | **Met (core)** | `ExportLockRefunded`; test `escrow_late_import_refunded` |
| Wire reject mapping | **Partial** | Mapping есть; pwmd preflight — см. nits |
| Tests / smoke | **Unverified in review** | dlltool env; **pwm-testing обязан** |

**Оговорка:** source lock release только при IMPORT на том же `State` — federation/CY E2E → V6-10.

---

## 3. Style and module shape

- `check_entity_name_segments.py` на diff-путях: **violations: []**
- Wire JSON / u128: not applicable (no peer wire change in slice)

---

## 4. Safety

- EXPORT atomicity; refund `saturating_add`; replay via `imported_set`
- Seal ordering: двойной `refund_exp_locks` корректен при `timeout=0`
- No new hot-path unwrap in production

---

## 5. Tests

| Test | Coverage |
|------|----------|
| `export_debit_fee_ok` | Lock after EXPORT |
| `import_credits_target_happy_path` | Released + credit |
| `escrow_refund_timeout` | Refund at unlock_height |
| `escrow_late_import_refunded` | `ExportLockRefunded` |
| `escrow_seal_refunds` | Seal path |
| `mode_b_escrow_smoke.cmd` | Filter `escrow_` |

---

## 6. Verdict

**PASS_WITH_NITS** — pwm-core соответствует RFC9; конвейер → **pwm-testing**.

| P | Nit | Action |
|---|-----|--------|
| P1 | pwmd preflight после refund → duplicate import, не `E_EXPORT_LOCK_REFUNDED` | Follow-up / V6-10 |
| P2 | Source lock release только same-State IMPORT | V6-10 federation |
| P3 | Нет unit-теста `reject_wire` для `ExportLockRefunded` | Mechanical (~3 lines) |
| P4 | `export_reject_low_no_mut`: assert empty locks | Optional |

---

## 7. Participation

```yaml
agent: pwm-review
result: PASS_WITH_NITS
artifacts: docs/reviews/20260607-v6-sprint5-mode-b-escrow-review.md
```
