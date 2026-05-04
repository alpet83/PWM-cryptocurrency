# RFC 10 — Формат файла кошелька (schema v3, multi-address)

**Статус:** draft (Sprint 14).  
**Заменяет черновик ссылки «RFC 8 wallet»:** номер `8` занят [8-shard-runtime-identity-and-peering.md](8-shard-runtime-identity-and-peering.md).

## 1. Мотивация

- Текущий `schema_version: 2`: один signing-аккаунт на файл (`derivation_*`, `account_id_*` на корне).
- Требование: несколько **owned** адресов (ветки `derivation_path`) в одном файле, общий KDF/AEAD и `address_book` для получателей.
- Терминология: человеко-читаемое представление id в YAML — поле **`id_pretty`** (ранее в документации/CLI фигурировало как `account_id_human` / «human»); см. [docs/CHANGELOG.md](../CHANGELOG.md).

## 2. Нормативный пример YAML (отступ **4 пробела** на уровень)

```yaml
schema_version: 3
mode: encrypted
created_at_unix_sec: 1777220793
wallet_created_at_unix_sec: 1777220793
country_code_label: null
encrypted_payload_b64: "y...jw=="
aead_nonce_b64: "7u4WJU7kOQI9vsFV"
kdf: pbkdf2_sha256
kdf_iters: 100000
accounts:
    - derivation_path: "m/0/105053"
      derivation_index: 105053
      domain_u16: 11515
      flags_mask_u32: 1023
      expected_flags_u32: 1
      flags_derived_u32: 505245697
      id_hex: "2cfb1e1d7001d108b39e05b194f2d1b126931bbfef38506e34297a5474ddae5e"
      id_pretty: "pwm1-CY/FB-f1E1D7001-td108b39e05b194f2d1b126931bbfef38506e34297a5474ddae5e"
      added_at_unix_sec: 1777220800
address_book:
    - address: "pwm1qyqqqqpvqqkqqr8ezaj6wqstk687pgqfdrfrhqefdqzeeal7fta3jwuyfmnxkqn7fr3"
      label: "CY-receiver-1"
```

### 2.1 Семантика времени

- **`created_at_unix_sec`** (корень): сохраняется как в v2 — момент **первичного** создания файла кошелька (backward compatibility для операторов).
- **`wallet_created_at_unix_sec`** (опционально): дубликат явной семантики «кошелёк как контейнер»; если отсутствует, равен `created_at_unix_sec` при чтении.
- **`accounts[].added_at_unix_sec`** (опционально): момент добавления **данной** ветки; рекомендуется выставлять в CLI при `wallet account add`.

## 3. Поля

### 3.1 Корень файла

| Поле | Обязательность | Описание |
|------|----------------|----------|
| `schema_version` | да | целое `3` |

Если ключ `schema_version` **отсутствует** в файле (legacy), загрузчики в `pwm-cli` / `pwm-core` интерпретируют это как **`2`** — текущий стабильный формат до v3 (согласовано с `serde(default)` на `WalletReadHeader`).
| `mode` | да | `encrypted` \| `plaintext_dev` |
| `created_at_unix_sec` | да | unix sec создания файла (v2-семантика) |
| `wallet_created_at_unix_sec` | нет | см. §2.1 |
| `country_code_label` | нет | опциональная метка уровня кошелька |
| KDF/AEAD поля | для `encrypted` | как в v2 |
| `active_account_id_hex` | нет, legacy | прежний wallet-level marker; загрузчики не требуют его и не используют как криптографический источник |
| `accounts` | да | непустой массив owned-записей |
| `address_book` | нет | как в v2 — allow-list для `tx-send --wallet` |

### 3.2 Элемент `accounts[]`

| Поле | Обязательность | Описание |
|------|----------------|----------|
| `derivation_path` | да | MVP: только строка вида `m/0/<u32>` |
| `derivation_index` | да | должен согласовываться с path |
| `domain_u16` | да | high-domain; должен совпадать с байтами `id_hex` |
| `flags_mask_u32` / `expected_flags_u32` / `flags_derived_u32` | да | policy как в v2 per-account |
| `id_hex` | да | 32-byte hex без `0x` |
| `id_pretty` | да | pretty-строка для UI/логов (бывш. `account_id_human`) |
| `added_at_unix_sec` | нет | см. §2.1 |

**Инварианты:** пересчёт `account_id_from_parts(pk, derivation_index)` для записи из master seed должен совпадать с `id_hex`; иначе `load` → ошибка.

## 4. Encrypted payload (MVP — вариант **A**)

В AEAD-блоке хранится JSON (или бинарный TLV — вне MVP) минимум с:

- `master_seed_hex` (или только бинарные 32 байта в hex).

Ключи подписи для любого `accounts[]` элемента **не** хранятся отдельно; выводятся из master + `derivation_index` при unlock.

## 5. Миграция с v2

1. Одна запись `accounts[0]` из корневых полей v2.
2. `id_pretty` = `account_id_human` v2.
3. Новые записи не обязаны сохранять `active_account_id_hex`; runtime выбирает signing account по команде/выделенной строке, а CLI без явного selector использует детерминированный default: минимальный `derivation_index`, затем минимальный `id_hex`.
4. Команда CLI `wallet upgrade-v3` (имя на усмотрение реализации) — явная миграция; автоматическая перезапись при первом открытии **не** обязательна (решение реализации + review).

## 6. CLI / TUI (acceptance Sprint 14)

- CLI: `wallet account list | add | use`.
- TUI: **левая панель** — список **всех** `accounts[]` (pretty + короткий hex); выбранная строка Owner является sender для F6/подписи.

## 7. Связанные документы

- [sprint-14-wallet-schema-audit.md](../reviews/sprint-14-wallet-schema-audit.md)
- [pwm-cli.md](../pwm-cli.md)
- [WALLET_SECURITY_MODES.md](../WALLET_SECURITY_MODES.md)
