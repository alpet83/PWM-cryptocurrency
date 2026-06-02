---
name: MVP v5 Tokenomics Hardening Plan
overview: Roadmap MVP v5 — укрепление токеномики и дизайн IPv4 distribution: ленивое накопление марок до потолка u32::MAX без demurrage (staked-only, часовая модель), динамическая float инфляция (~5% annual), on-chain ClaimIPv4Batch tx, реализация отложенной активации политик (ADR 0005), spec-уровень address flags + conservation (ADR 0006), ADR по governance аренды доменов (ADR 0007), TUI saturation display, CLI/genesis hardening. 8 спринтов.
todos:
  - id: v5-sprint-1-spec-adr-freeze
    content: "Sprint V5-1: freeze lazy marks model (staked-only, hours), float inflation spec, ADR 0005 normalization, ADR 0006 address flags + conservation, ADR 0007 domain lease governance"
    status: completed
  - id: v5-sprint-2-core-model
    content: "Sprint V5-2: core state model — marks_last_block field, inflation config в GenCfg, ClaimIPv4Batch tx shape, snapshot schema v3 bump"
    status: completed
  - id: v5-sprint-3-marks-inflation
    content: "Sprint V5-3: реализация lazy marks engine (staked-only, satur_hours ceil) и float inflation formula (season_coeff_ppm, dynamic block_reward)"
    status: completed
  - id: v5-sprint-4-deferred-activation
    content: "Sprint V5-4: реализация ActivationMode::Deferred с activate_at_height, normative update RFC 6/7, policy evaluator extension"
    status: completed
  - id: v5-sprint-5-ipv4-claim-onchain
    content: "Sprint V5-5: ClaimIPv4Batch on-chain validation/apply в state::apply_tx, тесты happy/reject paths"
    status: completed
  - id: v5-sprint-6-tui-marks-saturation
    content: "Sprint V5-6: TUI marks saturation column/bar — effective_marks display, saturation percentage, zero-balance safety"
    status: completed
  - id: v5-sprint-7-cli-genesis-doc
    content: "Sprint V5-7: CLI enhancements (account inspect с marks detail, tx-policy-set --activate-at-height), 21B genesis design doc"
    status: completed
  - id: v5-sprint-8-closeout
    content: "Sprint V5-8: integrated devnet gate и closeout — checklist, CONCEPT_ROADMAP, GLOSSARY, CHANGELOG"
    status: in_progress
isProject: false
---

# MVP v5 Tokenomics Hardening Plan

## Цель и формат

- **Цель:** привести экономику марок и инфляции в соответствие с Whitepaper; заложить on-chain фундамент IPv4 distribution; реализовать отложенную активацию политик (ADR 0005); зафиксировать address flags + conservation в спецификации (ADR 0006); закрыть governance ADR для аренды доменов (ADR 0007).
- **Главный demo-ready результат:** оператор видит лениво накапливаемый marks-баланс со степенью насыщения в TUI; блок-ревард меняется динамически по сезонному коэффициенту; `tx-policy-set --activation deferred --activate-at-height N` проходит end-to-end от CLI до policy evaluator; on-chain `ClaimIPv4Batch` tx принимается нодой с проверкой подписи registry.
- **Scope:**
  - Ленивое накопление марок (staked-only): `marks_last_block`, формула через часы насыщения, sans demurrage/TTL:
    ```
    delta_blocks  = current_height - marks_last_block
    delta_hours   = delta_blocks / blocks_per_hour
    staked_coins  = floor(account.staked_pwm_raw / 1_000_000)
    marks_rate    = marks_per_coin_per_hour          // GenCfg, default = 1

    remaining     = u32::MAX - stored_marks
    satur_hours   = ceil(remaining / (staked_coins * marks_rate))
    effective_hours = min(delta_hours, satur_hours)
    generated     = (staked_coins * marks_rate * effective_hours) as u32
    effective_marks = min(u32::MAX, stored_marks + generated)
    ```
    При `staked_coins == 0` → `generated = 0`. Потолок `u32::MAX` достигается через `satur_hours` (ceil) и финальный clamp — overflow u64 невозможен.
  - Float inflation: динамический `block_reward` на базе `season_coeff_ppm`, целевое ~5% annual
  - Deferred activation (ADR 0005 → code): `ActivationMode::Deferred`, `activate_at_height: u64`, auto-activate по высоте в evaluator
  - Address flags + conservation flag (ADR 0006 — **spec only**, без runtime в V5)
  - Domain lease governance (ADR 0007 — **spec only**)
  - IPv4 Claim on-chain: `ClaimIPv4Batch { phase, batch_root, registry_sig }` tx-тип, `GenCfg.ipv4_claim_phases`
  - TUI: marks saturation bar / percentage, inflation stats
  - CLI: `--activation deferred --activate-at-height` для `tx-policy-set`; `account inspect` с marks saturation
  - 21B genesis design doc (allocation table structure, IPv4-weighted formula)
- **Out of scope для V5:** address flags runtime enforcement, conservation-flag delayed Transfer в mempool/seal, production off-chain claim registry, domain lease auction runtime, PoS validator admission (V6), PQC signatures (V8+), Nginx/email reference implementations (V7).
- **Критерий завершения спринта:** каждый спринт оставляет воспроизводимый артефакт: RFC/ADR freeze, model+tests, engine tests, evaluator tests, CLI/TUI demo, или integrated review.

## Принятые решения V5

- **Marks без demurrage (staked-only, RFC 0012 v2):** модель ленивого начисления через часы насыщения. Начисление только на `staked_pwm` — liquid balance марки не генерирует.
  Поля `marks_expiry_block`, `last_claim_unix_time`, `last_claim_anchor_ref`, `free_claim_utc_day` **удаляются**; `TxBody::Claim` **удаляется**.
  Единственный cursor: `marks_last_block: u64` (высота последнего touch). Единицы стейка: `whole_pwm_staked = floor(staked_raw / 1_000_000)` — сохраняется из RFC 0012 v1.
  Timing: `delta_hours = floor((current_height - marks_last_block) / blocks_per_hour)` (GenCfg `blocks_per_hour`, default 3600) — блоковая база детерминированнее chain timestamp.
  GenCfg параметры: `blocks_per_hour` (default 3600), `marks_per_coin_per_hour` (default 1).
  Формула: `generated = (whole_pwm_staked * rate * min(delta_hours, satur_hours)) as u32`, где `satur_hours = ceil((u32::MAX - stored_marks) / (whole_pwm_staked * rate))`; итог — `min(u32::MAX, stored_marks + generated)`.
  Потолок `u32::MAX` достигается через ceiling `satur_hours` и финальный clamp — floor давал permanent stall при `remaining < per_hour` (на 1M PWM стейке ~967k marks ниже cap).
- **Float inflation:** не фиксированный `block_reward`, а вычисляемый: `reward = base_emission_per_block * season_coeff_ppm / 1_000_000`. `season_coeff_ppm` уже существует в `GenCfg` как основа; V5 делает его применение обязательным в chain seal. Целевой диапазон: ~5% annual при текущей stake participation. Детали формулы — в [RFC 0019](../rfc/19-float-inflation.md).
- **Deferred activation:** только расширение `ActivationMode` enum — добавляем вариант `Deferred { activate_at_height: u64 }`. Никакого auto-activate отдельной транзакцией — высота цепи проверяется в `evaluate_policy`. Детерминистичность: только высота цепи, не wall-clock. Scope ограничен ADR 0005.
- **Address flags (ADR 0006, spec only):** Флаги **уже вшиты в адрес** (брутфорсятся при генерации с V1 — часть bech32DX структуры). Добавлять новое поле в `Account` не требуется. V5 задача ADR 0006 — **формализовать семантику нескольких младших битов** этого существующего адресного поля: определить, какие значения обязаны проверять валидаторы (например, бит `COSIGN_NON_DISABLEABLE`, бит `CONSERVATION`). До V6 валидаторы флаги **не проверяют**; ADR 0006 фиксирует только нормативный смысл битов.
- **ClaimIPv4Batch:** новый вид `TxBody`, не вариация `Transfer`. Принимается только от authority/registry-адреса из `GenCfg.ipv4_claim_phases[*].registry_address`. On-chain state: `Account.ipv4_claimed_phase: Option<u8>` — минимальная запись, не полный bitmap (он остаётся off-chain).
- **Snapshot schema v3:** V5 добавляет `marks_last_block`, `deferred_policies`, `ipv4_claimed_phase` к Account; `ipv4_claim_phases` к GenCfg; `schema_version` bump до v3 с миграционным gate. Поле `address_flags` в Account **не добавляется** — флаги хранятся в самом адресе.
- **Public JSON encoding for V5 `u128`:** `staked_pwm_raw`, `fee`, `base_emission_per_block`, `legacy_block_reward` и другие публичные экономические `u128` поля сериализуются на JSON/API/operator surfaces как decimal string; binary/state-hash representations этим правилом не меняются.

## Зависимости между спринтами

```text
V5-1 ──► V5-2 ──► V5-3 ──► V5-6 (TUI)
                  V5-3 ──►┐
                  V5-4 ──►┴─► V5-7 (CLI + genesis doc)
          V5-2 ──────────────► V5-5 (IPv4 Claim on-chain)

V5-5, V5-6, V5-7 ──► V5-8 (closeout)
```

Смысл: spec/ADR freeze нужен до кодовых слайсов; core model (schema, fields, tx shape) — до engine logic; V5-5 (IPv4 Claim) требует только core model из V5-2, не зависит от marks engine; V5-6 (TUI) требует marks engine из V5-3; V5-7 (CLI) требует и marks engine (V5-3), и deferred activation CLI backend (V5-4); closeout ждёт всех трёх прикладных спринтов.

## Базовые артефакты перед Sprint V5-1

- [CONCEPT_ROADMAP.md](../CONCEPT_ROADMAP.md) — секция **MVP V5**, задачи экономики, IPv4 claim window, policy/address hardening, критерии готовности V5
- [DRAFT_WHITEPAPER-ru.md](../../DRAFT_WHITEPAPER-ru.md) — §3 токеномика (марки, инфляция), §5 глупые контракты (conservation address)
- [docs/adr/0002-ipv4-claiming-design.md](../adr/0002-ipv4-claiming-design.md) — ADR по IPv4 Claim (V3 closeout)
- [docs/adr/0004-cleanup-chain-bootstrap-snapshot-and-anchoring.md](../adr/0004-cleanup-chain-bootstrap-snapshot-and-anchoring.md)
- [docs/adr/0005-policy-deferred-activation.md](../adr/0005-policy-deferred-activation.md) — черновик, нуждается в нормализации
- [docs/rfc/6-policy-engine.md](../rfc/6-policy-engine.md), [docs/rfc/7-tx-and-state-model.md](../rfc/7-tx-and-state-model.md) — policy baseline; нужно normative обновление под Deferred
- [docs/plans/mvp_v4.md](mvp_v4.md) — итоговое состояние V4 как baseline
- [docs/MVP-checklist.md](../MVP-checklist.md) — backlog/defer rows связанные с V5

## Обязательный ритуал в начале каждого спринта

- Создать/обновить `tasks/<id>.json` со статусом `in_progress`, scope, acceptance criteria и planned delegations.
- Если спринт широкий (несколько кратов/файлов), запустить `pwm-info` на reuse-карту до делегирования.
- Для кодовых слайсов держать конвейер **`pwm-coding` → `pwm-review` → `pwm-testing`** (review до testing — spec/contract gate раньше executable gate).
- Для doc-only слайсов: оркестраторская правка `docs/`, финальный quality gate — `pwm-review`.

## Обязанности оркестратора

- Не размывать V5 в PoS admission, domain auction runtime, PQC: эти темы явно в backlog/V6+.
- Не реализовывать address flags enforcement в V5: ADR 0006 — только spec; runtime — V6.
- Не реализовывать conservation flag delayed Transfer в V5: mempool/seal semantics — отдельный V6 ADR.
- Не менять existing `Transfer/Stake/BurnMark/PolicyTx` wire format: все V5 добавления — additive.
- Не допускать dynamic dispatch / side effects в marks calculation или inflation formula: pure arithmetic only.
- В каждом handoff для `pwm-*` субагентов явно требовать skill `colloquium-cqds-mcp`.
- Вести `tasks/*.json`: delegations, token estimates, artifacts, review links, status.

---

## Sprint V5-1: Spec/RFC/ADR freeze

**Цель:** зафиксировать все нормативные границы V5 до кода.

**Scope:**

- **RFC 0012 major revision (v2):** V5 полностью заменяет модель explicit ClaimTx + anchor_ref на lazy accumulation. Нормативные изменения:
  - Базa времени: `delta_seconds / 3600` → `delta_blocks / blocks_per_hour` (GenCfg, default 3600); детерминированнее block timestamp.
  - Удалить: `anchor_ref`, `last_claim_anchor_ref`, continuity-breaking rule, `last_free_claim_utc_day`, `CLAIM_ALL = u32::MAX`.
  - Ввести: `marks_last_block: u64` как единственный state cursor; saturation ceiling (`satur_hours`); touch-semantics на Transfer/Stake/Unstake/BurnMark/PolicyTx/INIT.
  - Сохранить: `whole_pwm_staked = floor(staked_raw / 1_000_000)`, rate `1 whole PWM × 1 hour = 1 mark`, staked-only generation.
  - Обновить validation semantics: убрать over-claim, continuity, free-day инварианты; ввести saturation clamp.
- **RFC 0011 addendum:** Депрецировать/удалить `ClaimTx` (`tx_type: "claim_mark"`): lazy model делает явный клейм избыточным. BurnMarkTx v2 — без изменений. Удалить из active scope error codes: `CLAIM_DELTA_INVALID`, `CLAIM_ANCHOR_RANGE_INVALID`, `CLAIM_ANCHOR_CONTINUITY_BROKEN`, `CLAIM_OVER_MATURED`, `FREE_CLAIM_DAILY_LIMIT`.
- **RFC 0013 addendum:** Из claim policy matrix удалить ClaimTx phase path и anchor incompatibility predicates (`E_ANCHOR_*`, `E_FREE_CLAIM_DAILY_LIMIT`, `E_CLAIM_UNITS_INVALID`, `E_CLAIM_OVER_MATURED`). Burn policy matrix — без изменений. Добавить: touch-semantics policy, saturation no-op rule.
- Обновить или создать экономический RFC (RFC 19, так как RFC 17 уже занят runtime-log-control) по float inflation: `base_emission_per_block`, `season_coeff_ppm`, target ~5% annual, правило применения в `Chain::seal`.
- Нормализовать ADR 0005 до статуса **Accepted**: устранить черновые оговорки, зафиксировать правило конфликта `ActivatePolicy` до достижения высоты, уточнить genesis-height convention.
- Написать **ADR 0006: Address flags и non-disableable profiles** (нормативная спецификация битов, без нового Account field): зафиксировать, что флаги **уже являются частью адреса** (brute-forced при генерации с V1), определить семантику нескольких **младших бит** адресного поля — например, бит `COSIGN_NON_DISABLEABLE` (policy cosign_required не может быть отключена policy-транзакцией), бит `CONSERVATION` (24-часовое окно задержки Transfer, enforcement в V6), — описать ожидаемое поведение валидаторов при V6 enforcement. Явно зафиксировать: поле в Account **не добавляется**, флаги читаются из самого адреса при валидации.
- Написать **ADR 0007: Domain lease parameter governance**: параметры аренды `domain_lo > 0` (min rent, grace period, auction duration, renewal window, max annual adjustment), протокол корректировки validator voting в пределах, заданных протоколом, запрет применения к активным арендам, No Burn Principle.
- Зафиксировать в `docs/plans/mvp_v5.md` (этот файл) scope freeze и out-of-scope backlog.

**Acceptance criteria:**

- RFC 0012 v2 содержит полную нормативную замену модели: `marks_last_block`, blocks_per_hour timing, saturation ceiling, touch-semantics, явное удаление anchor_ref/free-claim-day/ClaimTx из scope.
- RFC 0011 addendum явно помечает `ClaimTx` как deprecated/removed в V5 и перечисляет удаляемые error codes.
- RFC 0013 addendum явно выводит из active scope ClaimTx policy matrix и anchor predicates.
- Экономический RFC по float inflation содержит формулу `base_emission * season_coeff_ppm / 1_000_000` и правило fallback при нуле.
- ADR 0005 переведён из Draft → Accepted с устранением всех `TODO/TBD` блокирующих кодирование.
- ADR 0006: описывает flag bits в адресной модели, явно помечает runtime enforcement как V6.
- ADR 0007: описывает governance domain lease params.
- Ни один кодовый слайс не стартует с неразрешённой wire-амбигуитетой по V5 fields.

**Файлы/модули (ориентир):**

- `docs/rfc/12-claim-maturity-and-state-model.md` (**major revision v2** — lazy model)
- `docs/rfc/11-burn-purpose-and-claim-tx.md` (addendum: ClaimTx deprecated)
- `docs/rfc/13-claim-policy-matrix.md` (addendum: ClaimTx policy matrix retired)
- `docs/rfc/14-claim-burn-api-error-contract.md` (addendum: retired ClaimTx wire surface)
- `docs/rfc/19-float-inflation.md` (новый RFC для float inflation)
- `docs/adr/0005-policy-deferred-activation.md`
- `docs/adr/0006-address-flags-and-nondisableable-profiles.md` (новый)
- `docs/adr/0007-domain-lease-parameter-governance.md` (новый)
- `docs/plans/mvp_v5.md`
- `docs/CONCEPT_ROADMAP.md` (minor cross-refs)

**Demo-ready output:** команда видит полный V5 spec contract до первого кода, включая нормативное описание lazy marks модели в RFC 0012 v2.

**Gate (2026-05-23):** закрыт после review-fixes и rereview PASS — [20260523-v5-sprint1-spec-adr-freeze-rereview.md](../reviews/20260523-v5-sprint1-spec-adr-freeze-rereview.md). Следующий спринт: V5-2 core model.

### Декомпозиция V5-2 на coding-слайсы

| # | Ticket | Scope |
|---|--------|--------|
| 1 | `tasks/20260524-v5-s2-slice1-gencfg.json` | GenCfg + `ClaimPhaseConfig` extension, defaults, serde tests |
| 2 | `tasks/20260524-v5-s2-slice2-account.json` | Account V5 fields; remove legacy claim fields; mechanical compile fixes |
| 3 | `tasks/20260524-v5-s2-slice3-drop-claim.json` | Remove `TxBody::Claim` / `ClaimMode` / legacy claim errors |
| 4 | `tasks/20260524-v5-s2-slice4-ipv4-batch.json` | `ClaimIPv4Batch` tx shape + `validate_tx_shape` stub |
| 5 | `tasks/20260524-v5-s2-slice5-snapshot-v3.json` | Snapshot schema v3 + v2 migration + replay test |

Umbrella: `tasks/done/20260524-v5-sprint2-core-model.json`. Порядок строгий: 1→5. Lazy marks / inflation runtime — **V5-3**, не смешивать.

**Gate (2026-05-24):** закрыт после slices 1–5, review-fixes (`87af492`), rereview и integrated testing PASS — [20260524-v5-s2-review-fixes-rereview.md](../reviews/20260524-v5-s2-review-fixes-rereview.md). Следующий спринт: **V5-3** lazy marks engine + float inflation.

---

## Sprint V5-2: Core state model и serialization

**Цель:** добавить V5 data structures без изменения поведения.

**Scope:**

- **Account — добавить:** `marks_last_block: u64`, `deferred_policies: Vec<DeferredPolicyEntry>` (policy kind + activate_at_height), `ipv4_claimed_phase: Option<u8>`. Поле `address_flags` **не добавляется** — флаги уже являются частью адреса (bech32DX).
- **Account — удалить устаревшие поля RFC 0012 v1:** `last_claim_unix_time: u64`, `last_claim_anchor_ref: u64`, `free_claim_utc_day: Option<u64>`. Эти поля заменяются одним `marks_last_block`. Миграция: при чтении старого snapshot конвертировать `last_claim_unix_time` → `marks_last_block` через `blocks_per_hour` approximation (или обнулить, см. миграционный контракт RFC 0012 v2).
- **TxBody::Claim — удалить:** lazy model делает `ClaimTx` (tx_type: "claim_mark") избыточным. Вариант убирается из enum вместе с `ClaimMode`, `ClaimMode::Free/Paid`, `CLAIM_ALL` sentinel и associated error codes (`ClaimAnchorRangeInvalid`, `ClaimAnchorContinuityBroken`, `ClaimOverMatured`, `FreeClaimDailyLimit`, `ClaimFeeModeConflict`, `ClaimDeltaInvalid`). Обратная совместимость: unknown tx_type в десериализации → structured error, не panic.
- **GenCfg — добавить:** `blocks_per_hour: u64` (default 3600), `marks_per_coin_per_hour: u32` (default 1), `base_emission_per_block: u64`, `season_coeff_ppm: u32`, `ipv4_claim_phases: Vec<ClaimPhaseConfig>`.
- Добавить новый вариант `TxBody::ClaimIPv4Batch { phase: u8, batch_root: [u8;32], registry_sig: Signature }` (не путать с удалённым `TxBody::Claim`; это IPv4-allocation, не marks materialization).
- Обновить snapshot schema → v3: миграционный gate, backward reject для неизвестных version; тест: replay старого v2 snapshot с explicit field conversion.
- Каноническая сериализация / десериализация всех новых полей; deterministic включение в state root.
- Тесты на serde round-trip: Account v2→v3 migration (включая удалённые поля), GenCfg extension, ClaimIPv4Batch encoding.

**Acceptance criteria:**

- Старые V4 devnet fixtures загружаются или отвергаются с явным version-gate — без молчаливого corrupted state.
- `cargo check --workspace` чистый после изменений.
- `cargo test -p pwm-core --lib` зелёный (включая serde/snapshot тесты).
- Snapshot replay determinism test проходит на genesis с V5 schema.
- `TxBody::Claim` вариант отсутствует в enum; десериализация unknown `tx_type = "claim_mark"` возвращает structured error.
- `ClaimIPv4Batch` не проходит `validate_tx_shape` без registry_sig (заглушка, full evaluation — V5-5).
- `Account` не содержит `last_claim_unix_time`, `last_claim_anchor_ref`, `free_claim_utc_day`.

**Файлы/модули (ориентир):**

- `crates/pwm-core/src/types.rs` (Account поля: добавить/удалить, ClaimPhaseConfig)
- `crates/pwm-core/src/tx.rs` (TxBody: удалить Claim, добавить ClaimIPv4Batch; ClaimMode удалить)
- `crates/pwm-core/src/reject_wire.rs` (удалить legacy Claim error codes)
- `crates/pwm-core/src/genesis.rs` (GenCfg extension)
- `crates/pwm-core/src/snapshot.rs` / `schema.rs` (v3 migration)
- `crates/pwm-core/src/state.rs` (validate_tx_shape для ClaimIPv4Batch)

---

## Sprint V5-3: Lazy marks engine + float inflation

**Gate (2026-05-24):** закрыт после slices 1–3 (coding → review → testing PASS) — umbrella [tasks/done/20260524-v5-sprint3-lazy-marks-inflation.json](../../tasks/done/20260524-v5-sprint3-lazy-marks-inflation.json); reviews [20260524-v5-s3-slice3-chain-seal-review.md](../reviews/20260524-v5-s3-slice3-chain-seal-review.md). Следующий спринт: **V5-4** deferred activation.


- Реализовать `compute_lazy_marks(account, current_height, gen_cfg) -> u32` согласно RFC 0012 v2:
  ```
  whole_pwm_staked = floor(account.staked_pwm_raw / 1_000_000)  // RFC 0012 единицы
  delta_hours      = floor((current_height - marks_last_block) / gen_cfg.blocks_per_hour)
  rate             = gen_cfg.marks_per_coin_per_hour             // default = 1
  satur_hours      = ceil((u32::MAX - stored_marks) / (whole_pwm_staked * rate))
  effective_h      = min(delta_hours, satur_hours)
  generated        = (whole_pwm_staked * rate * effective_h) as u32
  effective_marks  = min(u32::MAX, stored_marks + generated)
  ```
  При `whole_pwm_staked == 0` → `generated = 0`. Чистая функция, без IO. Семантика "1 whole PWM × 1 hour = 1 mark" сохраняется из RFC 0012 v1.
- Интегрировать touch-semantics при каждом state-touch аккаунта (RFC 0012 v2 расширяет авто-клейм из RFC 0012 v1): Transfer (sender + recipient), Stake/Unstake (owner), BurnMark (owner), PolicyTx (account), INIT (new account). Touch = вычислить delta → применить к `stored_marks` → обновить `marks_last_block = current_height`. Touch-semantics — единственное место мутации `marks_last_block`.
- Реализовать `compute_block_reward(gen_cfg, block_height) -> u64` используя `base_emission_per_block * season_coeff_ppm / 1_000_000`. Если `season_coeff_ppm == 0` в GenCfg — fallback к фиксированному `block_reward` (обратная совместимость с существующими devnet конфигами).
- Применить `compute_block_reward` в `Chain::seal` вместо (или в дополнение к) существующего фиксированного `block_reward`.
- Тесты: marks accumulation до насыщения, saturation clamp, multiple touches, inflation math at various heights/seasons, backward-compat fallback.

**Acceptance criteria:**

- `compute_lazy_marks` — чистая функция, не мутирует state напрямую, без IO.
- После N блоков без touch: `effective_hours = (N / blocks_per_hour)`, `generated = staked_coins * marks_rate * effective_hours`, clamp через `satur_hours`.
- После touch: `stored_marks` обновляется, `marks_last_block = current_height`.
- Saturation: при `stored_marks == u32::MAX` дальнейшее начисление не изменяет баланс.
- `compute_block_reward` не паникует при любых параметрах GenCfg (saturating arithmetic).
- `cargo test -p pwm-core marks_` и `cargo test -p pwm-core inflation_` зелёные.
- `cargo test -p pwm-core --lib` общий зелёный.

**Файлы/модули (ориентир):**

- `crates/pwm-core/src/marks.rs` (новый) или `crates/pwm-core/src/economics.rs`
- `crates/pwm-core/src/state.rs` (touch-integration)
- `crates/pwm-core/src/chain.rs` (dynamic block_reward в seal)

---

## Sprint V5-4: Deferred activation реализация

**Gate (2026-05-24):** закрыт после slices 1–3 (coding → review → testing PASS) — umbrella [tasks/done/20260524-v5-sprint4-deferred-activation.json](../../tasks/done/20260524-v5-sprint4-deferred-activation.json); reviews [20260524-v5-s4-slice3-spec-tests-review.md](../reviews/20260524-v5-s4-slice3-spec-tests-review.md). Следующий спринт: **V5-5** IPv4 Claim on-chain.

**Цель:** реализовать `ActivationMode::Deferred` (ADR 0005 → normative code).

**Scope:**

- Обновить `ActivationMode` enum: `Dormant | Immediately | Deferred { activate_at_height: u64 }`.
- Обновить `SetPolicy` / `PolicyAction::SetPolicy`: добавить опциональный `activate_at_height` при `activation == Deferred`.
- Обновить `evaluate_policy`: для политики в `Deferred`-режиме проверять `chain_tip_height >= activate_at_height`; если нет — политика не активна (эквивалент Dormant до достижения высоты).
- Применить auto-activate семантику: по достижении `activate_at_height` политика переходит в active без дополнительного `ActivatePolicy`.
- Reject: `ActivatePolicy` для уже `Deferred`-установленной политики до достижения высоты → возвращать stable policy error code (`E_POLICY_DEFERRED_NOT_YET_ACTIVE` или аналог).
- `DeactivatePolicy` для `Deferred`-политики до наступления высоты — допускается, снимает запись.
- Обновить snapshot schema для `deferred_policies` поля Account (V5-2 заложил поле, V5-4 заполняет логику).
- Normative update RFC 6/7: вставить `Deferred` как третий variant, правила auto-activate и конфликта.
- Тесты: установка Deferred → не активна до высоты → автоактивация на высоте → ActivatePolicy-конфликт → DeactivatePolicy до высоты.

**Acceptance criteria:**

- `evaluate_policy` детерминирован: одинаковый tx + pre-state + height → одинаковое решение.
- Деактивация до высоты не оставляет «зависшей» записи в Account.
- Тест: `policy_deferred_auto_activates_at_height` — PASS.
- Тест: `policy_deferred_activate_before_height_rejected` — PASS.
- `cargo test -p pwm-core policy_` (все policy тесты) зелёные.
- `cargo check -p pwm-core && cargo check -p pwmd && cargo check -p pwm-cli` чистые.

**Файлы/модули (ориентир):**

- `crates/pwm-core/src/policy.rs` (evaluator)
- `crates/pwm-core/src/types.rs` (Account deferred_policies logic)
- `crates/pwm-core/src/tx.rs` (ActivationMode enum)
- `docs/rfc/6-policy-engine.md`, `docs/rfc/7-tx-and-state-model.md` (normative update)

---

## Sprint V5-5: IPv4 Claim on-chain

**Gate (2026-05-24):** закрыт после slices 1–2 (coding → review → testing PASS) — umbrella [tasks/done/20260524-v5-sprint5-ipv4-claim-onchain.json](../../tasks/done/20260524-v5-sprint5-ipv4-claim-onchain.json); reviews [20260524-v5-s5-slice2-reject-fixture-review.md](../reviews/20260524-v5-s5-slice2-reject-fixture-review.md). Следующий спринт: **V5-6** TUI marks saturation.

**Цель:** реализовать on-chain primitive для IPv4 batch-allocation — минимальный, изолированный от TUI/CLI слайс в `pwm-core`.

**Scope:**

- Реализовать полную проверку и применение `TxBody::ClaimIPv4Batch` в `state::apply_tx`:
  - Проверить, что `phase` соответствует записи в `GenCfg.ipv4_claim_phases`.
  - Проверить `registry_sig` относительно `registry_address` из соответствующей `ClaimPhaseConfig`.
  - Проверить anti-double-claim: `Account.ipv4_claimed_phase.is_none()` для данной фазы.
  - Apply: кредитовать `destination_address` на `allocation` PWM; установить `ipv4_claimed_phase = Some(phase)`.
  - Rejects со stable error codes: неизвестная фаза, фаза уже claimed, invalid registry_sig, destination not initialized (нет INIT).
- Обновить `validate_tx_shape` (заглушка из V5-2) до полной structure-level проверки.
- Тесты: happy path (баланс вырос, claimed phase установлена), double-claim reject, invalid registry_sig reject, unknown phase reject, destination-not-init reject.

**Acceptance criteria:**

- `ClaimIPv4Batch` применяется к state: destination balance увеличивается на `allocation`.
- Double-claim в рамках одной фазы отвергается с explicit error.
- `cargo test -p pwm-core claim_` — все зелёные.
- `cargo check --workspace` чистый.

**Файлы/модули (ориентир):**

- `crates/pwm-core/src/state.rs` (ClaimIPv4Batch apply + validate path)
- `crates/pwm-core/src/tx.rs` (validate_tx_shape extension)

---

## Sprint V5-6: TUI marks saturation

**Gate (2026-05-24):** закрыт после slices 1–2 (coding → review → testing PASS) — umbrella [tasks/done/20260524-v5-sprint6-tui-marks-saturation.json](../../tasks/done/20260524-v5-sprint6-tui-marks-saturation.json); reviews [20260524-v5-s6-slice2-ui-saturation-column-review.md](../reviews/20260524-v5-s6-slice2-ui-saturation-column-review.md). Следующий спринт: **V5-7** CLI + genesis doc.

**Цель:** сделать marks saturation наглядным в TUI — первый пользовательский срез marks engine.

**Scope:**

- Добавить в таблицу аккаунтов TUI колонку (или отдельный статус-бар) с marks saturation:
  - Отображать `effective_marks` (вычисленный на текущий head height через `compute_lazy_marks`), а не только `stored_marks`.
  - Показывать степень насыщения: `{effective} / {cap}` или процент.
  - При `staked_pwm == 0` — показывать `0 / cap (0%)` без паники.
  - При `stored_marks == u32::MAX` — явно маркировать как «насыщен».
- Добавить poll-side вычисление `effective_marks`: при получении ответа `/v1/account/:id` клиент TUI вычисляет effective marks локально (head height из `/v1/head`), не ждёт мутации state на ноде.
- Минимальное кэширование head height в TUI widget (не пересчитывать на каждый render frame, только при обновлении poll).

**Acceptance criteria:**

- TUI запускается без паники при нулевых marks/staked балансах.
- Колонка/бар отображает разумное значение effective marks при запущенном devnet.
- `cargo check -p pwm-tui` чистый.
- Ручной smoke: TUI на demo devnet показывает growing marks при активном стейке.

**Файлы/модули (ориентир):**

- `crates/pwm-tui/src/` (marks saturation widget)
- `crates/pwm-tui/src/poll.rs` или аналог (head height fetch + effective_marks calc)

---

## Sprint V5-7: CLI enhancements + 21B genesis design doc

**Gate (2026-05-24):** закрыт после slices 1–3 (coding → review → testing PASS; slice2 testing rerun after `--lib` gate fix) — umbrella [tasks/done/20260524-v5-sprint7-cli-genesis-doc.json](../../tasks/done/20260524-v5-sprint7-cli-genesis-doc.json); reviews [20260524-v5-s7-slice2-tx-policy-deferred-review.md](../reviews/20260524-v5-s7-slice2-tx-policy-deferred-review.md). Следующий спринт: **V5-8** operator smoke + closeout.

**Цель:** операторский срез — CLI с marks detail и deferred activation, плюс зафиксировать 21B genesis design.

**Scope:**

- **CLI account inspect:** расширить `pwm account-info` (или аналог) выводом `marks: {effective} / {cap} ({pct}%)`, `marks_last_block`, текущего `staked_pwm`. Использовать те же `compute_lazy_marks` + head height fetch, что и TUI в V5-6.
- **CLI tx-policy-set deferred:** добавить флаги `--activation deferred --activate-at-height <N>` к команде `tx-policy-set`. V5-4 даёт backend; V5-7 даёт CLI front. Валидация: `--activate-at-height` обязателен при `--activation deferred`; иначе — user-friendly error.
- **21B genesis design doc:** написать `docs/genesis-21b-design.md`:
  - Allocation table structure: verifier premine, IPv4 claim phases pool (~20B), team/ops reserve, public devnet faucet.
  - IPv4-weighted formula (sqrt-weighted или tier-based: /8 → full tier, /16 → √(256) tier, /24 → 1 unit).
  - Phasing: 5 итераций по ~4B PWM, интервал ~1–2 года.
  - Placeholder section для production genesis после claim registry onboarding.
  - Cross-ref с ADR 0002 (IPv4 claiming design).

**Acceptance criteria:**

- `pwm account-info` выводит effective marks и saturation без паники при нулевых балансах.
- `pwm tx-policy-set --activation deferred --activate-at-height 500` принимается и создаёт корректный tx.
- `pwm tx-policy-set --activation deferred` (без `--activate-at-height`) возвращает понятную ошибку.
- `cargo check -p pwm-cli` чистый.
- `docs/genesis-21b-design.md` содержит allocation table structure и formula section.

**Файлы/модули (ориентир):**

- `crates/pwm-cli/src/` (account-info расширение, tx-policy-set deferred flags)
- `docs/genesis-21b-design.md` (новый)

---

## Sprint V5-9 (pre-closeout): CY cluster multi-hour E2E

**Цель:** перед финальным V5 closeout прогнать **живой** RFC16 кластер **CY** (лаунчеры в корне репо: `cy-cluster-proposer.ps1`, `cy-cluster-attester.ps1`) несколько часов и закрыть регрессии, не видимые в single-node `devnet_v5_operator_smoke`.

**Операторский runbook:** [runbooks/v5-cy-cluster-precloseout-soak.md](runbooks/v5-cy-cluster-precloseout-soak.md)

**Слайсы (тикеты):**

| Slice | Ticket | Фокус |
|-------|--------|--------|
| s1 | `20260529-v5-cy-e2e-s1-cluster-bootstrap-stability` | Старт кластера, кворум, sync, стабилизация — **PASS (live)** |
| s2 | `20260531-v5-cy-e2e-s2-marks-saturation-soak-rerun` | 2–6 ч насыщение lazy marks — **PASS (live, PARTIAL: 2 staked)** |
| s3 | `20260529-v5-cy-e2e-s3-mass-burn-batches` | Массовые `BurnMark` — **PASS (live, 1e9 marks, 5 tx)** |

**Umbrella:** `tasks/done/20260529-v5-precloseout-cy-e2e-umbrella.json` — **done** (owner sign-off 2026-05-30).

**Reports:** `tmp/cy-e2e-s1-20260528_220256.md`, `tmp/cy-e2e-s2-20260530_082418.md`, `tmp/cy-e2e-s3-20260530_141317.md`

**Gate:** s1–s3 PASS ✅; doc review PASS_WITH_NITS ([review](../reviews/20260530-v5-precloseout-cy-e2e-docs-version-review.md)); sprint-final closeout PASS ([review](../reviews/20260530-v5-sprint-final-closeout-review.md)) — owner sign-off pending.

---

## Sprint V5-8: Integrated devnet gate и closeout

**Цель:** закрыть V5 как coherent tokenomics + IPv4 foundation release.

**Scope:**

- End-to-end smoke на demo devnet: lazy marks accumulate over N blocks, saturation display in TUI, deferred policy activates at block N, ClaimIPv4Batch accepted by node (с test registry key из demo genesis).
- Обновить `docs/MVP-checklist.md`: добавить V5 traceability block.
- Обновить `docs/CONCEPT_ROADMAP.md`: пометить V5 критерии готовности, расставить `[x]`.
- Обновить `docs/GLOSSARY.md` (финальное ревью спринта — sprint-final glossary check).
- Обновить `CHANGELOG.md` после принятых gate.
- Final `pwm-review` с явной пометкой «финальное ревью спринта V5».
- Backlog separation: зафиксировать в плане, что уходит в V6 (address flags runtime, conservation delayed Transfer, domain lease auction, PoS admission).

**Acceptance criteria:**

- `cargo fmt --check`, `cargo check --workspace`, `cargo test -p pwm-core --lib`, `cargo test -p pwmd --lib`, `cargo check -p pwm-cli`, `cargo check -p pwm-tui` — все зелёные.
- TUI демонстрирует marks saturation на запущенном demo devnet.
- Все V5 критерии готовности из CONCEPT_ROADMAP.md покрыты или явно deferred с owner-approved rationale.
- Glossary обновлён: lazy accumulation (staked-only, hours model), marks saturation, deferred activation, ClaimIPv4Batch.

---

## Межспринтовые гейты качества

- **Simplicity Gate:** ни один V5 слайс не вводит нового async channel, background loop или VM; все вычисления — pure arithmetic в `evaluate_policy` и `state::apply_tx`.
- **Purity Gate:** `compute_lazy_marks` и `compute_block_reward` — чистые функции без IO и state mutation.
- **Additivity Gate:** все V5 wire-fields — additive; нет breaking changes для существующих V4 tx types и Account fields.
- **Schema Gate:** snapshot v3 включает V5 fields детерминированно; replay v2 snapshot → v3 работает или явно rejected version gate.
- **ADR Gate (spec-only):** ADR 0006 и 0007 — только нормативная формализация; любая попытка enforcement семантики флагов адреса в validate/apply path в V5 — стоп, переносить в V6 ticket.

## Риски и контрмеры

- **Миграция legacy Claim полей (snapshot v2 → v3):** `last_claim_unix_time` нужно конвертировать в `marks_last_block`. Exact conversion невозможна (unix time ≠ block height без маппинга); безопасная стратегия — обнулить `marks_last_block` при миграции (аккаунт начнёт накапливать с нуля). Нормативный контракт миграции зафиксировать в RFC 0012 v2.
- **marks_last_block staleness:** при пропуске touch аккаунта snapshot может накопить большой delta. Митигация: saturating_add + u32::MAX clamp в любой точке вычисления.
- **season_coeff_ppm = 0 devnet regression:** при `season_coeff_ppm = 0` в старых dev genesis configs block_reward рассчитается в 0. Митигация: fallback к фиксированному значению при `season_coeff_ppm = 0`.
- **ClaimIPv4Batch registry_sig spoofing:** без production registry ключа вся chain может быть заспамлена fake claims. Митигация: в demo genesis использовать deterministic test registry key; в production genesis — отдельный onboarding процесс.
- **Deferred activation height overflow:** `activate_at_height: u64` — без overflow при arithmetic; использовать saturating/checked operations.
- **TUI marks computation performance:** `effective_marks` вычисляется на каждый render-tick с текущим head height. Митигация: кэшировать последний known height на стороне TUI widget.
- **ADR 0006 scope creep в V5:** риск, что conservation flag просочится в mempool/seal как «маленькое дополнение». Митигация: явный acceptance criterion — в V5 `validate_tx_shape` и `apply_tx` **не читают биты флагов адреса**; pwm-review должен флагировать любое обращение к адресным битам в apply/validate path как выход за рамки V5.

## Декомпозиция на таски

- Sprint V5-1: `tasks/20260523-v5-sprint1-spec-adr-freeze.json`
- Sprint V5-2: `tasks/done/20260524-v5-sprint2-core-model.json` (slices `20260524-v5-s2-slice1` … `slice5`)
- Sprint V5-3: `tasks/done/20260524-v5-sprint3-lazy-marks-inflation.json` (slices 1–3)
- Sprint V5-4: `tasks/20260523-v5-sprint4-deferred-activation.json`
- Sprint V5-5: `tasks/20260523-v5-sprint5-ipv4-claim-onchain.json`
- Sprint V5-6: `tasks/20260523-v5-sprint6-tui-marks-saturation.json`
- Sprint V5-7: `tasks/20260523-v5-sprint7-cli-genesis-doc.json`
- Sprint V5-8: `tasks/20260523-v5-sprint8-closeout.json`

---

## Итоговое состояние кода и документов после V5 closeout

- `Account` содержит новые поля: `marks_last_block: u64`, `deferred_policies: Vec<DeferredPolicyEntry>`, `ipv4_claimed_phase: Option<u8>`. Поле `address_flags` в Account **не добавляется** — флаги уже являются частью самого адреса (брутфорсятся при генерации с V1).
- `GenCfg` содержит `blocks_per_hour`, `marks_per_coin_per_hour`, `ipv4_claim_phases: Vec<ClaimPhaseConfig>` и использует `season_coeff_ppm` в динамической формуле block_reward.
- `TxBody` включает `ClaimIPv4Batch { phase, batch_root, registry_sig }` вариант с полной validation/apply в state.
- `ActivationMode` включает `Deferred { activate_at_height }` вариант; evaluator auto-activates по высоте.
- Snapshot schema v3 с миграционным gate (backward reject для неизвестных version).
- TUI отображает effective marks (staked-only, часовая модель) и степень насыщения.
- CLI поддерживает `--activation deferred --activate-at-height` и `account-info` с marks detail.
- ADR 0005 — Accepted; ADR 0006, ADR 0007 — Draft/Accepted (spec only, runtime enforcement в V6).
- RFC 0012 v2 — Active (lazy marks model, marks_last_block, saturation ceiling, touch-semantics); RFC 0011 addendum — ClaimTx deprecated; RFC 0013 addendum — ClaimTx policy matrix retired.
- `docs/genesis-21b-design.md` — зафиксирован allocation table структура для IPv4-weighted 21B genesis.

---

_Конец плана MVP v5._
