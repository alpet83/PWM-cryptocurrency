# Sprint 8 Test Report

Дата: 2026-04-25  
Этап: Slice 1/6 (`marks_quota` state wiring)

## Verdict

**PASS**

## Commands and results

- `cargo fmt --check` -> PASS
- `cargo check -p pwmd` -> PASS
- `cargo check -p pwm-core` -> PASS
- `cargo test -p pwmd` -> PASS (`56 passed; 0 failed`)

## Slice 1 invariants parity

- Burn-path uses quota model: `BURN_MARK` debits `marks_quota`, not `balance_pwm`.
- Insufficient quota reject has no side effects on account/quota state.
- Init/default quota semantics remain deterministic with normalization.
- Snapshot compatibility guard added for orphan `marks_quota` IDs.

## Residual risks

- Runtime/e2e burn-path via HTTP surface is not yet the focus of Slice 1 and will be covered in Slice 2+.

---

## Slice 2 Test Gate

Дата: 2026-04-25  
Вердикт: **PASS**

### Commands and results

- `cargo fmt --check` -> PASS
- `cargo check -p pwmd` -> PASS
- `cargo check -p pwm-core` -> PASS
- `cargo test -p pwmd` -> PASS (`58 passed; 0 failed`)
- `cargo test -p pwm-core burn_mark` -> PASS (`2 passed; 0 failed`)

### Slice 2 invariants parity

- Explicit reject on insufficient quota is preserved (`TxError::InsufficientMarks`).
- Reject path confirms no side effects on `account`, `fee_pool`, `marks_quota`.
- `BURN_MARK` tx guard behavior is explicitly covered for policy-invalid beneficiary reject and same-shard allow path.

### Residual risks (Slice 2)

- Full `cargo test -p pwm-core` suite не запускался, только targeted burn subset.
- API-surface e2e для burn flow остаётся задачей следующих slices.

---

## Slice 3 Test Gate

Дата: 2026-04-25  
Вердикт: **PASS**

### Commands and results

- `cargo fmt --check` -> PASS
- `cargo check -p pwmd` -> PASS
- `cargo check -p pwm-core` -> PASS
- `cargo test -p pwmd` -> PASS (`58 passed; 0 failed`)
- `cargo test -p pwm-core burn_mark` -> PASS (`4 passed; 0 failed`)

### Slice 3 invariants parity

- Mark-based flow сохраняет zero-fee baseline (`TxBody::fee_amount()` returns `0` for `BurnMark`).
- Fee-path side effects для burn-сценариев явно покрыты и не затрагивают `fee_pool`/балансы вне ожидаемой логики.
- Reject-путь с недостаточной quota остаётся deterministic и без побочных эффектов.

### Residual risks (Slice 3)

- Полный `cargo test -p pwm-core` не запускался (только targeted burn subset).
- Cross-domain burn context boundary ещё не реализован (планируется в Slice 4).

---

## Slice 4 Test Gate

Дата: 2026-04-25  
Вердикт: **PASS**

### Commands and results

- `cargo fmt --check` -> PASS
- `cargo check -p pwmd` -> PASS
- `cargo check -p pwm-core` -> PASS
- `cargo test -p pwmd` -> PASS (`59 passed; 0 failed`)
- `cargo test -p pwm-core burn_mark` -> PASS (`5 passed; 0 failed`)

### Slice 4 invariants parity

- Cross-domain burn context is blocked on source boundary (`pwmd` local guard).
- Source-only boundary is also enforced at core state transition (`pwm-core` hard invariant).
- Reject path side effects remain controlled (`account`/`fee_pool`/`marks_quota` invariants covered).
- Target-side burn behavior not introduced.

### Residual risks (Slice 4)

- Full `cargo test -p pwm-core` suite remains pending (targeted burn subset used for this slice).
- End-to-end multi-node scenarios stay out of scope for this slice.

---

## Slice 5 Test Gate (sprint consolidated regression)

Дата: 2026-04-25  
Вердикт: **PASS**

### Commands and results

- `cargo fmt --check` -> PASS
- `cargo check -p pwmd` -> PASS
- `cargo check -p pwm-core` -> PASS
- `cargo test -p pwmd` -> PASS (`59 passed; 0 failed`)
- `cargo test -p pwm-core` -> PASS (`56 passed; 0 failed`)

### Sprint 8 invariants parity (consolidated)

- Quota-path burn, zero-fee baseline, source-only burn context — согласованы с предыдущими slice-отчётами; полный `pwm-core` suite не выявил регрессий вне burn-тестов.

### Residual risks (Sprint 8 → Sprint 9)

- E2E multi-node и операторский UX остаются в зоне Sprint 9+ (CLI/TUI и интеграционные сценарии).
