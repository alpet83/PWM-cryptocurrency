# Sprint 14 — аудит полей wallet YAML (v2 → v3)

**Роль:** pwm-review (read-only анализ исходников).  
**Дата:** 2026-04-27  
**Метод поиска:** локальный `rg` по `P:/opt/docker/pwm-protocol/crates` (эквивалент полноты для офлайн-копии). Для проекта, зарегистрированного в **Colloquium/CQDS**, повторить тот же паттерн поиска через **`cq_select_project`** → **`cq_files_ctl` / smart-grep по `crates/`** (см. skill `colloquium-cqds-mcp`), чтобы не зависеть от IDE search.

## Сводная таблица полей v2

| Поле | Уровень (v2) | Семантика | Потребители (файлы) | Рекомендация для v3 |
|------|----------------|-----------|---------------------|---------------------|
| `schema_version` | корень | версия формата файла | `pwm-cli` wallet, serde | оставить корень; значение `3` |
| `mode` | корень | `encrypted` / `plaintext_dev` | `wallet.rs`, TUI | корень |
| `created_at_unix_sec` | корень | время создания записи wallet (сейчас `WalletYaml::now_unix_sec` при создании) | `wallet.rs` | **уточнить:** оставить как *время создания файла кошелька* на корне; при multi-account добавить опционально `added_at_unix_sec` **внутри** каждого элемента `accounts[]` для аудита добавления адреса (RFC 10 вариант **b**). |
| `country_code_label` | корень | метка регуляторного домена (опционально) | `wallet init/import` | корень: default label для новых аккаунтов или `null`; пер-аккаунт при необходимости позже |
| `derivation_index` | корень | единственный owned индекс | CLI, TUI, `wallet_read` | **перенести в `accounts[]`**; корень удалить для v3 |
| `derivation_path` | корень | `m/0/N` | wallet | **перенести в `accounts[]`** |
| `domain_u16` | корень | домен активного аккаунта | load/validate | **перенести в `accounts[]`**; дублировать на корне не рекомендуется |
| `flags_mask_u32` / `expected_flags_u32` / `flags_derived_u32` | корень | user-profile flags | bruteforce, tx | **перенести в `accounts[]`** |
| `account_id_hex` | корень | canonical id hex | signing, show | v3: в записи аккаунта как **`id_hex`** (или оставить имя `account_id_hex` — выбрать один канон в RFC 10; здесь предложено **`id_hex`** + миграция) |
| `account_id_human` | корень | pretty human (legacy имя) | CLI show, TUI | v3: **`id_pretty`** (та же строка; переименование для терминологии, см. changelog) |
| `master_seed_*` / `signing_key_*` / `verifying_key_*` | корень (plaintext) или внутри encrypted payload | секреты | `wallet_secrets`, apply_protection | v3 MVP: payload **A** — только master seed; ключи per-account деривировать |
| `encrypted_payload_b64`, KDF/AEAD поля | корень | шифрование | `wallet_crypto` | корень без изменений роли |
| `address_book` | корень | allow-list получателей, **не** owned-адреса | `address_book.rs`, CLI | корень; семантика не меняется |
| `ignored_legacy_pretty_entries` | serde skip, runtime | счётчик миграций book | load | оставить вне сериализации / аналог для v3 |

## WalletReadHeader (pwm-core)

Те же поля identity на **одном** аккаунте: `derivation_index`, `account_id_human`, `domain_u16`, optional `account_id_hex`, secrets optional. Для v3 потребуется либо расширение типа, либо отдельный `WalletReadV3` + загрузчик.

## Вывод по `created_at_unix_sec`

В текущем коде поле задаётся **один раз при создании** файла (`to_wallet_yaml_with_metadata` → `WalletYaml::now_unix_sec`) и отражает **момент записи wallet-файла**, а не отдельного адреса. Для multi-address рекомендация RFC:

- **Корень:** `created_at_unix_sec` — время первичного создания файла кошелька (backward compat).
- **`accounts[]`:** опционально `added_at_unix_sec` на каждую запись при `wallet account add`.

## Следующие шаги (оркестратор)

1. Зафиксировать RFC: [docs/rfc/10-wallet-file-format-v3.md](../rfc/10-wallet-file-format-v3.md).  
2. Конвейер код-слайсов: `pwm-coding` → `pwm-testing` → `pwm-review` на каждый инкремент (serde, миграция, CLI, TUI).
