# CONCEPT_PROGRESS.md — Прогресс MVP v2 + V5 к реализации DRAFT_WHITEPAPER

**Дата:** 2026-06-02  
**Цель:** Оценить насколько текущая реализация (база MVP v2 + прогресс MVP v5 токеномики) покрывает концепт из `DRAFT_WHITEPAPER.md`.  
**Публикация:** этот файл — **внешняя** карта покрытия; внутренний роадмап версий/ADR — [`CONCEPT_ROADMAP.md`](CONCEPT_ROADMAP.md) (не публикуется, не индексируется в README).  
**Уровни готовности:** ✅ Полностью | 🔶 Частично / MVP-форма | 🔄 В работе | ⏳ Запланировано (план/roadmap) | ❌ Не начато / Defer

---

## 1. Общая картина

| Раздел Whitepaper | Состояние | Комментарии |
|---|---|---|
| §1 Introduction — концепция и цели | ✅ | Концепция зафиксирована, документация согласована |
| §2 Purpose — анти-спам, псевдоавторизация | 🔶 | `BURN_MARK` реализован (CLI + TUI F5), но интеграция с внешними системами (email, messengers) — defer |
| §3 Economic model — эмиссия, инфляция, марки | 🔶 | V5: lazy marks (staked-only, `u32::MAX` cap), float `block_reward` в seal, `ClaimIPv4Batch` on-chain; эмиссия PWM V2-3; полная IPv4 registry/off-chain фазность и 21B genesis amounts — defer |
| §4 Адресация, кластеры, HD-derivation, honeypots | 🔶 | HD-derivation (SLIP-0010), domain_code, bech32DX — реализованы; honeypots — не начато |
| §4.4 PQC (quantum-safe signatures) | ❌ | Ed25519; Dilithium/SPHINCS+ — явно defer |
| §5 Dumb Contracts (политики, multisig, CLTV) | 🔶 | Базовые policy hooks (recipient domain, TTL, sender filter, routing, default) — RFC/спеки; runtime — не реализован |
| §6 Arbitrator — частичная централизация | ❌ | Не начато; концепт зарезервирован |
| §7 Anti-spam и марки на практике | 🔶 | Сжигание марок end-to-end работает (CLI/TUI); X-PWM header для email, AI API — defer |
| §8 Offchain burning + L3 | 🔶 | Offchain stub (`docs/OFFCHAIN_STUB.md`, `offchain-batch`); batch Merkle demo — без продакшн интеграции |
| §9 AI integration | ❌ | Концепт описан, код не начат |
| §10 Business model | ✅ | Описано в WP; не требует кода |
| §11 Roadmap (Phases 1–4) | 🔄 | V2–V5 спринтовые gates закрыты (см. §3b); Phase 3–4 (V6+ PoS, offchain prod) — будущее |
| §12 Long-term (media trust, deepfake) | ❌ | Видение, не требует текущей реализации |

---

## 2. Детальная карта по компонентам

### 2.1 Ядро и консенсус

| Компонент | WP § | Реализация | Статус |
|---|---|---|---|
| Account-based state (balance/staked/marks) | §3–4 | `pwm-core/src/state.rs` — `Account` struct | ✅ |
| PoA devnet consensus (round-robin) | §6 (v0) | `pwm-core/src/chain.rs` — `Chain::boot`, `seal` | ✅ |
| Cluster attestation (proposer + attester, 2-of-2 / 2-of-3) | §6 (переход) | RFC 16; спринт V2-9 **закрыт** (`tasks/20260513-v2-sprint9-rfc16-cluster-attestation.json`): wire E2E, 2-of-2/2-of-3 happy, негативы no-quorum + binding mismatch, degradation 2-of-3, follower TCP convergence; op backlog: расширенный §11 fault-matrix (partition-lite) по желанию | ✅ (MVP gate) |
| Same-shard sync v1 (mempool gossip, header-first block sync) | — (инфраструктура) | V2-8 — slices 0–5 готовы; **legacy** slice 6 wave-pack (multi-sealer) **blocked** — цели перенесены на V2-9, не отдельный green gate | ✅ (база + V2-9 приёмка) |
| Geo-sharding (domain_hi routing) | §4 | `domain_code` u16, range classification; EXPORT/IMPORT flow | ✅ |
| Cross-shard EXPORT/IMPORT + replay protection | §4 (sharding) | Sprint 13–15; bridge trust refusal, one-window closure | ✅ |
| Seasonal emission coefficient (лето/зима) | §3 | V2-3: `season_coeff_ppm`, `GenCfg` policy v2 | ✅ |
| Stake threshold для эмиссии (~100k PWM) | §3 | V2-3: `pwm_stake_min` в `GenCfg` | ✅ (MVP) |
| Marks accrual formula (1 PWM × 1 hour = 1 mark) | §3 | V5-3: lazy `compute_lazy_marks` (staked-only, `blocks_per_hour`); V2-5 legacy superseded | ✅ |
| Marks storage + lazy cursor | §3 | `stored_marks: u32`, `marks_last_block: u64`, `effective_marks` at poll/touch | ✅ |
| Float inflation in seal | §3 | V5-3: `compute_block_reward` + `season_coeff_ppm` в `Chain::seal` | ✅ |
| Deferred policy activation | §5 | V5-4: `ActivationMode::Deferred { activate_at_height }`, evaluator auto-activate | ✅ |
| IPv4 claim on-chain (`ClaimIPv4Batch`) | §3 | V5-5: registry-sig gated apply; legacy `ClaimTx` retired | ✅ |
| Snapshot genesis anchor (light) | — | ADR 0008 **Implemented**; `genesis_anchor` в snapshot v3 wire | ✅ |
| Block reward to producer | §3 | V5 float reward + `RewPol::ToProducerAccount` | ✅ |
| Genesis funding + validator set | §3 | `genesis.rs`: `GenCfg`, `GRow`, `dev_net()` | ✅ |
| Persistence (JsonFile + ClickHouse optional) | — | Epoch JSONL, manifest, autosnapshot | ✅ |

### 2.2 Адресная модель и криптография

| Компонент | WP § | Реализация | Статус |
|---|---|---|---|
| HD derivation (SLIP-0010, `m/0'/i`) | §4.2 | `pwm-core/src/hd.rs` | ✅ |
| Domain-code brute-match | §4.2 | `addr-bruteforce` CLI, `account_id_from_parts` | ✅ |
| bech32DX адресный формат | §4.1 | Phase 1: pretty `pwm1-…`, canonical bech32DX | ✅ |
| INIT transaction (activation) | §5.1 | `INIT { index, flags }`, account activation | ✅ |
| Ed25519 sign/verify | §4.4 (временно) | `pwm-core/src/crypto.rs` | ✅ |
| Quantum-safe (Dilithium/SPHINCS+) | §4.4 | Не реализовано | ❌ |
| Honeypot addresses | §4.3 | Не реализовано | ❌ |
| Address metadata (TTL, filter, routing policies) | §5 | WHITE_SPEC §9 RFC pack; runtime not implemented | ⏳ (RFC) |

### 2.3 Транзакции и контракты

| Компонент | WP § | Реализация | Статус |
|---|---|---|---|
| TRANSFER | §3 | `TRANSFER { to, amount, fee }` | ✅ |
| STAKE / UNSTAKE | §3 | CLI + TUI (F7 / Shift+F7) | ✅ |
| BURN_MARK | §2, §7 | CLI `tx-burn-mark --amount --purpose` + TUI F5 | ✅ |
| ClaimTx (explicit + auto-claim) | §3 | **V5: retired**; марки — lazy touch; IPv4 — `ClaimIPv4Batch` | 🔶 |
| EXPORT / IMPORT (cross-shard) | §4 | Sprint 13+; provenance, replay guard | ✅ |
| Dumb contracts — policy engine | §5 | RFC hooks (routing, TTL, sender filter, default); runtime — нет | ⏳ |
| Dumb contracts — CLTV | §5 | Не реализовано | ❌ |
| Dumb contracts — multisig/cosign | §5 | Не реализовано | ❌ |
| INIT tx (корпоративная подпись) | §5.1 | Базовый INIT (single key); cosign — нет | 🔶 |
| IMPORT min fee (0.01 PWM) | §7.3 | V2: target shard fee_pool | ✅ |

### 2.4 Клиенты (CLI / TUI / API)

| Компонент | WP § | Реализация | Статус |
|---|---|---|---|
| REST API (pwmd) | — | `/v1/head`, `/v1/account`, `/v1/tx`, `/v1/status`, `/v1/peers`, `/v1/cross-shard/facts` | ✅ |
| CLI: key-gen, addr-derive, tx-init/send/stake/unstake/burn/claim/export/import | §3 | `pwm-cli` — все базовые команды | ✅ |
| CLI: wallet-first path (`--wallet`, encrypted) | §3.5 | Phase 1: encrypted wallet, backup/recover | ✅ |
| TUI: account table (PWM / Staked / Marks / Init) | — | Таблица с панелями Owner/Receivers | ✅ |
| TUI: F5 Burn form (effective marks, saturation) | §7 | V5-6 + pre-publish polish: `effective_marks` в detail/F5, saturation column, без Claim UX; runbook [v5-tui-marks-operator-path.md](runbooks/v5-tui-marks-operator-path.md) | ✅ |
| TUI: F6 Send form | — | Phase 1: validate, submit, status | ✅ |
| TUI: F7 Stake / Shift+F7 Unstake forms | §3 | V2-6: stake/unstake forms | ✅ |
| TUI: wallet lock/encrypt controls (F3/F4) | — | Phase 1 | ✅ |
| TUI: history modal (H) | — | Phase 1: pending/ok/error | ✅ |
| TUI: Debug panel | — | `PWM_TUI_DEBUG=1` | ✅ |
| JSON reject wire (structured errors) | — | V2-1: `phase`, `tx_kind`, `response_class`, `error.code` | ✅ |
| CLI/TUI: URI support (`pwm:<address>?amount=`) | — | Phase 1 | ✅ |

### 2.5 Сетевой слой и Peering

| Компонент | WP § | Реализация | Статус |
|---|---|---|---|
| Seed-list PoA devnet topology | — | Explicit seeds, `--transport-peer-seed` | ✅ |
| Real transport TCP peering | — | `--transport-real`, wire protocol | ✅ |
| Capability negotiation | — | Feature gates, version negotiation | ✅ |
| Same-shard mempool gossip | — | V2-8 Slice 2: best-effort dedup/rate-limit | ✅ |
| Header-first block sync | — | V2-8 Slice 3: live sync + catch-up | ✅ |
| Epoch catch-up fallback | — | V2-8 Slice 4: chunk transfer | ✅ |
| Bridge federation (level-2 trust) | §7.5 | Commitment digest, trust refusal, one-window closure | ✅ |
| Federated relay (one-window cross-shard) | — | Sprint 15: relay HTTP, gossip-style | ✅ |

### 2.6 Экономика и токеномика (WP §3)

| Компонент | WP Spec | Реализация | Статус |
|---|---|---|---|
| Base issue: 21B coins | §3 | Genesis funding — фиксированное, не 21B | 🔶 (devnet) |
| IPv4 capping distribution | §3 | V5-5: on-chain `ClaimIPv4Batch` + phase gate; полный registry/off-chain bootstrap — defer | 🔶 |
| Floating annual inflation (~5%) | §3 | V5-3: динамический `block_reward` в seal (~5% target + `season_coeff_ppm`) | ✅ |
| Seasonal variation (лето/зима) | §3 | V2-3 + V5 seal: `season_coeff_ppm` | ✅ |
| Marks generation from staking | §3 | V5 lazy engine: staked-only, cap `u32::MAX`, cursor `marks_last_block: u64` | ✅ |
| Lazy marks saturation | Roadmap V5 | V5-3 engine + V5-6 TUI saturation column/detail; см. [plans/mvp_v5.md](plans/mvp_v5.md) | ✅ |
| Marks burn only (no tradeable balance) | §3 | `stored_marks` — отдельный счётчик, не переводимый | ✅ |
| Legacy `marks_quota` mirror (pwmd snapshot) | §3 | Удалён 2026-06-02 (pre-publish polish); canonical — `stored_marks` + lazy engine | ✅ |

### 2.7 Инфраструктура и DevOps

| Компонент | Реализация | Статус |
|---|---|---|
| Workspace: pwm-core, pwmd, pwm-cli, pwm-tui | Rust Cargo workspace | ✅ |
| Automated testing | `cargo test --workspace`, unit + integration | ✅ |
| Cluster lab scripts | CY lab (proposer/attester/follower), 2–3 node | ✅ |
| Automated smoke tests | `cy_cluster_mvp_v2_tail_smoke.ps1`, two-node smoke | ✅ |
| ClickHouse snapshot backend | Optional feature `clickhouse-snapshot` | ✅ |
| Agent prompts (orchestrator/coding/testing/review) | `.cursor/agents/`, `docs/AGENT_PROMPTS.md` | ✅ |
| Commit protocol (runtime → public mirror) | `git_safe_commit` MCP, two-tree workflow | ✅ |
| Code quality linting | `check_entity_name_segments.py`, name policies | ✅ |

---

## 3. Спринты MVP v2 — текущий статус

| Спринт | Описание | Статус | Примечание |
|---|---|---|---|
| **V2-1** | Спецификации, RFC pack (claims/burn), acceptance criteria | ✅ Закрыт | RFC 11–14, WHITE_SPEC §9 |
| **V2-2** | Единый `marks` в state, удаление `marks_quota` | ✅ Закрыт | Consensus state cleaned |
| **V2-3** | Эмиссия: киты + сезонный множитель | ✅ Закрыт | Policy v2, schema v5 |
| **V2-4** | BURN_MARK end-to-end, CLI/TUI burn flow | ✅ Закрыт | Full operator path |
| **V2-5** | marks u32 + формула нормализации | ✅ Закрыт | 1 PWM × 1h = 1 mark |
| **V2-6** | TUI Stake/Unstake + F5 auto-claim | ✅ Закрыт | F7/Shift+F7 формы |
| **V2-7** | Burn UX fixes + genesis marks | ✅ Закрыт | 6 дефектов устранено |
| **V2-8** | Same-shard sync v1 (slices 0–5) | ✅ Готово | Slice 6 → V2-9 |
| **V2-9** | RFC 16 cluster attestation | ✅ Закрыт (спринтовой gate) | См. `docs/reviews/20260510-v2-9-slice-bc-review.md`; legacy V2-8 slice 6 — `blocked`, не требуется для закрытия |

### 3b. Спринты MVP v5 — статус (sprint-final + pre-publish 2026-06-02)

| Gate | Описание | Статус | Артефакт |
|---|---|---|---|
| **V5-1** | RFC/ADR freeze (lazy marks, float inflation, ADR 0005–0007) | ✅ | [reviews/20260523-v5-sprint1-spec-adr-freeze-rereview.md](reviews/20260523-v5-sprint1-spec-adr-freeze-rereview.md) |
| **V5-2** | Core model: `marks_last_block`, deferred policies, `ClaimIPv4Batch`, schema v3 | ✅ | [reviews/20260524-v5-s2-review-fixes-rereview.md](reviews/20260524-v5-s2-review-fixes-rereview.md) |
| **V5-3** | Lazy marks engine + float inflation в seal | ✅ | `tasks/done/20260524-v5-sprint3-lazy-marks-inflation.json` |
| **V5-4** | Deferred activation (`activate_at_height`) | ✅ | `tasks/done/20260524-v5-sprint4-deferred-activation.json` |
| **V5-5** | IPv4 `ClaimIPv4Batch` on-chain | ✅ | `tasks/done/20260524-v5-sprint5-ipv4-claim-onchain.json` |
| **V5-6** | TUI marks saturation / effective display | ✅ | `tasks/done/20260524-v5-sprint6-tui-marks-saturation.json` |
| **V5-7** | CLI `account-info`, deferred policy CLI, genesis-21b doc | ✅ | `tasks/done/20260524-v5-sprint7-cli-genesis-doc.json` |
| **V5-8** | Integrated gate + operator closeout | ✅ | [plans/mvp_v5.md](plans/mvp_v5.md); `tasks/20260524-v5-sprint8-operator-closeout.json` |
| **V5-9** | CY cluster E2E (bootstrap, marks soak, mass burn) | ✅ | `tasks/done/20260530-v5-sprint-final-closeout.json` |
| **Pre-publish** | TUI operator UX, genesis anchor land, `marks_quota` cleanup | ✅ | [reviews/20260602-v5-prepublish-polish-integrated-review.md](reviews/20260602-v5-prepublish-polish-integrated-review.md); ADR [0008](adr/0008-snapshot-genesis-anchor-light.md) |

**Owner sign-off:** сводные критерии V5 закрыты владельцем (2026-06-02); детальный чеклист — [CONCEPT_ROADMAP.md](CONCEPT_ROADMAP.md) (§ MVP V5, внутренний документ).

---

## 4. Что уже возможно (demonstration-ready)

На текущий момент можно продемонстрировать:

1. **Два независимых geo-shard** — две ноды `pwmd` с разными `domain_hi`, раздельный state, реальный transport peering.
2. **Same-shard операции** — INIT, TRANSFER, STAKE, UNSTAKE, BURN_MARK через CLI и TUI.
3. **Cross-shard перевод** — EXPORT → relay → IMPORT с replay protection и bridge trust.
4. **Эмиссия марок** — стейк → accrue → burn с формулой `1 PWM × 1h = 1 mark`.
5. **TUI с полным циклом** — просмотр балансов, stake/unstake, send, burn (с auto-claim), wallet controls.
6. **Same-shard sync** — ведомая нода догоняет tip, mempool gossip, block delivery.
7. **Cluster attestation** — 2-of-2 и 2-of-3 по RFC 16, wire + негативные harness; ведомая same-shard нода вне кластера догоняет tip (приёмка спринта).
8. **Persistence** — JsonFile epoch-режим, autosnapshot, опционально ClickHouse.
9. **V5 lazy marks** — stake → poll head показывает `effective_marks` → F5 burn materialize; saturation в таблице/detail (runbook [v5-tui-marks-operator-path.md](runbooks/v5-tui-marks-operator-path.md)).
10. **IPv4 claim batch** — on-chain `ClaimIPv4Batch` с registry-sig (devnet authority path).
11. **Genesis anchor light** — snapshot v3 `genesis_anchor` по ADR 0008 при trust-load/migrate.

---

## 5. Что запланировано но ещё не реализовано

| Элемент | Источник | Приоритет |
|---|---|---|
| **Опционально: расширенные cluster fault tests** | RFC 16 §11 (partition-lite и т.д. сверх минимума чеклиста) | Низкий (backlog pwm-coding) |
| **Dumb contracts runtime** — policy engine execution, routing/TTL/filter | WP §5, WHITE_SPEC §9 | Средний (RFC готов, код нет) |
| **Arbitrator** — зональный арбитр, freeze/reversal | WP §6 | Низкий (defer) |
| **PQC** — quantum-safe signatures | WP §4.4 | Низкий (defer) |
| **IPv4 capping (production registry + фазы)** | WP §3 | Средний (on-chain batch есть; off-chain registry/bootstrap — defer) |
| **21B coin genesis (runtime amounts)** | WP §3 | Низкий (дизайн задокументирован; devnet amounts) |
| **X-PWM email header integration** | WP §7 | Низкий (defer) |
| **AI API integration** | WP §9 | Низкий (defer) |
| **Honeypot addresses** | WP §4.3 | Низкий (defer) |
| **Corporate multisig/cosign INIT** | WP §5 | Средний |
| **CLTV scheduling** | WP §5 | Низкий (defer) |
| **Offchain burning production** | WP §8 | Средний (stub есть) |

---

## 6. Архитектурные риски и открытые вопросы

1. **Консенсусный пивот** (multi-sealer → single proposer + cluster attestation) **для спринтового gate закрыт** (V2-9). Остаётся продуктовая полировка и long-run soak на реальных стендах.
2. **Policy engine gap:** WHITE_SPEC §9 описывает политики (routing, TTL, filter, default), но runtime execution не реализован. Это критический пробел для WP §5 «dumb contracts».
3. **Масштабирование offchain:** stub batch-burn есть, но нет production-ready API для интеграций (email platforms, messengers).
4. **Lazy marks (остаток):** движок и TUI saturation shipped (V5-3/V5-6, pre-publish polish). Остаётся операторский нит: accrual hint в TUI использует `DEF_BLOCKS_PER_HOUR`, не live genesis `/v1/status` — см. integrated review NIT-UX-1.
5. **Genesis amounts:** текущий devnet genesis ≠ 21B coins из WP; это нормально для dev, но потребует отдельного genesis-файла для демо.

---

## 7. Резюме покрытия Whitepaper

| Категория | Покрытие |
|---|---|
| **Реализовано и работает** | ~50% концепта (рост за счёт V5 tokenomics: lazy marks, float seal, IPv4 batch, genesis anchor) |
| **MVP-форма / частично** | ~25% (21B genesis amounts, IPv4 registry фазы, dumb contracts runtime, offchain stub) |
| **Запланировано / в работе** | ~8% (расширенные cluster fault tests, live-genesis TUI accrual hint) |
| **Defer / не начато** | ~15% (PQC, arbitrator, production IPv4 registry, AI integration, honeypots) |

**Общий вывод:** MVP v2 + **закрытый спринтовый gate V5** дают **основной технический каркас** и **токеномику марок/инфляции** в devnet-форме. Следующие критические шаги к demonstration-ready концепту:
- Опционально: **расширенный fault-matrix** для cluster (§11) и операторские soak на длинных цепочках
- Реализация **policy engine runtime** для dumb contracts (§5 WP)
- Минимальная **offchain burn API** для демо интеграций (§7–8 WP)
- Подготовка **демо genesis** с реалистичными amounts и валидаторами (дизайн: [genesis-21b-design.md](genesis-21b-design.md); runtime 21B — open)
- Опционально: live-genesis **TUI accrual hint** (`DEF_BLOCKS_PER_HOUR` → `/v1/status` params)
