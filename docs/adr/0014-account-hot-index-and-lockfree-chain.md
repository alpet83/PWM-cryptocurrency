---
adr: "0014"
title: "Account Hot Index и Lock-Free Block Chain для worker precheck"
status: draft
date: 2026-06-27
related:
  - docs/adr/0013-tx-pipeline-seda.md
  - docs/plans/perf-optimization-spectrum.md
  - docs/reviews/v7-s2-ramp-results.md
  - docs/reviews/v7-s3-worker-scale-results.md
---

# ADR 0014 — Account Hot Index и Lock-Free Block Chain

## Контекст

V7-S2/S3 показал: узкое место worker precheck — клонирование `Arc<State>` в
`precheck_apply_with_ctx` (O(N аккаунтов) на каждый tx). При 8 воркерах
параллельные клоны конкурируют за память и дают регрессию throughput
относительно 2-воркерного baseline (44 vs 52 tx/block на debug build).

Источник истины (`Chain::seal`) остаётся на `Arc<State>` — это не меняется.
Нужен легковесный hot-path индекс для worker precheck.

---

## Решение

### Уровень 1 — Account Hot Index (текущий sprint scope)

Плоская in-memory карта балансов, обновляемая только sealer'ом:

```rust
struct AccountHot {
    balance:         u128,   // текущий баланс
    nonce:           u64,    // текущий nonce
    flags:           u32,    // флаги аккаунта
    active_policies: u8,     // 0 → skip evaluate_policy (plain transfer fast-path)
    initialized:     bool,
}

type HotIndex = ArcSwap<HashMap<AccountId, AccountHot>>;
```

**Инвариант:** только seal loop пишет в индекс. Воркеры — только читают.

**Чтение воркером:**
```rust
let snap = hot_index.load();  // O(1), Arc clone, без локов
let hot = snap.get(&sender_id)?;
if hot.balance < amount + fee { return Err(TxRejectReason::PrecheckFailed) }
if hot.nonce != expected_nonce { return Err(TxRejectReason::StaleDuplicate) }
if hot.active_policies == 0 { /* skip evaluate_policy */ }
```

**Обновление после seal:**
```rust
// Seal выдаёт список изменённых аккаунтов
let mut new_map = (**hot_index.load()).clone();  // O(N), раз в блок ~1 сек
for changed in seal_result.modified {
    new_map.insert(changed.id, AccountHot::from(&changed));
}
hot_index.store(Arc::new(new_map));  // атомарный swap
```

**Инициализация при старте:**
- С ClickHouse: `SELECT account_id, balance, nonce, flags, active_policies FROM pwm_account_state WHERE block_height = max(block_height)`
- Без CH: однократный обход `current_state` — `for (id, acct) in state.accounts() { map.insert(...) }` (O(N), выполняется один раз)
- Флаг `[pipeline] index_mode = "rescan"` форсирует rescan вместо CH-загрузки

**Масштаб:** 40 байт × 100k аккаунтов = 4 МБ. До ~1M аккаунтов — полная карта без ограничений.

---

### Уровень 2 — Lock-Free Block Chain (Tier 2 / будущий ADR)

При росте числа аккаунтов (> ~1M) полная карта потребует сотни МБ.
Решение: LRU-карта только для горячих аккаунтов + fallback по цепочке блоков.

**Структура цепочки:**

```rust
struct SealedBlock {
    height:   u64,
    prev:     Option<Arc<SealedBlock>>,  // immutable после seal
    accounts: HashMap<AccountId, AccountHot>,  // только изменённые в этом блоке
}

// Tail — атомарно обновляется при каждом seal
static CHAIN_TAIL: ArcSwap<SealedBlock>;
```

Воркер делает `CHAIN_TAIL.load()` (O(1), без лока), затем при cache miss
траверсирует `prev` назад — каждый блок immutable, Arc гарантирует
что блок не освобождается пока воркер держит ссылку.

**Цепочка fallback для преобразования cache miss:**

```
1. Hot index (ArcSwap<HashMap>)          O(1)  — горячие аккаунты
2. CH: pwm_account_state                 O(1)  — если CH доступен
3. Lock-free chain [tip .. snap_height]  O(K)  — последние K блоков в памяти
   (bounded: K = число блоков с последнего fat snapshot)
4. Полный rescan от snapshot             O(N)  — холодный путь, ожидаемо медленный
```

**Гарантия:** большинство нод держат в памяти цепочку от текущего tip до
ближайшего fat snapshot. Fat snapshot либо в CH (быстро, через Tier 2),
либо в файловой системе. Без snapshot и без CH — полный rescan, медленно,
это ожидаемое поведение при деградированной конфигурации.

**Ограничение in-memory цепочки:**
```rust
const MAX_CHAIN_MEMORY_BLOCKS: u64 = 1_000;  // или до ближайшего snap
// При превышении — старые блоки освобождаются (Arc refcount → 0)
```

**Обновление tail при seal:**
```rust
let new_block = Arc::new(SealedBlock {
    height: new_height,
    prev: Some(Arc::clone(&CHAIN_TAIL.load())),
    accounts: changed_accounts,
});
CHAIN_TAIL.store(new_block);  // Release store, воркеры видят через Acquire load
```

Это SPMC: один producer (sealer), много read-only consumers (воркеры).
Нет CAS, нет mutex — корректность гарантируется immutability блоков и
атомарностью ArcSwap.

---

## Что НЕ делать

**Воркеры не должны писать в hot index.** При параллельных tx от одного
отправителя два воркера могут прочитать один balance, оба принять tx,
оба записать уменьшенный balance — двойная трата, которую seal поймает
eviction'ом. Это отменяет цель zero-eviction из V7-S2.

---

## Матрица решений

| Сценарий | Архитектура | Когда |
|----------|-------------|-------|
| ≤1M аккаунтов, без CH | Полная ArcSwap<HashMap> | Текущий sprint (V7-S3+) |
| ≤1M аккаунтов, с CH | Полная карта, CH для инициализации | Tier 2 |
| >1M аккаунтов | LRU hot map + lock-free chain fallback | ADR отдельный |
| >10M аккаунтов | CH как primary read backend | Tier 2 полный |

---

## Статус

- [x] Концепция обсуждена (2026-06-27)
- [ ] Уровень 1 (HotIndex) — coding ticket создан
- [ ] Уровень 2 (lock-free chain) — отдельный ADR при переходе к Tier 2
