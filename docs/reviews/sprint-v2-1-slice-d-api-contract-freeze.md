# Sprint V2-1 — Slice D: API contract freeze (error mapping and reject shapes)

**Дата:** 2026-05-05  
**Статус:** RFC freeze (docs-only, без правок `crates/*`)  
**База:** [sprint-v2-1-slice-c-policy-matrix-freeze.md](./sprint-v2-1-slice-c-policy-matrix-freeze.md), [sprint-v2-1-slice-c-test-report.md](./sprint-v2-1-slice-c-test-report.md), [sprint-v2-1-slice-c-review.md](./sprint-v2-1-slice-c-review.md)

---

## 1) Scope и стабильность контракта

Slice D фиксирует wire/API слой для отказов (`reject paths`) и симметрию вердиктов между `mempool`, `preflight` и `apply`.

Норматив D:

1. устанавливает стабильное соответствие `семантика решения -> error code -> response class`;
2. закрепляет минимальный JSON-контракт отказа для клиентов;
3. делает трассируемость к decision classes Slice C (`E_*`);
4. явно включает **burn-related** случаи, чтобы закрыть scope-gap из Slice C review/testing.

Гарантия совместимости: значения `error.code` из таблиц ниже считаются стабильными по смыслу; расширение возможно только аддитивно (новые коды без переопределения старых).

---

## 2) Response classes (единая API-классификация)

Для всех reject-ответов используются классы:

- `VALIDATION_ERROR` — нарушение схемы/формата/базовых диапазонов;
- `POLICY_REJECT` — политика режима, fee, day-limit и иные rule-level ограничения;
- `STATE_CONFLICT` — конфликт с текущим canonical state (anchor continuity/range, over-matured, reorg mismatch);
- `TEMPORARY_UNAVAILABLE` — временная недоступность канонического представления состояния (retry-able).

`response_class` обязателен и должен быть детерминированно выводим из `error.code`.

---

## 3) Stable mapping: semantics -> API response class

| Семантика отказа | Stable `error.code` | `response_class` | Traceability (Slice C) |
|---|---|---|---|
| Нарушение обязательных полей/типов ClaimTx | `E_SCHEMA_INVALID` | `VALIDATION_ERROR` | C-POL-1, §6 |
| Некорректная связка `mode`/`fee` | `E_MODE_FEE_CONFLICT` | `POLICY_REJECT` | C-POL-1, C-POL-2, §6 |
| Недостаточная комиссия в `paid` режиме | `E_FEE_POLICY_REJECT` | `POLICY_REJECT` | C-POL-2, §6 |
| Anchor range invalid (`future` / non-monotonic) | `E_ANCHOR_RANGE_INVALID` | `STATE_CONFLICT` | C-ANC-A/B, §6 |
| Anchor continuity broken | `E_ANCHOR_CONTINUITY_BROKEN` | `STATE_CONFLICT` | C-ANC-C, §6 |
| Canonical anchor view unavailable | `E_ANCHOR_STATE_UNAVAILABLE` | `TEMPORARY_UNAVAILABLE` | C-ANC-D, §6 |
| Невалидный `claim_units` | `E_CLAIM_UNITS_INVALID` | `VALIDATION_ERROR` | C-POL-1, §6 |
| `claim_units > matured_units_available_int` | `E_CLAIM_OVER_MATURED` | `STATE_CONFLICT` | C-MAT-1, §6 |
| Повторная free-claim в тот же `utc_day` | `E_FREE_CLAIM_DAILY_LIMIT` | `POLICY_REJECT` | C-POL-2, C-REO-1, §6 |
| Reorg/replay state mismatch | `E_REORG_STATE_MISMATCH` | `STATE_CONFLICT` | C-REO-1, §6 |
| Burn: невалидная структура/типы BurnMarkTx | `E_BURN_SCHEMA_INVALID` | `VALIDATION_ERROR` | Slice C scope carry-over (burn) |
| Burn: невалидный диапазон `burn_units` (`<=0` или overflow) | `E_BURN_UNITS_INVALID` | `VALIDATION_ERROR` | Slice C scope carry-over (burn) |
| Burn: попытка сжечь сверх доступного баланса/mark-supply | `E_BURN_OVER_BALANCE` | `STATE_CONFLICT` | Slice C scope carry-over (burn) |
| Burn: daily/rate policy ограничение (если policy включена) | `E_BURN_POLICY_REJECT` | `POLICY_REJECT` | Slice C scope carry-over (burn) |

Примечание: для burn-кейсов применяются те же фазовые принципы C-POL-* (равенство класса решения между фазами при одинаковом snapshot), даже если детальные burn-предикаты будут расширяться в следующем RFC.

---

## 4) Consistency contract: preflight vs mempool vs apply

### D-CON-1. Семантическая симметрия

При одинаковом входе и эквивалентном pre-state:

1. `preflight` **MUST** вернуть тот же `error.code`, что и `apply` при включении в тот же canonical контекст;
2. `mempool` **MUST** совпадать с `apply` для consensus-critical причин отказа;
3. если `mempool` использует локальные операционные послабления (например, fee admission), это не меняет обязательный mapping для consensus-critical веток.

### D-CON-2. Snapshot drift и повторяемость

Если между `preflight` и `apply` изменился canonical state (гонка высоты/reorg), допускается смена результата, но:

- новый отказ всё равно обязан использовать код из стабильной таблицы Slice D;
- при C-ANC-D класс должен быть `TEMPORARY_UNAVAILABLE`, а не произвольный generic error.

### D-CON-3. Burn parity

Burn-ветки следуют тем же правилам симметрии `preflight/mempool/apply`, что и claim:

- одинаковый snapshot -> одинаковый `error.code`;
- отличие возможно только при state drift;
- drift не даёт права менять семантический класс ошибки.

---

## 5) Minimal JSON reject shapes (normative)

Минимальный reject-ответ:

```json
{
  "ok": false,
  "phase": "preflight",
  "tx_kind": "claim",
  "response_class": "STATE_CONFLICT",
  "error": {
    "code": "E_CLAIM_OVER_MATURED",
    "message": "claim_units exceeds matured_units_available_int",
    "trace_id": "8f74f3ef-6d4e-4f4b-aece-3ac0e0f7d1d1"
  }
}
```

Обязательные поля:

- `ok` (bool, всегда `false` для reject);
- `phase` (`mempool|preflight|apply`);
- `tx_kind` (`claim|burn`), где проверки/ошибки по `purpose` относятся к `tx_kind = burn`;
- `response_class` (одно из 4 значений §2);
- `error.code` (стабильный `E_*` из §3);
- `error.message` (краткое человекочитаемое пояснение);
- `error.trace_id` (корреляционный идентификатор для диагностики).

### Пример A: preflight reject (claim / maturity)

```json
{
  "ok": false,
  "phase": "preflight",
  "tx_kind": "claim",
  "response_class": "STATE_CONFLICT",
  "error": {
    "code": "E_CLAIM_OVER_MATURED",
    "message": "claim_units exceeds matured_units_available_int",
    "trace_id": "trc-preflight-001"
  }
}
```

### Пример B: mempool reject (burn / schema)

```json
{
  "ok": false,
  "phase": "mempool",
  "tx_kind": "burn",
  "response_class": "VALIDATION_ERROR",
  "error": {
    "code": "E_BURN_SCHEMA_INVALID",
    "message": "missing burn_units",
    "trace_id": "trc-mempool-019"
  }
}
```

### Пример C: apply reject (anchor unavailable, retry-able)

```json
{
  "ok": false,
  "phase": "apply",
  "tx_kind": "claim",
  "response_class": "TEMPORARY_UNAVAILABLE",
  "error": {
    "code": "E_ANCHOR_STATE_UNAVAILABLE",
    "message": "canonical anchor view is temporarily unavailable",
    "trace_id": "trc-apply-044"
  }
}
```

---

## 6) Decision log (Slice D)

1. Зафиксирован стабильный API mapping `семантика -> E_* -> response_class`.
2. Закреплён инвариант фазовой симметрии для `preflight/mempool/apply`.
3. Определён минимальный JSON reject-контракт с обязательными полями.
4. Добавлены burn-related error-кейсы для явного закрытия scope-gap из Slice C.
5. Установлена трассируемость к Slice C decision classes (`C-POL-*`, `C-MAT-1`, `C-ANC-*`, `C-REO-1`).

---

## 7) Handoff notes

- Для реализации в `crates/*` коды из §3 должны вводиться как стабильные enum/const с аддитивным расширением.
- `pwm-testing` может строить негативные API-кейсы напрямую от `error.code + response_class + phase`.
- `pwm-review` при кодовой интеграции проверяет, что burn-кейсы не редуцированы к generic/internal error без стабильного `E_*`.
