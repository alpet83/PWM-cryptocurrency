# Sprint V2-1 / Slice 0 — Spec↔Impl Audit

Дата: 2026-05-05  
План/якорь: `docs/plans/mvp_v2.md` (единый `marks`, `marks_quota` как legacy-заглушка), `docs/MVP-checklist.md` §1.

## Findings

### High

1. **`BURN_MARK` списывает `marks_quota`, а не `marks` (прямое расхождение с целевой моделью v2).**  
   В `State::apply_tx` ветка `TxBody::BurnMark` берёт `quota = self.marks_quota_of(&id)` и уменьшает `self.marks_quota`, при этом `Account.marks` не меняется.
   - Impact: поведение burn расходится с планом "единый баланс marks".
   - Files: `crates/pwm-core/src/state.rs`, `crates/pwm-core/src/tx.rs`.

2. **`marks` и `marks_quota` могут расходиться, что делает spendability неочевидной для клиентов.**  
   `accrue_marks` увеличивает `Account.marks`, затем вызывает `normalize_marks_quota` (backfill/retain), но уже потраченная квота burn остаётся отдельным состоянием.
   - Impact: пользователь видит `marks`, но фактический лимит burn определяется другим счётчиком.
   - Files: `crates/pwm-core/src/state.rs`, `crates/pwmd/src/api/types.rs`, `crates/pwmd/src/api/common.rs`.

### Medium

3. **Текущий API аккаунта публикует только `marks`, скрывая `marks_quota`-ограничение.**  
   `AcctOut` содержит `marks`, а сериализация берёт `ac.marks.to_string()`. Поле квоты наружу не выводится.
   - Impact: API/UX не отражает реальную способность выполнить burn в текущей реализации.
   - Files: `crates/pwmd/src/api/types.rs`, `crates/pwmd/src/api/common.rs`.

4. **Эмиссия PWM сейчас фиксированная и без порога стейка.**  
   При `seal` всегда вызывается `reward_producer(..., cfg.block_reward)`, `block_reward` задаётся в `GenCfg`.
   - Impact: нет RFC-first механики "PWM only from large stake (~100k)".
   - Files: `crates/pwm-core/src/chain.rs`, `crates/pwm-core/src/genesis.rs`.

5. **Эмиссия marks сейчас линейная по стейку и без явного минимального порога.**  
   `accrue_marks(coeff)` начисляет `staked * coeff / 1_000_000` всем initialized account.
   - Impact: отсутствует явный контролируемый порог "marks from ~1 stake" как нормативное правило.
   - Files: `crates/pwm-core/src/state.rs`, `crates/pwm-core/src/chain.rs`, `crates/pwm-core/src/genesis.rs`.

### Low

6. **Тесты ядра по burn закрепляют legacy-поведение квоты как норму.**  
   Тесты проверяют списание `marks_quota` и неизменность PWM/fee_pool.
   - Impact: при миграции на единый `marks` потребуется точечный пересмотр test oracle.
   - Files: `crates/pwm-core/src/state.rs`.

## Impacted files (consolidated)

- `crates/pwm-core/src/state.rs`
- `crates/pwm-core/src/chain.rs`
- `crates/pwm-core/src/genesis.rs`
- `crates/pwm-core/src/tx.rs`
- `crates/pwmd/src/api/types.rs`
- `crates/pwmd/src/api/common.rs`
- (tests in same module) `crates/pwm-core/src/state.rs`

## Где именно burn списывает квоту и что затронет миграция

Текущая точка списания:
- `State::apply_tx` -> `TxBody::BurnMark`:
  - проверка доменного контекста `burn_context_is_source_domain(tx)`;
  - чтение `quota = marks_quota_of(account_id)`;
  - reject при `quota < mark_amount` (`InsufficientMarks`);
  - уменьшение `self.marks_quota.insert(id, quota - mark_amount)`.

Минимальный радиус миграции к "burn from marks":
- удалить/обойти зависимость от `marks_quota_of` внутри burn-ветки;
- списывать `Account.marks` (в `accounts`) атомарно с nonce-update;
- пересмотреть `normalize_marks_quota` и хранение `marks_quota` как legacy read-only либо удалить на отдельном слайсе;
- обновить тесты burn-кейсов в `state.rs` (assert по `marks`, не по `marks_quota`);
- проверить `TxError::InsufficientMarks` в API mapping (семантика остаётся, источник данных меняется).

## Где и как сейчас начисляется эмиссия PWM/marks

- **PWM:** в `Chain::seal` после применения tx вызывается `st.reward_producer(&prod_acct, cfg.block_reward)`.
- **marks:** в `Chain::seal` вызывается `st.accrue_marks(cfg.marks_coeff)`.
- **Формула marks:** `staked * coeff / 1_000_000`, только для `initialized` account.
- **Конфигурация:** `GenCfg` содержит `block_reward` и `marks_coeff`; `dev_net()` задаёт значения по умолчанию (`100`, `10_000`).

## RFC-first: минимальные изменения под будущие пороги эмиссии

Ниже не реализация, а минимальные требования к следующему код-слайсу:

1. Добавить в конфиг наград (`GenCfg` или вложенный policy-объект) явные пороги:
   - `pwm_min_stake_for_emission` (ориентир ~100k),
   - `marks_min_stake_for_emission` (ориентир ~1).
2. Вынести расчёт награды в детерминированную функцию политики (единая точка для `seal`):
   - входы: `height`, `timestamp`, stake/validator snapshot;
   - выходы: deltas PWM + marks.
3. В `reward_producer`/`seal` применять PWM-награждение только при выполнении порога (и будущих RFC-условий).
4. В `accrue_marks` применять минимальный порог стейка до начисления marks.
5. До фикса RFC пометить пороги как config-driven placeholders, чтобы не хардкодить финальные числа в runtime-логике.

## Proposed next slices V2-1 (narrow)

1. **Slice 1:** freeze normative text для "single marks" + явный статус `marks_quota` (legacy-only).
2. **Slice 2:** RFC-delta для эмиссии: пороги PWM/marks + deterministic inputs + seasonality placeholders.
3. **Slice 3:** mapping table spec→code (state/chain/genesis/api) с acceptance assertions.
4. **Slice 4:** compatibility note по snapshot/replay при будущем изменении reward policy.
5. **Slice 5:** test-plan draft для `pwm-testing` (burn semantics, emission thresholds, API consistency).

## Open questions к владельцу

1. Подтвердить стратегию для `marks_quota` в V2-2: удалить сразу или оставить временно как read-only mirror для миграционного окна?
2. Порог PWM (~100k): это глобальный абсолютный минимум или параметр на валидатора/epoch?
3. Порог marks (~1): трактовать как `>= 1 whole PWM staked` или как минимальный stake-unit после возможной future decimal policy?
4. Нужен ли в V2-2 публичный API-флаг "legacy_quota_mode" на переходный период, или достаточно только внутренней совместимости?

## Acceptance checklist (закрытие V2-1 Slice 0)

- [x] Подготовлен sprint-checklist для Slice 0.
- [x] Зафиксированы расхождения spec↔impl по `marks`/`marks_quota`.
- [x] Описаны точки списания burn и минимальный радиус будущей миграции.
- [x] Описаны текущие точки эмиссии PWM/marks.
- [x] Сформулированы RFC-first минимальные требования для порогов эмиссии.
- [x] Сформирован список узких next slices V2-1.
- [x] Вынесены открытые вопросы владельцу.

