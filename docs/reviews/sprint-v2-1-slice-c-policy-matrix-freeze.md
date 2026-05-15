# Sprint V2-1 — Slice C: Policy matrix freeze (mempool / preflight / apply)

**Дата:** 2026-05-05  
**Статус:** RFC freeze (docs-only, без правок `crates/*`)  
**База:** [sprint-v2-1-slice-a-tx-schema-freeze.md](./sprint-v2-1-slice-a-tx-schema-freeze.md), [sprint-v2-1-slice-b-state-freeze.md](./sprint-v2-1-slice-b-state-freeze.md), [sprint-v2-1-slice-b-test-report.md](./sprint-v2-1-slice-b-test-report.md), [sprint-v2-1-slice-b-review.md](./sprint-v2-1-slice-b-review.md), [sprint-v2-1-slice-1-test-matrix.md](./sprint-v2-1-slice-1-test-matrix.md)

---

## 1) Scope freeze

Slice C фиксирует policy-уровень для ClaimTx/BurnMarkTx в трёх фазах исполнения:

1. `mempool` (admission и ранний отбор),
2. `preflight` (симуляция/проверка для API/клиента),
3. `apply` (каноническое применение в блоке).

Нормы Slice C:

- задают единый порядок проверок по фазам;
- закрывают `N-MAT-5` единственным правилом округления;
- формализуют предикаты incompatibility для `anchor_ref` (finding M1 из Slice B review);
- уточняют `P-REO-04` для rollback/reorg;
- фиксируют соответствие "решение -> класс ожидаемой ошибки" без финального API-wire формата.

---

## 2) Фазовая policy matrix (ClaimTx)

### C-POL-1. Единый порядок проверок

Для `mempool`, `preflight`, `apply` используется один и тот же логический порядок:

1. **Schema/field gates** (тип, обязательные поля, базовые диапазоны).
2. **Mode/fee policy** (`free|paid`, совместимость с fee и policy-порогами).
3. **Anchor/state compatibility** (предикаты C-ANC-*).
4. **Maturity arithmetic** (доступная дельта и правило округления C-MAT-1).
5. **Free-day/reorg-sensitive checks** (`utc_day`, rollback-correctness контекст).
6. **State transition guards** (no over-claim, monotonic anchor на применении).

`apply` является канонической фазой: при конфликте из-за гонок состояния её вердикт приоритетен, а `mempool/preflight` обязаны сходиться с `apply` при одинаковом входном snapshot.

### C-POL-2. Матрица "фаза -> обязательные проверки"

| Политический блок | mempool | preflight | apply |
|---|---|---|---|
| Schema + mandatory fields | MUST | MUST | MUST |
| `mode`/`fee` consistency | MUST | MUST | MUST |
| Fee threshold for paid | SHOULD (по локальной политике mempool) | MUST | MUST |
| Anchor incompatibility predicates (C-ANC-*) | MUST (snapshot-based) | MUST (snapshot-based) | MUST (canonical state) |
| Maturity bounds + rounding (C-MAT-1) | MUST (estimate on tip snapshot) | MUST | MUST |
| Free-day (`last_free_claim_utc_day`, chain-time day) | SHOULD for optimistic admission, MUST on final admit | MUST (with declared block-time context) | MUST |
| Reorg-sensitive rollback correctness (C-REO-1) | N/A as direct mutation, but MUST not persist irreversible side effects | N/A as direct mutation, but MUST mirror canonical replay logic | MUST |

Пояснение: для `mempool` допускается локальный отказ от строгой fee-политики как операционный выбор, но все **consensus-critical** проверки обязательны и не могут быть ослаблены.

---

## 3) Fix по N-MAT-5 (округление)

### C-MAT-1. Единственное правило округления

Фиксируется правило: **усечение дробной части в пользу сети (floor to integer units)**.

- `matured_units_raw` вычисляется в детерминированной арифметике.
- Материализуемая величина: `matured_units = floor(matured_units_raw)`.
- Любой `sub-quantum remainder` не переносится как отдельный переносимый state-credit.
- `claim_units` валидно только при `claim_units <= matured_units_available_int`, где доступность рассчитана с учётом floor.

Это закрывает `P-MAT-06 / N-MAT-5` как единственный нормативный вариант для всех фаз.

---

## 4) Формализация anchor incompatibility predicates (M1)

### C-ANC-1. Минимальный набор предикатов несовместимости

ClaimTx считается anchor-incompatible (и отклоняется), если выполняется любой из предикатов:

1. **C-ANC-A (future anchor):** `anchor_ref > inclusion_height`.
2. **C-ANC-B (non-monotonic anchor):** `anchor_ref < last_claim_anchor_ref(account)`.
3. **C-ANC-C (continuity violation):** в интервале `(anchor_ref, inclusion_height]` обнаружен хотя бы один переход `B(h) != B(h-1)` для данного аккаунта, если claim заявлен как использующий непрерывность этого интервала.
4. **C-ANC-D (missing canonical anchor view):** нода не может построить канонический state snapshot, достаточный для проверки claim относительно `anchor_ref` на высоте включения (включая reorg-переходный момент до восстановления канона).

### C-ANC-2. Семантика применения по фазам

- `mempool/preflight` проверяют C-ANC-* по доступному snapshot и обязаны вернуть тот же класс отказа, который возник бы на `apply` при неизменном canonical pre-state.
- `apply` проверяет C-ANC-* строго по state canonical ветки блока включения.

---

## 5) Уточнение P-REO-04 (reorg/rollback semantics)

### C-REO-1. Норматив для `last_free_claim_utc_day` и последнего успешного claim

При rollback/reorg состояние `last_free_claim_utc_day` и `last_claim_anchor_ref` определяется **только** replay canonical ветки после точки расхождения:

1. Эффекты orphaned-ветки полностью аннулируются.
2. Если free-claim была успешна только в orphaned-ветке, free-slot считается неиспользованным после отката.
3. Если claim материализовала `marks` только в orphaned-ветке, эта материализация не существует после отката.
4. Повторное включение логически того же claim в новом каноне валидируется как обычная транзакция на восстановленном canonical state, без "призрачных" ограничений от orphaned history.

Это закрывает placeholder `P-REO-04` на уровне policy semantics.

---

## 6) Decision -> expected error code class

Ниже фиксируются **классы** (семантические категории), без обязательства финального API-представления.

| Decision / predicate | Expected error code class |
|---|---|
| Нарушение обязательных полей/типов ClaimTx | `E_SCHEMA_INVALID` |
| Некорректная связка `mode`/`fee` | `E_MODE_FEE_CONFLICT` |
| Недостаточная комиссия в `paid` режиме | `E_FEE_POLICY_REJECT` |
| C-ANC-A или C-ANC-B | `E_ANCHOR_RANGE_INVALID` |
| C-ANC-C | `E_ANCHOR_CONTINUITY_BROKEN` |
| C-ANC-D | `E_ANCHOR_STATE_UNAVAILABLE` |
| `claim_units <= 0` или нецелевое значение | `E_CLAIM_UNITS_INVALID` |
| `claim_units > matured_units_available_int` после C-MAT-1 | `E_CLAIM_OVER_MATURED` |
| Повторная free-claim в том же `utc_day` | `E_FREE_CLAIM_DAILY_LIMIT` |
| Нарушение rollback-replay корректности (detected on apply/replay checks) | `E_REORG_STATE_MISMATCH` |

---

## 7) Decision log (Slice C)

1. Зафиксирован единый фазовый порядок policy-checks для `mempool/preflight/apply`.
2. `N-MAT-5` закрыт правилом `floor` (усечение в пользу сети), без переноса sub-quantum remainder.
3. Finding M1 из Slice B review закрыт через формальный набор предикатов C-ANC-A..D.
4. `P-REO-04` уточнён как canonical-replay-only правило для free-day и last successful claim.
5. Зафиксирована таблица соответствия решений и классов ошибок (без финальной API-формы).
6. Нормативно сохранена симметрия verdict между фазами при одинаковом snapshot.

---

## 8) Handoff notes (к Slice D и implementation)

- Slice D должен закрепить wire/API формат для классов `E_*` (имя, поле, trace).
- При реализации в `crates/*` коды должны оставаться аддитивно-расширяемыми и стабильными по смыслу.
- Тестовый слой может использовать эту матрицу как источник ожидаемого класса для негативных кейсов `P-MAT-*`, `P-FRE-*`, `P-REO-*`.
