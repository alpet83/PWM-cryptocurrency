# Sprint V2-1 — Slice B: State freeze for maturity, claim, free-day

**Дата:** 2026-05-05  
**Статус:** RFC freeze (docs-only, без правок `crates/*`)  
**База:** [sprint-v2-1-slice-a-tx-schema-freeze.md](./sprint-v2-1-slice-a-tx-schema-freeze.md), [sprint-v2-1-slice-1-test-matrix.md](./sprint-v2-1-slice-1-test-matrix.md), [sprint-v2-1-slice-1-normative-freeze.md](./sprint-v2-1-slice-1-normative-freeze.md)

---

## 1) Scope и цель freeze

Этот слайс фиксирует state-семантику для:

1. релевантного баланса в maturity-учёте;
2. полей `anchor_ref` и `claim_units` в ClaimTx;
3. событий сброса непрерывности maturity при изменении баланса (включая частичные изменения);
4. canonical UTC-day free-claim marker и chain-time правила;
5. baseline поведения при reorg/rollback;
6. инвариантов state machine для детерминированного replay.

---

## 2) Нормативные решения (state freeze)

### B-STATE-1. Релевантный баланс для maturity

- Для maturity используется только `staked_pwm_units` аккаунта (целое значение в минимальных единицах PWM).
- `liquid` баланс, `marks`, комиссии и прочие поля не входят в maturity base.
- В каждый момент высоты `h` релевантный баланс обозначается `B(h) = staked_pwm_units(h)`.

**Обоснование:** уменьшается неоднозначность и attack-surface; maturity привязан только к stake-состоянию, а не к движению ликвидных средств.

### B-STATE-2. Семантика `anchor_ref`

- `anchor_ref` в ClaimTx — это опорная высота блока (`u64`), относительно которой claim-проверка должна иметь детерминированный replay.
- Валидатор обязан проверить, что `anchor_ref <= inclusion_height`.
- Для успешного claim требуется `anchor_ref >= claim_state.last_claim_anchor_ref` (монотонность опоры).
- При несовместимости anchor с каноническим state на высоте включения — отклонение claim.

### B-STATE-3. Семантика `claim_units`

- `claim_units` — запрошенная к материализации целая дельта `marks` (`u64`), без дробей.
- Допустимость: `0 < claim_units <= matured_units_available(account, inclusion_height, anchor_ref)`.
- `matured_units_available` рассчитывается из укоренённых completed-интервалов неподвижности (`B` неизменен) после `last_claim_anchor_ref`.
- При успешном применении:
  - `marks += claim_units`;
  - `last_claim_anchor_ref = inclusion_height`;
  - `matured_credit` уменьшается на `claim_units` (или обнуляется при полном claim, в зависимости от внутреннего представления).

### B-STATE-4. Сброс непрерывности при изменении релевантного баланса

- Непрерывность maturity определяется по неизменности `B(h)`.
- Любое изменение `B(h)` между соседними canonical высотами (`B(h) != B(h-1)`) вызывает сброс текущего непрерывного интервала.
- Это правило одинаково для:
  - увеличения stake,
  - уменьшения stake,
  - частичного изменения stake (на любую ненулевую дельту),
  - событий slashing/forced adjustment, если они меняют `staked_pwm_units`.
- После сброса новый интервал начинается с высоты изменения (новый baseline).

### B-STATE-5. Canonical free-claim marker (UTC-day)

- Для бесплатной claim хранится один канонический маркер: `last_free_claim_utc_day: u32|u64`.
- Day-index вычисляется только из **chain time** блока включения:
  - `utc_day = floor(block_unix_time_utc / 86400)`.
- Локальные часы клиента, timezone клиента и wall-clock ноды не участвуют в решении.
- Правило допуска free-claim:
  - если `last_free_claim_utc_day != utc_day` -> free-claim может пройти (при прочих валидных условиях);
  - если `last_free_claim_utc_day == utc_day` -> повторная free-claim отклоняется (`FREE_CLAIM_DAILY_LIMIT`), остаётся paid fallback.

### B-STATE-6. Baseline для reorg/rollback

- `claim_state` и free-day marker являются частью replayable chain state и должны полностью откатываться при rollback.
- После reorg состояние claim/free определяется только canonical веткой:
  - изменения из orphaned блоков не сохраняются;
  - free-slot и claim-credit восстанавливаются согласно replay canonical chain.
- Детерминизм: одинаковый canonical префикс блоков всегда даёт одинаковое `claim_state`.

---

## 3) Минимальная модель состояния аккаунта (для Slice C/D handoff)

Рекомендуемый baseline полей state:

- `staked_pwm_units: u64`
- `maturity_continuity_start_height: u64` (начало текущего непрерывного интервала `B`-стабильности)
- `last_balance_change_height: u64`
- `last_claim_anchor_ref: u64`
- `last_free_claim_utc_day: u64`
- `matured_credit_units: u64` (если выбран явный аккумулятор)

Допускается эквивалентное внутреннее представление, но внешняя семантика B-STATE-1..6 обязана сохраниться.

---

## 4) Инварианты state machine

1. **Monotonic anchor:** `last_claim_anchor_ref` не убывает по canonical chain.
2. **No over-claim:** за один claim нельзя материализовать больше доступной matured-дельты.
3. **Balance-change reset:** любое `B`-изменение (включая частичное) прерывает текущую непрерывность.
4. **Single free/day:** не более одной успешной free-claim на аккаунт в одном `utc_day`.
5. **Chain-time authority:** free-day определяется только block time canonical цепи.
6. **Replay determinism:** state после replay canonical blocks не зависит от локального времени/окружения ноды.
7. **Rollback correctness:** после reorg не остаётся побочных claim/free эффектов orphaned-ветки.

---

## 5) Связь с тест-матрицей и следующими слайсами

- Закрывает placeholder-решения для `P-RST-03`, `P-FRE-06`, `P-REO-*` на уровне baseline semantics.
- Slice C должен зафиксировать policy-validation matrix (ошибки и edge ordering для mempool/apply/preflight).
- Slice D должен закрепить API-формат ошибок и trace-поля для claim rejection path.

---

## 6) Decision log (Slice B)

1. Релевантный баланс для maturity: только `staked_pwm_units`.
2. `anchor_ref` зафиксирован как опорная высота с монотонной проверкой.
3. `claim_units` трактуется как целая материализуемая дельта, ограниченная доступным matured-credit.
4. Любое ненулевое изменение релевантного баланса (включая частичное) сбрасывает непрерывность.
5. Free-claim marker зафиксирован как `utc_day` из chain time (`floor(ts/86400)`).
6. Reorg/rollback baseline: full replay canonical branch без сохранения orphaned claim/free эффектов.
7. Принят набор инвариантов state machine для дальнейшей кодовой реализации и тестирования.
