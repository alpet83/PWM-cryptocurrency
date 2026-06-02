# ADR 0008: Genesis anchor для Epoch Snapshot (лёгкая проверка + миграция)

## Статус

Implemented (operational layer, не Bootstrap Snapshot из ADR 0004).

## Контекст

**Проблема:** при trust-default load (`validate_snapshot_trusted`) узел доверяет локальному `pwm-data.json` + `epochs/`. Связка с `--genesis-file` сейчас сводится к сравнению `genesis_accounts` с `GenCfg`; `digest(state0())` используется в peer hello, но **не обязателен** при load snapshot.

Следствие: можно подменить checkpoint `state` и epoch-историю (согласованно между собой), оставив тот же genesis file — нода продолжит seal на чужой ветке. Особенно критично для devnet/CY, где агенты и скрипты часто правят `tmp/state-*`.

**Ограничения владельца (2026-06):**

1. Полный replay genesis→tip без оптимизаций сейчас **дорог** — нужны лёгкие проверки на trust path.
2. Достаточно **одной** подписи якоря (защита от легкомысленной правки / ИИ-агента), не k-of-n Bootstrap Snapshot.
3. **Block height=1** — надёжный референс для контроля prune/tail: в PWM нет sealed block 0; genesis = `state0()` + `prev_gen()`; block 1 цепляется к genesis и несёт PoA-подпись producer из validator set (при cluster — дополнительно cluster attest на wire, но якорь для диска — header block 1).
4. **Миграция** старых snapshot без поля обязательна.

Связанные документы: [guide-node-storage-and-snapshot.md](../guide-node-storage-and-snapshot.md), [FEATURES.md](../FEATURES.md), ADR [0004](0004-cleanup-chain-bootstrap-snapshot-and-anchoring.md) (Bootstrap Snapshot — отложено).

## Решение

### 1. Поле `genesis_anchor` в snapshot wire (v3, `schema_v=1`)

Добавить в `pwm-data.json` (v3 wire) объект:

```json
"genesis_anchor": {
  "schema_v": 1,
  "genesis_state_root": "<hex32>",
  "gencfg_digest": "<hex32>",
  "block1_hdr_hash": "<hex32>",
  "signer_prod_idx": 0,
  "signature": "<hex64>"
}
```

| Поле | Смысл |
|------|--------|
| `genesis_state_root` | `digest(GenCfg.state0())` — виртуальный genesis state |
| `gencfg_digest` | Канонический hash экономики/validator set/network (см. impl: стабильный subset `GenCfg`, без секретов) |
| `block1_hdr_hash` | `hdr_hash(block1.hdr)`; для `checkpoint_height==0` — 32 нулевых байта |
| `signer_prod_idx` | Индекс валидатора в `cfg.vals.set`, чей pubkey проверяет `signature` |
| `signature` | Ed25519 над `anchor_preimage` |

**`anchor_preimage` (фиксированный порядок, blake3):**

```text
PWMv0/SNAPGENANCHOR/v1
|| genesis_state_root (32)
|| gencfg_digest (32)
|| block1_hdr_hash (32)
```

Подпись: `sign(validator[signer_prod_idx].sk, anchor_preimage)` — **одна** подпись при создании/миграции якоря (fool-protection). Не путать с PoA-подписью **внутри** `block1.hdr.sig`.

### 2. Лёгкие проверки на trust load (без full replay)

После decode snapshot, **до** принятия `state`:

1. **Commitments:** `genesis_state_root` и `gencfg_digest` MUST совпадать с значениями, вычисленными из `--genesis-file` → иначе `fail-closed`.
2. **Anchor signature:** если `genesis_anchor` присутствует — `verify(signature)` против `cfg.vals[signer_prod_idx]` → иначе `fail-closed`.
3. **Genesis preflight (block 1):** если `checkpoint_height >= 1`:
   - загрузить block height=1 из epochs (или tail, если tip==1);
   - `block.hdr.prev_hash == prev_gen()`;
   - `hdr_hash(block.hdr) == genesis_anchor.block1_hdr_hash`;
   - PoA `block.hdr.verify_sig(producer_pk)`;
   - **лёгкий replay:** применить txs block 1 к `cfg.state0()`, сравнить `digest(state_after_1)` с `block.hdr.state_root` (ловит подмену genesis distribution без полного rescan).
4. Если `checkpoint_height == 0`: `digest(snapshot.state) == genesis_state_root` (уже частично есть; унифицировать с anchor).

Full replay (`--snapshot-verify-chain`) остаётся audit mode; не меняется.

### 3. Запись якоря

- При каждом **save** snapshot (autosnapshot / shutdown persist): всегда пересчитывать и записывать `genesis_anchor` (подпись — см. ниже).
- Подписывает узел, если в runtime есть приватный ключ validator, совпадающий с `signer_prod_idx` (обычно proposer / genesis-decrypted key). Иначе save без подписи **запрещён** для новых snapshot; для migrate — отдельный путь.

### 4. Миграция (обязательна)

Старые `pwm-data.json` без `genesis_anchor`:

| Условие | Поведение |
|---------|-----------|
| Trust load, preflight (п.2–3) **PASS**, у узла есть genesis validator key | **Migrate-on-load:** вычислить anchor, подписать, `warn` один раз (`snapshot genesis_anchor migrated at load`), принять state; при следующем autosnapshot — persist в файл |
| Trust load, preflight PASS, **нет** ключа | Load с `warn` + in-memory anchor **без** подписи **нельзя** — требовать `PWM_SNAPSHOT_ANCHOR_MIGRATE=1` **и** однократный `--snapshot-verify-chain` **или** CLI `pwm-cli snapshot-anchor-migrate` (подпись офлайн) |
| Preflight FAIL | **fail-closed**, без bypass |
| `PWM_SNAPSHOT_VERIFY_CHAIN=1` | Full replay; после успеха — записать anchor при следующем save |

**Не** делать silent accept старых snapshot без хотя бы commitments check после grace period (опционально: env `PWM_SNAPSHOT_ANCHOR_LEGACY_OK=1` только для CI fixtures — document as unsafe).

### 5. Prune / tail и block 1

- Epoch layout хранит block 1 в `epochs/block_e0.json` (или аналог). Preflight **обязан** уметь загрузить height=1 с диска, даже если tail не включает block 1.
- Если block 1 **физически отсутствует** (будущий prune): trust load **fail** с явным `missing genesis anchor block1 (pruned)` — оператор должен restore epoch e0 или `--snapshot-verify-chain` с полным архивом. Это и есть «контроль prune дистрибуции» через референс block 1.

### 6. Вне scope этого ADR

- k-of-n подпись snapshot summary (→ ADR 0004 Bootstrap Snapshot).
- Подпись каждого epoch файла.
- ClickHouse trust-default (остаётся full replay).
- Изменение wire consensus / PWM_PROTOCOL_VERSION.

## Последствия

- Trust-default перестаёт быть «слепым» к подмене genesis checkpoint при сохранении скорости.
- Старые devnet state потребуют однократной миграции или verify-chain.
- Genesis build / `demo-genesis-*` должны при первом snapshot записывать anchor (coding follow-up).
- [FEATURES.md](../FEATURES.md) — ссылка на этот ADR.

## Критерии приёмки (coding)

- Unit: anchor preimage sign/verify; mismatch genesis_state_root → err; block1 tamper → err.
- Integration: load legacy snapshot without anchor → migrate + warn; save → anchor present.
- `cargo test -p pwmd` snapshot/io tests; без регрессии trust load на CY fixtures.
- Runbook: [guide-node-storage-and-snapshot.md](../guide-node-storage-and-snapshot.md) § Genesis anchor.

## Эволюция к Bootstrap Snapshot (pruned distribution)

ADR 0008 — **операционный** слой (`Epoch Snapshot`). Для **pruned chains** опора — [RFC 0020](../rfc/20-bootstrap-snapshot-pruned-distribution.md) + [ADR 0004](0004-cleanup-chain-bootstrap-snapshot-and-anchoring.md).

| ADR 0008 (сейчас) | Bootstrap Snapshot (будущее) |
|-------------------|------------------------------|
| `genesis_state_root`, `gencfg_digest` | Те же поля в `genesis_fingerprint` (обязательны) |
| `block1_hdr_hash` | `chain_origin_hdr_hash` (неизменяемый якорь происхождения) |
| Одна подпись validator | **k-of-n** `shard_validator_attestations[]` по активному validator set шарда |
| Локальный trust load | Публикация + cleanup-chain commitment; дистрибуция pruned пакета |

**Совместимость impl:** имена и preimage ADR 0008 не менять без bump `schema_v`; Bootstrap добавляет обёртку и кворум, не ломает epoch wire.

## Ссылки

- `crates/pwmd/src/snapshot/io.rs` — `validate_snapshot` / `validate_snapshot_trusted`
- `crates/pwm-core/src/chain.rs` — `prev_gen()`
- [RFC 0020: Bootstrap Snapshot и pruned distribution](../rfc/20-bootstrap-snapshot-pruned-distribution.md)
- [ADR 0004](0004-cleanup-chain-bootstrap-snapshot-and-anchoring.md)
- `docs/reviews/20260531-v5-cy-lab-seal-manual-console-shutdown-review.md` (контекст CY lab, не эта фича)
