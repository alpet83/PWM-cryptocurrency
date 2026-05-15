# Sprint V2-1 RFC Inputs (2026-05-05)

## Context

- Sprint V2-1 требует RFC-уровневой фиксации новых вводных владельца до следующего цикла product-реализации.
- Текущий шаг: зафиксировать normative inputs и вопросы дизайна без правок `crates/*`.
- Документ является входом для Slice 1 (RFC normative freeze) и должен быть использован как primary source при обновлении специки.

## Owner inputs

1. **BurnMarkTx: добавить `purpose`**
   - В BurnMarkTx добавить текстовое поле `purpose` длиной до примерно 80 символов.
   - Назначение поля: человеко-читаемая цель burn-операции.
   - Пример из вводных владельца: salted hash e-mail + salt.

2. **Созревание марок для неподвижного баланса**
   - Формула созревания: `1 PWM = 1 час`, альтернативно в блоковой форме: `1 PWM = 3600 блоков`.
   - Накопление относится к периоду неподвижного (неизменного) баланса между релевантными событиями учёта.

3. **Начисление марок только через claim**
   - Начисление не происходит автоматически фоном.
   - Пользователь инициирует claim-транзакцию.
   - Валидируется дельта по времени накопления относительно предыдущего claim.
   - После успешной подписи/применения claim баланс обновляется на расчётную дельту.

4. **Одна бесплатная claim-транзакция в сутки**
   - Разрешается 1 бесплатная claim-транзакция в сутки.
   - Требуется отдельный anti-complexity brainstorming, чтобы не усложнить state/policy/API.

## RFC impact

- **tx**
  - Расширение payload BurnMarkTx новым полем `purpose` (ограничение длины, нормализация/валидация).
  - Явное выделение claim-транзакции как механизма материализации накопленных марок.
  - Нормативно описать free-vs-paid режим claim (лимит 1 бесплатная/сутки).

- **state**
  - Добавить/уточнить state-поля для расчёта claim-доступной дельты от предыдущего claim и интервала неподвижного баланса.
  - Зафиксировать единицу времени (часы vs блоки) и canonical conversion rule.
  - Добавить минимум состояния для free-claim окна (например, last free-claim marker), избегая избыточной истории.

- **policy**
  - Нормативно определить правило созревания (`1 PWM = 1h` / `3600 blocks`) и порядок округления.
  - Определить правила валидности claim при изменениях баланса между claim-событиями.
  - Зафиксировать policy-ограничения на `purpose` (длина, допустимые символы/encoding, отсутствие PII в открытом виде как рекомендация).

- **API**
  - Обновить схемы/контракты для BurnMarkTx (`purpose`) и для claim (признак бесплатности/комиссии, причины отказа).
  - Добавить явные коды ошибок: превышен бесплатный лимит за сутки, некорректная дельта времени, невалидный `purpose`.
  - Обновить примеры запросов/ответов для client integrators.

## Open design questions

1. Что считать «сутками» для 1 бесплатной claim: UTC-day boundary, rolling 24h window или epoch-based bucket?
2. Какой минимальный state нужен для free-claim лимита без хранения длинной истории (anti-complexity)?
3. Как избежать роста ветвлений в валидации claim: единый preflight validator vs разрозненные policy checks?
4. Какая canonical time-base в RFC: часы, блоки, или dual-form с жёстким первичным источником?
5. Как интерпретировать «неподвижный баланс» при частичных изменениях баланса и reorg/rollback сценариях?
6. Требуется ли hash/format guidance для `purpose`, чтобы избежать утечки персональных данных и сохранить UX простым?

### Anti-complexity brainstorming for "1 free claim/day"

- Предпочесть **один простой limiter key на аккаунт** (`last_free_claim_slot`/`last_free_claim_day`) вместо истории claim-событий.
- Вынести решение free-or-paid в **один policy модуль/функцию**, чтобы не дублировать логику в mempool/consensus/API.
- Использовать **детерминированный day bucket**, вычисляемый из chain-time, чтобы исключить неоднозначность на клиентах.
- В RFC сразу прописать «fallback paid claim always allowed», чтобы не блокировать materialization и не плодить edge-cases.

## Proposed next RFC slices

1. **Slice A: Normative tx schema freeze**
   - Freeze полей BurnMarkTx/ClaimTx, ограничения `purpose`, и error taxonomy.

2. **Slice B: State accounting model freeze**
   - Freeze формулы накопления, точки отсчёта предыдущего claim, и состояние для free-claim лимитера.

3. **Slice C: Policy validation matrix**
   - Freeze детальные positive/negative правила для claim и burn с примерами edge-cases.

4. **Slice D: API contract alignment**
   - Freeze RPC/REST схемы и совместимость клиентских интеграций по новым полям и ошибкам.

5. **Slice E: Implementation handoff note**
   - Подготовить минимальный implementation brief для `pwm-coding`/`pwm-testing` без расширения scope за пределы V2-1.
