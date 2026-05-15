# CONCEPT_PROGRESS.md — Прогресс MVP v2 к реализации DRAFT_WHITEPAPER

**Дата:** 2026-05-22  
**Цель:** Оценить насколько текущая реализация (MVP v2) покрывает концепт из `DRAFT_WHITEPAPER.md`.  
**Уровни готовности:** ✅ Полностью | 🔶 Частично / MVP-форма | 🔄 В работе | ⏳ Запланировано (план/roadmap) | ❌ Не начато / Defer

---

## 1. Общая картина

| Раздел Whitepaper | Состояние | Комментарии |
|---|---|---|
| §1 Introduction — концепция и цели | ✅ | Концепция зафиксирована, документация согласована |
| §2 Purpose — анти-спам, псевдоавторизация | 🔶 | `BURN_MARK` реализован (CLI + TUI F5), но интеграция с внешними системами (email, messengers) — defer |
| §3 Economic model — эмиссия, инфляция, марки | 🔶 | Эмиссия PWM через V2-3 (киты + сезонность); марки accrue/burn работают; инфляция float + IPv4 capping — defer |
| §4 Адресация, кластеры, HD-derivation, honeypots | 🔶 | HD-derivation (SLIP-0010), domain_code, bech32DX — реализованы; honeypots — не начато |
| §4.4 PQC (quantum-safe signatures) | ❌ | Ed25519; Dilithium/SPHINCS+ — явно defer |
| §5 Dumb Contracts (политики, multisig, CLTV) | 🔶 | Базовые policy hooks (recipient domain, TTL, sender filter, routing, default) — RFC/спеки; runtime — не реализован |
| §6 Arbitrator — частичная централизация | ❌ | Не начато; концепт зарезервирован |
| §7 Anti-spam и марки на практике | 🔶 | Сжигание марок end-to-end работает (CLI/TUI); X-PWM header для email, AI API — defer |
| §8 Offchain burning + L3 | 🔶 | Offchain stub (`docs/OFFCHAIN_STUB.md`, `offchain-batch`); batch Merkle demo — без продакшн интеграции |
| §9 AI integration | ❌ | Концепт описан, код не начат |
| §10 Business model | ✅ | Описано в WP; не требует кода |
| §11 Roadmap (Phases 1–4) | 🔄 | Phase 1–2 в работе (MVP v2); Phase 3–4 — будущее |
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
| Marks accrual formula (1 PWM × 1 hour = 1 mark) | §3 | V2-5: `matured = (staked / 1_000_000) * hours` | ✅ |
| Marks type normalization (u32) | §3 | V2-5: `Account.marks: u32` | ✅ |
| Block reward to producer | §3 | `RewPol::ToProducerAccount` → policy-gated v2 | ✅ |
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
| ClaimTx (explicit + auto-claim) | §3 | V2: free/paid, maturity, anchor ref | ✅ |
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
| TUI: F5 Burn form (auto-claim, marks display) | §7 | V2-4 + V2-6 + V2-7: full burn flow | ✅ |
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
| IPv4 capping distribution | §3 | Не реализовано | ❌ |
| Floating annual inflation (~5%) | §3 | Фиксированный `block_reward` → policy-gated v2 | 🔶 |
| Seasonal variation (лето/зима) | §3 | V2-3: `season_coeff_ppm` | ✅ |
| Marks generation daily from staking | §3 | Per-block accrual, maturity formula | ✅ |
| Marks demurrage (TTL) | §3 | WHITE_SPEC §5: опционально, не реализовано | ❌ |
| Marks burn only (no tradeable balance) | §3 | `marks` — отдельный счётчик, не переводимый | ✅ |
| Unified marks balance (v2) | §3 | V2-2: `marks_quota` → единый `marks` | ✅ |

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

---

## 5. Что запланировано но ещё не реализовано

| Элемент | Источник | Приоритет |
|---|---|---|
| **Опционально: расширенные cluster fault tests** | RFC 16 §11 (partition-lite и т.д. сверх минимума чеклиста) | Низкий (backlog pwm-coding) |
| **Dumb contracts runtime** — policy engine execution, routing/TTL/filter | WP §5, WHITE_SPEC §9 | Средний (RFC готов, код нет) |
| **Arbitrator** — зональный арбитр, freeze/reversal | WP §6 | Низкий (defer) |
| **PQC** — quantum-safe signatures | WP §4.4 | Низкий (defer) |
| **IPv4 capping** — распределение по адресам | WP §3 | Низкий (defer) |
| **Full inflation model** — float ~5% annual | WP §3 | Средний (частично через season_coeff) |
| **Marks demurrage (TTL)** | WP §3 | Низкий (defer) |
| **Offchain burning production** | WP §8 | Средний (stub есть) |
| **X-PWM email header integration** | WP §7 | Низкий (defer) |
| **AI API integration** | WP §9 | Низкий (defer) |
| **Honeypot addresses** | WP §4.3 | Низкий (defer) |
| **Corporate multisig/cosign INIT** | WP §5 | Средний |
| **CLTV scheduling** | WP §5 | Низкий (defer) |
| **21B coin genesis** | WP §3 | Низкий (сейчас devnet amounts) |

---

## 6. Архитектурные риски и открытые вопросы

1. **Консенсусный пивот** (multi-sealer → single proposer + cluster attestation) **для спринтового gate закрыт** (V2-9). Остаётся продуктовая полировка и long-run soak на реальных стендах.
2. **Policy engine gap:** WHITE_SPEC §9 описывает политики (routing, TTL, filter, default), но runtime execution не реализован. Это критический пробел для WP §5 «dumb contracts».
3. **Масштабирование offchain:** stub batch-burn есть, но нет production-ready API для интеграций (email platforms, messengers).
4. **TTL марок (demurrage):** ключевой элемент WP §3, не реализован даже как stub.
5. **Genesis amounts:** текущий devnet genesis ≠ 21B coins из WP; это нормально для dev, но потребует отдельного genesis-файла для демо.

---

## 7. Резюме покрытия Whitepaper

| Категория | Покрытие |
|---|---|
| **Реализовано и работает** | ~45% концепта |
| **MVP-форма / частично** | ~25% (экономика базовая, политики как RFC, offchain stub) |
| **Запланировано / в работе** | ~10% (расширенные cluster fault tests, policy runtime) |
| **Defer / не начато** | ~15% (PQC, arbitrator, IPv4 capping, AI integration, demurrage, honeypots) |

**Общий вывод:** MVP v2 закрывает **основной технический каркас** блокчейн-ядра (консенсус, шардинг, синхронизация, эмиссия, марки, клиенты). Следующие критические шаги к demonstration-ready концепту:
- Опционально: **расширенный fault-matrix** для cluster (§11) и операторские soak на длинных цепочках
- Реализация **policy engine runtime** для dumb contracts (§5 WP)
- Минимальная **offchain burn API** для демо интеграций (§7–8 WP)
- Подготовка **демо genesis** с реалистичными amounts и валидаторами
