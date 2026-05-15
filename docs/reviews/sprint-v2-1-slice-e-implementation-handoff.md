# Sprint V2-1 — Slice E: implementation handoff (docs-only)

**Дата:** 2026-05-05  
**Статус:** Implementation handoff freeze (docs-only, без правок `crates/*`)  
**База:** [sprint-v2-1-rfc-inputs-20260505.md](./sprint-v2-1-rfc-inputs-20260505.md), [sprint-v2-1-slice-a-tx-schema-freeze.md](./sprint-v2-1-slice-a-tx-schema-freeze.md), [sprint-v2-1-slice-b-state-freeze.md](./sprint-v2-1-slice-b-state-freeze.md), [sprint-v2-1-slice-c-policy-matrix-freeze.md](./sprint-v2-1-slice-c-policy-matrix-freeze.md), [sprint-v2-1-slice-d-api-contract-freeze.md](./sprint-v2-1-slice-d-api-contract-freeze.md)

---

## 1) Normative decisions lock (A-D)

Ниже фиксируется единый lock-пакет, который считается обязательным для первой кодовой волны V2-1.

1. **BurnMarkTx v2**: обязательное поле `purpose`; лимит `1..80` UTF-8 байт после `trim`, запрет C0/C1 control chars, без NFC/NFKC transforms.
2. **ClaimTx baseline**: обязательные `mode`, `claim_units`, `anchor_ref`, `fee`; связка `mode=free -> fee=0`, `mode=paid -> fee>0`.
3. **Maturity base**: релевантен только `staked_pwm_units`; любое ненулевое изменение stake сбрасывает непрерывность интервала созревания.
4. **Time base**: canonical day bucket только из chain time `utc_day=floor(block_unix_time_utc/86400)`; клиентский wall-clock не участвует.
5. **Maturity rounding**: единственный вариант `floor` (в пользу сети); перенос дробного остатка как отдельного state-credit не допускается.
6. **Anchor predicates**: обязательны C-ANC-A..D (future/non-monotonic/continuity broken/state unavailable) одинаково по смыслу для preflight/mempool/apply.
7. **Free claim/day**: не более одной успешной free-claim на аккаунт в одном `utc_day`; paid fallback всегда разрешён при валидном tx.
8. **Stable errors/API class**: mapping `E_* -> response_class` из Slice D обязателен; расширение только аддитивно, переименование кодов запрещено.
9. **Reorg rule**: claim/free-state полностью replayable; orphaned effects не сохраняются.
10. **Legacy compatibility**: BurnMarkTx v1 (без `purpose`) допускается через временный adapter path до отдельного deprecation-решения.

---

## 2) File-level implementation map (первая код-волна)

Цель карты: ограничить первую реализацию минимально необходимыми точками входа и не расширять scope.

### 2.1 `crates/pwm-core` (консенсусная семантика)

- `crates/pwm-core/src/tx/*`  
  - добавить/уточнить schema structs для BurnMarkTx v2 и ClaimTx;
  - ввести normalizer/validator для `purpose` (`trim`, UTF-8 len, control-char gate).
- `crates/pwm-core/src/state/*`  
  - поля claim/free-day baseline: `last_claim_anchor_ref`, `last_free_claim_utc_day`, `maturity_continuity_start_height`, `matured_credit_units` (или эквивалентный internal view).
- `crates/pwm-core/src/policy/*`  
  - единая policy-функция для mode/fee/free-day и anchor predicates;
  - `floor`-округление matured units как единственный path.
- `crates/pwm-core/src/replay|chain/*`  
  - rollback/reorg корректность claim/free-state по canonical replay.

### 2.2 `crates/pwmd` (API/preflight/reject contract)

- `crates/pwmd/src/*tx*` / `*api*` / `*rpc*`  
  - wire-модель reject-ответа (`ok=false`, `phase`, `tx_kind`, `response_class`, `error{code,message,trace_id}`);
  - стабильный mapping `E_* -> response_class` для claim+burn;
  - preflight symmetry с consensus checks при одинаковом snapshot.
- `crates/pwmd/src/errors*`  
  - централизованный error registry (enum/const) без generic fallback для зафиксированных кейсов Slice D.

### 2.3 Клиентские слои (`pwm-cli`, `pwm-tui`) — первая волна только адаптация формата

- `crates/pwm-cli/src/*`  
  - сериализация новых tx-полей (`purpose`, `mode`, `anchor_ref`, `claim_units`);
  - отображение стабильных `error.code` и `response_class`.
- `crates/pwm-tui/src/*`  
  - минимальное чтение и показ reject-полей без новой UX-логики.

---

## 3) Phased rollout plan (code slices)

### Slice E-1 (foundation, consensus-first)

**Цель:** включить core-поля/инварианты без широкой API-полировки.  
**Состав:**
- tx schema + purpose validation;
- state baseline для claim/free-day;
- policy checks C-ANC/C-MAT/C-FRE в consensus path.

**Гейт:** deterministic apply/replay проходит на быстрых smoke- и unit-наборах.

### Slice E-2 (API and preflight parity)

**Цель:** закрепить стабильный reject-wire контракт и симметрию verdict.  
**Состав:**
- `E_* -> response_class` mapping в pwmd;
- reject JSON shapes в preflight/mempool/apply;
- burn/claim parity по кодам ошибок.

**Гейт:** одинаковый snapshot даёт одинаковый класс ошибки между preflight и apply.

### Slice E-3 (client adaptation and compatibility hardening)

**Цель:** сделать новые поля/ошибки пригодными для интеграторов без расширения экономики V2-1.  
**Состав:**
- pwm-cli/pwm-tui support новых payload/reject-полей;
- legacy BurnMarkTx v1 adapter checks + deprecation guardrails;
- документация миграции и контроль backward-совместимости.

**Гейт:** клиенты формируют v2 payload и корректно читают reject-контракт Slice D.

---

## 4) Миграционные риски `marks_quota -> marks`

1. **Скрытые остаточные ссылки в state/API**: старые поля/ошибки могут частично остаться в сериализации и ломать консистентность.
2. **Несогласованность округления**: если часть path ещё считает quota-like дроби, появятся расхождения preflight vs apply.
3. **Double-accounting при replay**: неверный reset continuity способен начислять marks повторно после reorg.
4. **Legacy client drift**: старые клиенты могут отправлять payload без `purpose`/`mode`, провоцируя неявные reject-классы.
5. **Error-code erosion**: fallback в generic/internal error разрушит контракт Slice D для интеграторов.
6. **UTC-day boundary race**: ошибочное использование локального времени ноды создаст недетерминизм free-claim лимита.

Митигирующий принцип первой волны: один источник истины для policy/state, без дублирования логики между consensus и API.

---

## 5) Test plan handoff для `pwm-testing`

Минимальный пакет для стартовой тестовой волны:

1. **Tx/schema tests**  
   - `purpose`: trim, UTF-8 byte limit (1/80/81), control-char negative cases;
   - ClaimTx: `mode/fee` конфликтные и валидные комбинации.
2. **State/maturity tests**  
   - reset непрерывности на любом изменении `staked_pwm_units`;
   - `floor`-округление и no over-claim.
3. **Free-day tests**  
   - одна free-claim в `utc_day`, вторая reject с `E_FREE_CLAIM_DAILY_LIMIT`;
   - paid fallback в тот же день остаётся доступен.
4. **Anchor/reorg tests**  
   - C-ANC-A..D негативы;
   - rollback canonical replay очищает orphaned claim/free-effects.
5. **API parity tests**  
   - preflight/apply дают тот же `error.code` и `response_class` на одинаковом snapshot;
   - reject JSON содержит обязательные поля (`phase`, `tx_kind`, `response_class`, `trace_id`).
6. **Compatibility tests**  
   - legacy BurnMarkTx v1 adapter path не ломает валидность legacy-трафика;
   - новые клиенты по умолчанию формируют v2.

Ожидаемый артефакт от `pwm-testing`: отчёт с coverage по блокам выше + список gap-рисков перед закрытием V2-1.

---

## 6) Done criteria для закрытия V2-1

V2-1 считается закрытым только при одновременном выполнении:

1. Реализованы lock-решения A-D без расхождений в consensus/API.
2. Миграция `marks_quota -> marks` не оставляет активных legacy-контуров в публичном контракте.
3. Claim path детерминирован: maturity, free-day, anchor predicates и replay/reorg корректны.
4. API выдаёт стабильные `E_*` и `response_class` для claim+burn, без generic подмен.
5. `pwm-testing` подтверждает test-gate PASS по tx/state/policy/api/reorg.
6. `pwm-review` подтверждает независимый quality gate PASS по рискам регрессии и совместимости.

---

## 7) Handoff note для следующего coding leg

Начинать реализацию с Slice E-1 (consensus-first), затем E-2 и E-3 без расширения экономического scope за рамки V2-1. Любые отклонения от lock-решений A-D считаются blocker и требуют отдельного RFC-решения.
