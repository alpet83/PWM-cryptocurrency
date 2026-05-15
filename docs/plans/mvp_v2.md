---
name: MVP v2 Multi-Sprint Plan
overview: Roadmap MVP v2 — спецификации и приёмка, два типа учёта (PWM + единый баланс марок), эмиссия наград валидаторам по WP с сезонностью и порогами стейка, сжигание марок и клиенты; `marks_quota` в коде — заглушка прошлой версии, целевая модель без отдельной «сжигаемой квоты»; каденс 2 недели, demo-ready после каждого спринта.
todos:
  - id: v2-sprint-1-spec
    content: "Sprint V2-1: спецификации, критерии приёмки, синхронизация WHITE/RFC/matrix"
    status: completed
  - id: v2-sprint-2-double-balance
    content: "Sprint V2-2: PWM + единый marks в ядре и RPC; вывод/миграция marks_quota как заглушки"
    status: completed
  - id: v2-sprint-3-emission-whales
    content: "Sprint V2-3: политика эмиссии «китам» + сезонный множитель (лето)"
    status: completed
  - id: v2-sprint-4-burn-clients
    content: "Sprint V2-4: BURN_MARK end-to-end и согласование CLI/TUI/wallet"
    status: completed
  - id: v2-sprint-8-shard-sync
    content: "Sprint V2-8: same-shard sync v1 — слайсы 0–5 закрыты; приёмка Slice 6 (wave-pack) перенесена на V2-9 из‑за пивота консенсуса (см. § спринта)"
    status: completed
  - id: v2-sprint-9-validator-clone-attestation
    content: "Sprint V2-9: RFC 16 cluster attestation — несколько слайсов (ядро + волны 2–3 узла + ведомые вне кластера)"
    status: completed
isProject: false
---

# MVP v2 Multi-Sprint Plan

## Цель и формат

- **Цель:** ввести согласованный **экономический и UX-слой v2** поверх account-based ядра (без перехода на UTXO): **монеты (PWM)** и **один баланс марок (`marks`)** — накопление и сжигание ведутся по этому же счётчику. Поле **`marks_quota` / отдельная «сжигаемая квота»** в текущей реализации рассматривается как **заглушка прошлой версии**; в v2 её нормативно не развиваем, а **сворачиваем к единому `marks`** (конкретика миграции и совместимости — в спринтах и будущем RFC). **Эмиссия PWM** — по черновику whitepaper (крупные валидаторы, сезонность), с **порогом стейка для старта эмиссии монет** (ориентир порядка **~100 000 PWM**, число нефиксировано до RFC). **Эмиссия марок** — с **минимального стейка порядка 1 монеты**; точные формулы и таблицы — **в будущем RFC**, не в этом плане. Полный операторский путь **сжигания марок** во всех клиентах.
- **Связь с v1:** план [docs/plans/mvp_v1_testnet_multi-sprint.md](mvp_v1_testnet_multi-sprint.md) остаётся roadmap межшардового testnet; изменения v2 **не должны ломать** согласованные v1 инварианты (роуминг, `EXPORT`/`IMPORT`, replay-guard) без явного решения и тикета.
- **Каденс:** спринт = **2 недели**.
- **Критерий завершения спринта:** внутренний **demo-ready** инкремент (запуск, сценарий, короткий guide или строки в существующих tester-guides).

**Приоритет плана:** какой спринт (v1 vs v2) ведёт оркестратор в данный момент, задаёт **владелец**; этот документ может выполняться параллельно или после закрытия выбранных ног v1 при явной фиксации в `tasks/*.json`.

## Зависимости между спринтами

- **V2-1** задаёт нормативную семантику; без него недопустимы устойчивые правки кода.
- **V2-2** фиксирует контракт состояния и **RPC/API** (**два** пользовательских числа по активам: PWM и **`marks`**; без отдельного «burnable» в целевой модели).
- **V2-3** меняет `GenCfg` / seal блока и распределение наград; требует **замороженной** семантики **PWM + единого `marks`** из V2-1–2.
- **V2-4** опирается на стабильный API из V2-2 и проверяет полный путь после возможных изменений эмиссии в V2-3.

```text
V2-1 → V2-2 → V2-3
          ↘     ↓
            V2-4
```

Смысл: клиентский слой (V2-4) — после стабилизации контракта (V2-2); e2e с новой эмиссией — после V2-3.

## Обязательный ритуал в начале каждого спринта

- Перед стартом реализации: **`pwm-coding`** генерирует sprint-checklist (scope, demo-ready критерии, негативные проверки, риски, rollback).
- Чек-лист публикуется в `docs/reviews/` или в `notes` тикета до цикла coding/delegation.

## Обязанности оркестратора (постоянно)

- Консолидировать решения, компромиссы, открытые вопросы по v2.
- Конвейер: **`pwm-coding`** → **`pwm-testing`** → **`pwm-review`** на каждом код-слайсе; при широком охвате — опционально **`pwm-info`** один раз перед серией слайсов (карта файлов, `tasks/*-info.json`).
- Узкие handoff с явными acceptance criteria; не раздувать контекст чата полными логами тестов.
- См. [docs/AGENT_PROMPT_orchestrator.md](../AGENT_PROMPT_orchestrator.md).

## Базовые артефакты перед Sprint V2-1

- [docs/WHITE_SPEC_v0.md](../WHITE_SPEC_v0.md) (§5–7; v1 упоминает `marks_quota` — для v2 потребуется согласованное обновление под **единый `marks`**).
- [docs/rfc/7-tx-and-state-model.md](../rfc/7-tx-and-state-model.md) (`MarkBurnTx` — привести к списанию с **`marks`**, не с отдельной квотой, в отдельном RFC/ревизии при необходимости).
- **Будущий RFC (не блокер для этого .md):** пороги эмиссии (**PWM** с крупного стейка ~100k; **марки** со стейка от ~1 монеты), детальные формулы.
- [docs/WHITEPAPER_COVERAGE_MATRIX.md](../WHITEPAPER_COVERAGE_MATRIX.md).
- [docs/MVP-checklist.md](../MVP-checklist.md) — согласовать строки, затрагиваемые v2.
- **Черновик whitepaper (источник формул «китов» и сезонности):** путь к файлу фиксирует владелец в Sprint V2-1 (репозиторий или внешний документ); до фиксации — блокер для нормативной формулы в V2-3.

## Текущее состояние кода (ориентиры)

- Состояние: [crates/pwm-core/src/state.rs](../../crates/pwm-core/src/state.rs) — после коммита `6c52b71` единственный консенсусный счётчик марок в `pwm_core::State` это **`Account.marks`**. Отдельного рабочего зеркала/контура `marks_quota` в runtime-состоянии больше нет; legacy-поле `marks_quota` допускается только в старых snapshot JSON при загрузке (со strict validate в `pwmd`).
- Аккаунт: [crates/pwm-core/src/types.rs](../../crates/pwm-core/src/types.rs).
- Награда за блок, genesis marks и claim-путь: [crates/pwm-core/src/chain.rs](../../crates/pwm-core/src/chain.rs), [crates/pwm-core/src/state.rs](../../crates/pwm-core/src/state.rs); genesis: [crates/pwm-core/src/genesis.rs](../../crates/pwm-core/src/genesis.rs) (`block_reward`, `RewPol::ToProducerAccount`).
- HTTP API (балансы, `marks`): [crates/pwmd/src/api/types.rs](../../crates/pwmd/src/api/types.rs), [crates/pwmd/src/api/common.rs](../../crates/pwmd/src/api/common.rs).

---

## Sprint V2-1 (Week 1–2): Спецификации и критерии приёмки

**Цель:** зафиксировать нормативную модель v2 и **критерии приёмки**, по которым `pwm-testing` и `pwm-review` закрывают спринты.

**Scope:**

- **Две** пользовательски значимые величины по активам: **PWM** и **единый `marks`** (и при необходимости **стейк** как отдельное поле состояния, без второго «марочного» счётчика).
- Зафиксировать статус **`marks_quota`**: только **заглушка прошлой версии**; план **миграции к одному `marks`** (включая `BURN_MARK`, `accrue_marks`, снапшоты) — черновик в V2-1, детали порогов эмиссии — **вынести в отдельный RFC**.
- Ориентиры для будущего RFC (не обязаны быть финальными числами в этом спринте): **эмиссия монет** начинается со **стейка не ниже большого порога** (пример **~100 000 PWM**); **эмиссия марок** — со стейка от порядка **1 монеты**.
- Формула **эмиссии PWM «китам»** (доля стейка, топ-N, и т.д. по WP) и **сезонный множитель (лето)** — детерминизм от `(height, timestamp)` или календарного правила.
- Обновления `WHITE_SPEC` / RFC / matrix; **acceptance checklist** в `docs/reviews/`.

**Slices:**

- **Slice 0:** аудит против текущего кода; список расхождений spec↔impl.
- **Slice 1:** черновик нормативных параграфов + таблица инвариантов (RPC: PWM + `marks`; burn списывает **`marks`**).
- **Slice 2:** `pwm-review` — отсутствие противоречий с v1 межшардом и с [docs/rfc/7-tx-and-state-model.md](../rfc/7-tx-and-state-model.md).

**Acceptance criteria:**

- Есть согласованное определение **PWM + единого `marks`**, статуса `marks_quota` как наследия и **черновика порогов эмиссии** (крупный стейк для монет / малый для марок — с отсылкой на будущий RFC); формула эмиссии PWM по WP (или явный defer только численных коэффициентов).
- Чеклист приёмки опубликован; тикет(ы) `tasks/*.json` отражают статус V2-1.

**Файлы/модули (ориентир):**

- [docs/WHITE_SPEC_v0.md](../WHITE_SPEC_v0.md), [docs/rfc/7-tx-and-state-model.md](../rfc/7-tx-and-state-model.md), [docs/WHITEPAPER_COVERAGE_MATRIX.md](../WHITEPAPER_COVERAGE_MATRIX.md), `docs/reviews/sprint-v2-1-*.md`.
- RFC publication pack V2-1 (claims/burn): [docs/rfc/README-v2-claims-pack.md](../rfc/README-v2-claims-pack.md).

**Demo-ready output:** не код, а **подписанный владельцем/ревью** пакет спеки + checklist; при необходимости — сессия чтения для команды.

---

## Sprint V2-2 (Week 3–4): Балансы PWM и марок (единый `marks`)

**Цель:** привести **ядро и RPC** к семантике **PWM + один счётчик `marks`**; убрать из продукта отдельную «сжигаемую квоту» (кодовая **миграция** с `marks_quota` по плану V2-1) без полной полировки всего UX (UX — V2-4).

**Scope:**

- Реализация правил из V2-1 в `pwm-core`: **`BURN_MARK` и начисление марок опираются на `marks`**; `marks_quota` — удалить, склеить с `marks` на переходный период или оставить только как внутренний legacy до вычищения — по решению V2-1.
- Ответы `pwmd`: для марок пользователю **одно число** (`marks`); не вводить второе марочное поле в API.
- Юнит-тесты: `accrue_marks`, burn, несколько блоков — согласованы с единым `marks`.
- Минимальная документация в [docs/pwm-core.md](../pwm-core.md) при изменении контракта состояния.

**Slices:**

- **Slice 0:** freeze API-формы ответа account (поля, типы строк для u128).
- **Slice 1:** `pwm-core` правки + тесты.
- **Slice 2:** `pwmd` handlers/types + тесты API.
- **Slice 3:** snapshot/replay smoke при изменении `State` (если затронуто).

**Acceptance criteria:**

- RPC возвращает согласованно **PWM** и **`marks`** для локального аккаунта (без отдельного burnable в целевом контракте).
- `cargo test -p pwm-core` и релевантные `pwmd` тесты зелёные.
- Нет скрытого изменения семантики межшардовых tx без отдельного решения.

**Файлы/модули:**

- [crates/pwm-core/src/state.rs](../../crates/pwm-core/src/state.rs), [crates/pwm-core/src/types.rs](../../crates/pwm-core/src/types.rs)
- [crates/pwmd/src/api/handlers_account.rs](../../crates/pwmd/src/api/handlers_account.rs), [crates/pwmd/src/api/types.rs](../../crates/pwmd/src/api/types.rs), [crates/pwmd/src/api/common.rs](../../crates/pwmd/src/api/common.rs)

**Demo-ready output:** узел + `curl`/CLI к account endpoint: **PWM** и **`marks`** после стейка (и при готовности — после burn в V2-4).

---

## Sprint V2-3 (Week 5–6): Эмиссия через награды валидаторам («киты» и сезонность)

**Цель:** заменить/расширить упрощённую модель «фиксированный `block_reward` продюсеру» согласно **V2-1**, с **детерминированным** распределением в пользу крупных валидаторов (как определено в WP) и **сезонным замедлением (лето)**.

**Scope:**

- Расширение [crates/pwm-core/src/genesis.rs](../../crates/pwm-core/src/genesis.rs) (`GenCfg`, `RewPol` или новые поля: коэффициенты, **порог стейка для эмиссии PWM** (ориентир **~100 k**), **минимальный стейк для эмиссии марок** (ориентир **1**), календарные/сезонные параметры — точные значения по RFC).
- Изменение точки начисления в [crates/pwm-core/src/chain.rs](../../crates/pwm-core/src/chain.rs) (высота, время заголовка, снимок стейков; условное начисление PWM/марок от порогов).
- Тесты: таблица высот/времени → ожидаемые дельты балансов валидаторов; регрессия PoA round-robin.
- Совместимость **replay / snapshot:** при смене параметров наград — не ломать валидацию без явного bump метаданных; см. риски в [docs/reviews/sprint-15-s3-16-do-snapshot-root-cause.md](../reviews/sprint-15-s3-16-do-snapshot-root-cause.md) (расхождение `block_reward` / genesis bundle).

**Slices:**

- **Slice 0:** дизайн freeze + перечень полей `GenCfg` + migration note для существующих `genesis.json`.
- **Slice 1:** реализация функции награды и интеграция в seal.
- **Slice 2:** тесты + обновление devnet factory (`dev_net`).
- **Slice 3:** документация оператора (genesis, версионирование).

**Acceptance criteria:**

- Эмиссия за блок соответствует формуле V2-1; сезонный множитель воспроизводим в тестах.
- Существующие сценарии v1 не ломаются или явно помечены как требующие нового genesis (документировано).

**Файлы/модули:**

- [crates/pwm-core/src/genesis.rs](../../crates/pwm-core/src/genesis.rs), [crates/pwm-core/src/chain.rs](../../crates/pwm-core/src/chain.rs), [crates/pwm-core/src/block.rs](../../crates/pwm-core/src/block.rs) (если нужны поля заголовка)

**Demo-ready output:** прогон цепочки N блоков с несколькими валидаторами и таблицей наград; сценарий «лето» с уменьшенной эмиссией.

---

## Sprint V2-4 (Week 7–8): Сжигание марок и адаптация клиентов <!-- status: completed 2026-05-06 -->

**Цель:** **полный операторский путь** `BURN_MARK` в CLI, TUI и согласованных сообщениях; при необходимости — wallet/offchain в [crates/pwm-core](../../crates/pwm-core).

**Scope:**

- Команды/экраны: ввод суммы сжигания, отображение **текущего `marks`**, подпись, submit, ошибки (`InsufficientMarks` / эквивалент при нехватке **`marks`**, доменный контекст).
- Согласование с RPC из V2-2 (PWM + `marks`).
- Расширение [docs/tester-guide-cli-tui-scenarios.md](../tester-guide-cli-tui-scenarios.md) или отдельный фрагмент runbook.
- Негативные e2e: недостаточно **`marks`**; отклонённый beneficiary по policy.

**Slices:**

- **Slice 0:** UX freeze + список команд/флагов.
- **Slice 1:** `pwm-cli` burn path + тесты.
- **Slice 2:** `pwm-tui` burn path + интеграционные тесты при наличии harness.
- **Slice 3:** сквозной smoke с `pwmd` + документация; `pwm-review` на согласованность текстов ошибок.
- **Slice 4 (зарезервировано, финал roadmap v2 в этом документе):** полный проход **`pwm-review`** по воркспейсу на соответствие актуальным промптам агентов (**продакшен ≤ 4** сегмента `snake_case`, **тесты и тест-хелперы ≤ 5**), инвентаризация долга по именам/стилю и приоритизация follow-up для **`pwm-coding`**. Не блокирует функциональное **demo-ready** закрытие V2-4; запуск по решению владельца после стабилизации клиентского слоя или как отдельная «уборочная» нога.

**Acceptance criteria:**

- Оператор выполняет burn без чтения исходников; **PWM** и **`marks`** видны до/после операции.
- Ошибки совпадают по смыслу между CLI/TUI и узлом.

**Файлы/модули:**

- [crates/pwm-cli/src/main.rs](../../crates/pwm-cli/src/main.rs), [crates/pwm-tui/src/](../../crates/pwm-tui/src) (account view, tx submit)
- [crates/pwm-core/src/tx.rs](../../crates/pwm-core/src/tx.rs) (при доработке контура подписи)
- [docs/tester-guide-cli-tui-scenarios.md](../tester-guide-cli-tui-scenarios.md)

**Demo-ready output:** сценарий «stake → accrue → burn» с проверкой **`marks`** и одним негативным кейсом.

---

## Sprint V2-5 (Week 9–10): marks u32 + нормализация формулы начисления <!-- status: completed 2026-05-06 -->

**Цель:** снизить тип баланса марок с `u128` до `u32` и одновременно нормализовать семантику генерации: **1 целый PWM застейкан на 1 час = 1 марка**. Устраняет гиперинфляцию марок (raw-единицы × часы), выравнивает CUDA-производительность, сокращает RPC-трафик.

**Ключевые решения (зафиксированы владельцем 2026-05-06):**

- **Формула:** `matured = (staked / PWM_RAW_SCALE) * hours` — где `PWM_RAW_SCALE = 1_000_000`; деление целочисленное, без дробей.
- **Тип `Account.marks`: `u128 → u32`**; `saturating_add`/`saturating_sub` сохраняются.
- **`CLAIM_ALL: u32 = u32::MAX`** (заменяет `u64::MAX as u128`).
- **Поля транзакций:** `BurnMark.mark_amount` и `ClaimTx.claim_units` — `u32`.
- **Snapshot migration:** при загрузке legacy-снапшота делить `marks` на `PWM_RAW_SCALE`, результат насыщать до `u32::MAX`.
- **`DEF_MARKS_STAKE_MIN`** обновить до `PWM_RAW_SCALE` (= 1 целый PWM) — явное пороговое значение вместо `1` raw.
- **Протокол:** при bump-версии genesis/schema фиксировать как `marks_scale_v2`.

**Scope:**

- `pwm-core`: `Account`, `matured_units_available`, `accrue_marks`, `accrue_marks_v2`, `apply_auto_claim`, `tx.rs` (`CLAIM_ALL`, `BurnMark`, `ClaimTx`), `genesis.rs`.
- `pwmd`: `AcctOut.marks` (сериализация — u32 в JSON).
- `pwm-cli`: `fetch_marks`, `cmd_tx`, `cli_cmd` (тип claim_units).
- `pwm-tui`: `AcctRow.marks`, `BurnForm.marks_available`, F5 auto-claim/burn flow.
- Docs: `docs/rfc/12-claim-maturity-and-state-model.md`, `docs/rfc/11-burn-purpose-and-claim-tx.md`, `docs/WHITE_SPEC_v0.md`.

**Slices:**

- **Slice 0:** RFC/doc freeze — обновить rfc-12, rfc-11, WHITE_SPEC; опубликовать `docs/reviews/v2-5-slice0-freeze-*.md`.
- **Slice 1:** `pwm-core` — тип, формула, CLAIM_ALL; migration в snapshot-загрузке; `cargo test -p pwm-core` зелёный.
- **Slice 2:** `pwmd` API + клиенты (`pwm-cli`, `pwm-tui`); `cargo check --workspace` зелёный.
- **Slice 3:** `pwm-testing` → `pwm-review`; граничные юнит-тесты (saturation, migration, 0-marks).

**Acceptance criteria:**

- `Account.marks` — `u32` во всей кодовой базе; нет `u128` marks в публичных API.
- Формула: тест `staked=1_000_000 raw (1 PWM), hours=1 → matured=1`; `staked=500_000 (0.5 PWM), hours=10 → matured=0` (truncation).
- `CLAIM_ALL = u32::MAX`; sentinel не конфликтует с реальным балансом (ноль-matured возвращает `ClaimOverMatured`, не паникует).
- Snapshot с legacy `marks=1_000_000_000_000` загружается как `marks=min(1_000_000, u32::MAX)=1_000_000`.
- `cargo test --workspace` зелёный.

**Файлы/модули (ориентир):**

- [`crates/pwm-core/src/state.rs`](../../crates/pwm-core/src/state.rs), [`crates/pwm-core/src/tx.rs`](../../crates/pwm-core/src/tx.rs), [`crates/pwm-core/src/genesis.rs`](../../crates/pwm-core/src/genesis.rs)
- [`crates/pwmd/src/api/types.rs`](../../crates/pwmd/src/api/types.rs)
- [`crates/pwm-cli/src/rpc_helpers.rs`](../../crates/pwm-cli/src/rpc_helpers.rs), [`crates/pwm-cli/src/cmd_tx.rs`](../../crates/pwm-cli/src/cmd_tx.rs)
- [`crates/pwm-tui/src/models.rs`](../../crates/pwm-tui/src/models.rs), [`crates/pwm-tui/src/burn_form.rs`](../../crates/pwm-tui/src/burn_form.rs), [`crates/pwm-tui/src/tui_loop.rs`](../../crates/pwm-tui/src/tui_loop.rs)

**Demo-ready output:** тест или `cargo test` матрица границ `u32`: saturation, migration, CLAIM_ALL sentinel round-trip.

---

## Sprint V2-6 (Week 11–12): TUI Stake/Unstake + F5 auto-claim UX <!-- status: completed 2026-05-06 -->

**Цель:** закрыть последний UX-пробел: добавить stake/unstake прямо в TUI и упростить F5 burn через авто-клейм при открытии.

**Ключевые решения (зафиксированы владельцем 2026-05-06):**
- **F7** — Stake form; **Shift+F7** (fallback F8) — Unstake form; шапка показывает текущий баланс PWM / staked.
- **F5 flow:** синхронный `ClaimTx(CLAIM_ALL)` при открытии → обновить marks → открыть BurnForm. При ошибке claim — открыть BurnForm с текущим балансом.
- **Hint:** если `staked==0 AND marks==0` — показать infobox вместо формы.
- **Удалить `marks_modal.rs`** и wall-clock guard TUI — больше не нужны.

**Acceptance criteria:**
- F7 / Shift+F7 формы работают, StakeTx/UnstakeTx уходят на ноду, баланс обновляется в TUI.
- F5 hint при нулевом стейке и марках.
- F5 auto-claim → BurnForm flow работает (claim success и fail).
- `marks_modal.rs` удалён, `cargo check --workspace` зелёный.
- Footer TUI обновлён (F7/Shift+F7 подсказки).

**Файлы (ориентир):**
- `crates/pwm-tui/src/tui_loop.rs`, `marks_modal.rs` (удалить), `tx_submit.rs`, `lib.rs`
- Новый `crates/pwm-tui/src/stake_form.rs`

**Task:** `tasks/20260506-v2-sprint6-tui-stake.json`

---

## Sprint V2-7 (Week 13): burn UX fixes + remove accrue_marks + genesis marks <!-- status: completed 2026-05-06 -->

**Цель:** закрыть 6 дефектов обнаруженных при живом TUI-тестировании.

**Fixes:**
1. Auto-fill beneficiary из правой панели TUI при открытии F5 burn form.
2. Убрать ошибочную валидацию кросс-доменного beneficiary в burn (`burn_ctx_source_dom` → удалить из `tx_policy.rs` + `tx.rs`).
3. При автоблокировке кошелька во время burn → внятное сообщение "Wallet is locked — press F3 to unlock".
4. Двойной баланс в TUI: spendable / staked.
5. Компактный label `F7/⇧F7 Stake/Unstake` в footer.
6. Убрать вызовы `accrue_marks` / `accrue_marks_v2` из `Chain::seal()`; добавить genesis-marks = `bal / 1_000_000` при инициализации state из genesis.

**Task:** `tasks/20260506-v2-sprint7-burn-fixes.json`

---

## Sprint V2-8 (Week 14-15): same-shard chain sync v1 (mempool + block sync + catch-up)

**Цель:** устранить архитектурный пробел, когда ноды одной шарды не синхронизируют историю по сети: добавить минимально работоспособный протокол same-shard sync с поэтапным внедрением и безопасными фиче-флагами.

**Основание:** [docs/reviews/20260508-shard-sync-architecture-and-sprint.md](../reviews/20260508-shard-sync-architecture-and-sprint.md), info-карта `tasks/20260508-shard-sync-sprint-design-info.json`.

**Ключевые решения (рамка спринта):**
- Реализуем гибридный путь: **A-min (wire-level live sync)** как обязательная основа + **B (epoch catch-up fallback)** как ускоритель догонки.
- До кодовых правок wire/state обязателен **Slice 0 RFC freeze** (протокол сообщений, fork-choice v1, anti-DoS, capability negotiation).
- Сохраняем совместимость со старыми пирами через feature gates/version negotiation; legacy peers не должны падать на неизвестных сообщениях.
- Automated waves (Slice 6) MUST опираться на зафиксированный baseline RFC 0015: детерминированный P0 proposer по `height + fixed validator set order` и bounded/monotonic обработку `finalized_height`.
- Для wave-проверок baseline `finalized_height` трактуется от локально наблюдаемого PoA finalized prefix (MVP source-of-truth), а регрессии peer-значений считаются stale и не должны откатывать локальный baseline.

**Slices:**
- **Slice 0 (RFC freeze):** новый/обновлённый RFC на same-shard sync v1, freeze message-contract и acceptance.
- **Slice 1 (wire skeleton):** типы сообщений sync (`headers/blocks/inv/get*`) + capability gates без полного apply; трассируемость по message taxonomy фиксируется через `docs/rfc/15-same-shard-sync-v1.md` §11 (Required now vs Deferred from §6).
- **Slice 2 (mempool gossip):** best-effort обмен pending tx между native peers с dedup/rate-limit.
- **Slice 3 (header-first + block apply):** догон tip и live update цепочки по сети, базовый fork-choice v1.
- **Slice 4 (epoch catch-up fallback):** chunk/epoch transfer для быстрой начальной догрузки длинной истории.
- **Slice 5 (observability + chaos/docs):** метрики, деградационные сигналы, operator runbook и negative suite.
- **Slice 6 (automated post-sprint waves):** автоматические многоволновые сценарии для проверки синхронности мемпула/цепочки и догонки нод в группе (2-ноды, затем 3-ноды), включая управляемую остановку по высоте.

**Acceptance criteria:**
- Для двух нод одной шарды: ведомая нода может догнать отстающий tip по сети и продолжать live-sync.
- Mempool gossip в native-паре доставляет валидные tx без дубликатного шторма.
- При недоступности live fetch допустим fallback через epochs (если включён), без нарушения валидности chain state.
- Совместимость старого/нового peer protocol задокументирована и проверена smoke-тестами.
- Документация оператора и RFC фиксируют ограничения v1 (включая anti-DoS лимиты и поведение при конфликтующих ветках).
- Есть автоматический wave-pack post-sprint:
  - **Wave A (2 ноды):** синхронность мемпула/блоков до 2 checkpoint windows и совпадение epoch/block файлов после одновременной остановки.
  - **Wave B (joiner):** третья нода подключается к двум рабочим, догоняет историю, затем повторяет инварианты Wave A в группе из 3.
  - **Wave C (negative/chaos):** drop/reconnect/повреждённые кадры/ограничения профилей не ломают восстановление синка.
- Для точности wave-тестов предусмотрен debug-параметр остановки ноды по высоте (`--debug-stop-height`, test-only).

**Статус и перенос приёмки (фиксация):** по слайсам **0–5** спринт доведён до **`READY_FOR_NEXT_BRANCH`** ([обёртка post-sprint](../reviews/20260508-v2-8-post-sprint-wrap-up.md)). Отдельно зафиксирован затык на пути **нескольких нод одной шарды с одной и той же identity валидатора**: при «хаотическом» состязании за seal наблюдались **расходящиеся цепочки** и **недетерминизм заголовков** (см. диагностику Wave A / tip hash в ревью slice 6); **полностью вылизать это только средствами sync и fork-choice**, без изменения модели «кто имеет право печатать блок», не получилось в разумных рамках. **Принято стратегическое решение:** не продолжать опираться на такое состязание, а целиться в **одного активного пропозера** и **аттесторов** (**[RFC 16](../rfc/16-validator-clone-attestation.md)**, Sprint **V2-9**). Следствие для чеклистов: исходные **Wave A / B / C** как «зелёный» gate на **старом** multi-sealer-пути **не закрываются**; **функциональная приёмка** многонодовых сценариев и **ведомых вне кластера печати** **переносится на V2-9** (новые тесты под новый контракт). Транспорт и sync **V2-8** остаются базой для доставки блоков подписчикам шарды.

**Файлы/модули (ориентир):**
- `crates/pwmd/src/transport/**` (wire, session, tick, policy)
- `crates/pwmd/src/lifecycle.rs` (apply/bootstrapping hooks)
- `crates/pwmd/src/snapshot/**` (epoch catch-up integration)
- `crates/pwm-core/src/chain.rs`, `crates/pwm-core/src/block.rs` (валидация входящих блоков в рамках v1 правил)
- `docs/rfc/8-shard-runtime-identity-and-peering.md`
- `docs/rfc/15-same-shard-sync-v1.md` (same-shard sync v1 contract; RFC freeze — Slice 0)

**Task (инициация):** `tasks/20260508-shard-sync-sprint-design.json`

---

## Sprint V2-9 (Week 16–17): Validator clone cluster attestation — RFC 16 (несколько слайсов)

**Цель:** перевести **Draft [RFC 16](../rfc/16-validator-clone-attestation.md)** (Variant A) в **управляемую реализацию за несколько слайсов** под **feature flags**, без включения по умолчанию в testnet до чеклиста владельца. В спринт входят **не только код ядра**, но и **многонодовые проверки** (2 и 3 узла кластера) и сценарий **ведомых нод шарды**, которые **не входят в кластер аттестации**, но должны **удерживать согласованное шардовое состояние** с цепочкой, исходящей от кластера.

**Основание:** [docs/reviews/20260511-single-sealer-S3-cluster-consensus-design.md](../reviews/20260511-single-sealer-S3-cluster-consensus-design.md), родительский трек `tasks/20260509-single-sealer-failover-profiles.json` (S3); спецификация **§16 Implementation readiness** в RFC 16 v0.4.6+.

**Слайсы (планируемая декомпозиция внутри спринта):**

- **Слайс A — ядро кластера:** узлов консенсуса **не более трёх** (§7.2 RFC); роли **proposer / attester** через конфиг/CLI; интеграция с **seal path** и **S2 lease** по §8 RFC (**не** смешивать кворум с lease); **§6.1** (bounded `T_tx_catchup`, лог `attest_tx_lag` / аналог); транспорт peer/wire+capability **или** lab OOB (§10) — фиксация в sprint-checklist.
- **Слайс B — волна из двух узлов кластера:** автоматизированные или полуавтоматические сценарии **2 узлов** (happy-path кворум, негатив без кворума, инжект fault по RFC §11 где применимо).
- **Слайс C — три узла + ведомые вне кластера:** волна **3 узлов** кластера (в т.ч. **2-of-3** и деградации); отдельно топология **«кластер печатает шард» + ≥1 нода той же шарды без ролей кластера** — проверка, что **ведомая** догоняет tip и **state шарды** согласован с источником блоков (опора на **same-shard sync v1**, спринт **V2-8**; если baseline sync ещё не готов — минимальная lab-топология и явный допуск в sprint-checklist).

**Вне скоупа спринта (как и раньше):** динамический join (Appendix B.5), отбор **k** из большого relay pool (§12.4), доказательство кворума в header.

**Зависимости / координация:** спринт **ортогонален** экономике v2 (burn/marks). **Наследование от V2-8:** многонодовые и «ведомый» сценарии, которые в плане V2-8 остались на **Slice 6** (wave-pack) из‑за пивота от multi-sealer конкуренции к **single proposer + attest**, **переносятся сюда** — новые тесты перекрывают цели старых волн на **новом** консенсусном контракте; sync-слой V2-8 по-прежнему используется для подтягивания цепочки у не-печатающих нод. С **V2-8** — **логическая связка** для приёмки «ведомый не в кластере»: без работающего same-shard пути проверка сводится к оговорённому baseline или переносится подчинённым тикетом. **Конфликт по `pwmd` transport/touchpoints** между V2-8 и V2-9 возможен — приоритет гейтит **владелец**; sequencing в `tasks/*.json`.

**Acceptance criteria:**

- За флагом: happy-path **2-of-2** и **2-of-3** (где применимо) attest → seal; негативы: нет кворума → нет seal; интеграционные тесты / lab-волны с инжектом fault по RFC §11.
- Прогон **двух узлов** и **трёх узлов** кластера задокументирован (автотесты и/или воспроизводимый runbook-шаг).
- Сценарий **ведомой ноды той же шарды без членства в кластере:** после стабилизации высоты — **согласованность шардового состояния** с ожидаемым tip (критерии конкретизируются в sprint-checklist: сравнение height/hash/state snapshot по согласованным точкам остановки).
- Логи содержат событие при срабатывании §6.1.
- Документация: runbook-заметка + ссылка на RFC 16; default **off** для публичного testnet.

**Task (инициация):** `tasks/20260513-v2-sprint9-rfc16-cluster-attestation.json`  
**Sprint checklist (операторский):** [docs/reviews/20260509-v2-9-rfc16-sprint-checklist.md](../reviews/20260509-v2-9-rfc16-sprint-checklist.md)

**Статус закрытия спринта (зафиксировано 2026-05-22):** спринтовой трек **done** (`tasks/20260513-v2-sprint9-rfc16-cluster-attestation.json`); объединённый приёмочный gate Slice B+C — **PASS** ([`docs/reviews/20260510-v2-9-slice-bc-review.md`](../reviews/20260510-v2-9-slice-bc-review.md)). Дополнительно к минимуму RFC §11 fault: harness **`cluster_partition_attest_stuck`** ([`tasks/20260522-v2-9-optional-partition-lite-fault.json`](../../tasks/20260522-v2-9-optional-partition-lite-fault.json)). Расширенная аналитика длинных логов (timeline, tail-only, словарь `pwmd-peer-*`) — **не** обязательна для закрытия V2-9; при необходимости — backlog для эксплуатации/V3. Pre-V3 concept documents are intentionally excluded from the public MVP v2 package while they remain active drafts.

---

## Межспринтовые гейты качества (каждый спринт)

- **Spec Gate:** `pwm-review` — нет новых противоречий между WHITE / RFC / matrix и реализацией.
- **Demo Gate:** воспроизводимый запуск + 1 happy-path + ≥2 negative сценария (где применимо).
- **Regression Gate:** не ломать согласованные v1 потоки без явного ADR/тикета.
- **Manual Visual Gate (условный):** TUI — при нестабильности UX или лагах.

## Риски и контрмеры

- **Риск legacy-снапшотов с `marks_quota`:** контрмера — держать единый runtime-счётчик `Account.marks`, а совместимость ограничивать только загрузкой старых snapshot JSON под строгой валидацией (`pwmd`); проверить сценарии `accrue_marks`/`BURN_MARK` на регрессию относительно единого `marks`.
- **Replay / snapshot mismatch** после смены `GenCfg`: контрмера — метаданные снапшота / строгая проверка bundle genesis (см. обсуждения sprint-15); явное версионирование конфига.
- **Разъезд CLI/TUI и RPC:** контрмера — один источник имён полей в спеке V2-2 и контрактные тесты на JSON при необходимости.

## Декомпозиция на таски

- На каждый спринт: минимум тикеты **`pwm-coding`** (реализация + узкие правки docs) и **`pwm-testing`** (регрессия, негативы, отчёт demo).
- В начале спринта — тикет/checklist от `pwm-coding` для sprint-checklist в `docs/reviews/`.
- Вести `tasks/<id>.json`: `in_progress` → `done|blocked`, `delegations[]`, ссылки на review.
- **Автоматический смок хвоста V2-9 / CY lab (операторский):** [`scripts/cy_cluster_mvp_v2_tail_smoke.ps1`](../../scripts/cy_cluster_mvp_v2_tail_smoke.ps1) — preflight `target/debug`, подъём 2 или 3 узлов (`-NodeCount`), допуск расхождения `head`, опционально `-RelayBurn` через RPC attester; режим `-Attach` при уже запущенном лабе. Несколько IP и те же порты: [`docs/runbooks/cy-lab-multi-ip-same-ports.md`](../runbooks/cy-lab-multi-ip-same-ports.md). Тикет: [`tasks/20260509-mvp-v2-tail-automated-smoke.json`](../../tasks/20260509-mvp-v2-tail-automated-smoke.json).

---

_Конец плана MVP v2._  

_Примечание (2026-05-22): дорожка **same-shard sync (V2-8 слайсы 0–5) + cluster attestation RFC 16 (V2-9)** зафиксирована как **закрытая** по приёмке в репозитории; YAML-статусы выше отражают публичный snapshot этого плана после подготовки к зеркалу._
