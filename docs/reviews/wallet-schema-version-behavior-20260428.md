# Wallet Schema Version Behavior — 2026-04-28

Repo: `P:/opt/docker/PWM-cryptocurrency`  
Scope: объяснить, почему новые wallet-файлы сейчас могут быть schema v1/v2, где назначается `schema_version`, как работает `--upgrade-wallet`, и что менять для цели «все новые сохранения по умолчанию в latest schema».

> Историческая заметка: этот review фиксирует состояние до обновления write-path. Актуальный статус: create-paths (`addr-bruteforce`, `wallet init`, `wallet import-seed`) теперь сохраняют wallet сразу в schema v3.

## 1) Command paths: `addr-bruteforce`, `wallet init`, `wallet import-seed`, account-команды с `--upgrade-wallet`

### `addr-bruteforce`
- CLI путь: `crates/pwm-cli/src/main.rs` (`Cmd::AddrBruteforce`).
- После brute-force вызывает:
  - `resolve_bruteforce_wallet_protection(wallet_passphrase.as_deref())`
  - затем `to_wallet_yaml_with_metadata(...)`
  - затем `save_wallet_yaml(...)`.
- Ключевой момент: `resolve_bruteforce_wallet_protection` при отсутствии passphrase возвращает `PlaintextDev` + warning; при наличии passphrase — `Encrypted`.

### `wallet init`
- CLI путь: `crates/pwm-cli/src/main.rs` (`WalletCmd::Init`).
- Создание wallet:
  - `resolve_wallet_protection(wallet_passphrase.as_deref(), plaintext_dev)`
  - `to_wallet_yaml_with_metadata(...)`
  - `save_wallet_yaml(...)`.
- В отличие от `addr-bruteforce`, тут encrypted считается дефолтом, но требует passphrase; plaintext только через явный `--plaintext-dev`.

### `wallet import-seed`
- CLI путь: `crates/pwm-cli/src/main.rs` (`WalletCmd::ImportSeed`).
- Логика сохранения такая же, как у `wallet init`:
  - `resolve_wallet_protection(...)`
  - `to_wallet_yaml_with_metadata(...)`
  - `save_wallet_yaml(...)`.

### `wallet account list|add|use` + `--upgrade-wallet`
- CLI путь: `crates/pwm-cli/src/main.rs` (`WalletCmd::Account`).
- Перед `wallet account *` при `--upgrade-wallet` вызывается `load_wallet_yaml_with_upgrade(&wallet, true)`.
- Далее:
  - `wallet_account_list(...)`
  - `wallet_account_add(...)`
  - `wallet_account_use(...)`.
- Внутри account-функций требуется schema v3 (`load_wallet_yaml_v3_raw`), поэтому:
  - без `--upgrade-wallet` v2-файл отклоняется;
  - с `--upgrade-wallet` v2 может быть мигрирован на диск в v3 и команды продолжают работать.

## 2) Где назначается `schema_version` (v1/v2/v3), включая `plaintext_dev`/`encrypted`

### Назначение v1/v2 при создании
Файл: `crates/pwm-cli/src/wallet.rs`

- `to_wallet_yaml_with_metadata(...)` создаёт базовый объект с:
  - `schema_version: 2`
  - `mode: "encrypted"`.
- `apply_protection(...)`:
  - для `WalletProtection::PlaintextDev` принудительно ставит:
    - `wallet.schema_version = 1`
    - `wallet.mode = "plaintext_dev"`;
  - для `WalletProtection::Encrypted` оставляет schema `2` (и заполняет encrypted-поля).
- Итого при **новом сохранении** через текущий write-path:
  - plaintext_dev => v1
  - encrypted => v2

### Откуда берётся v3
Файл: `crates/pwm-cli/src/wallet.rs`

- `migrate_wallet_v2_to_v3(...)` формирует структуру с `schema_version: 3`.
- `load_wallet_yaml_with_upgrade(path, upgrade_wallet)`:
  - если прочитан v2, делает in-memory миграцию в v3;
  - если `upgrade_wallet == true`, пишет мигрированный v3 на диск (`save_wallet_yaml_v3`).
- `parse_wallet_yaml_v3(...)` и маппинг в `WalletYaml` выставляют `schema_version: 3` на read-path.
- `wallet_account_add/use` работают только с v3 и пишут через `save_wallet_yaml_v3`.

### Детекция версии и legacy default
- В `pwm-cli` (`wallet.rs`) и `pwm-core` (`wallet_read.rs`) `detect_schema_version(...)` использует `unwrap_or(2)`.
- Это означает: если поле `schema_version` отсутствует, формат трактуется как v2 (legacy совместимость).

## 3) Текущее поведение (исторический снимок): by design или gap?

### Что явно by design
- Документация CLI (`docs/pwm-cli.md`) фиксирует:
  - `--upgrade-wallet` как явный opt-in для персистентной миграции v2 -> v3;
  - без флага read-path не перезаписывает файл;
  - `addr-bruteforce` может сохранять plaintext_dev при отсутствии passphrase (с warning).
- Тесты в `wallet.rs` подтверждают контракт:
  - без upgrade-флага v2 не перезаписывается;
  - с upgrade-флагом миграция в v3 сохраняется на диск.

### Где был gap относительно цели «все новые сохранения latest schema»
- На момент этого снимка новый wallet в `addr-bruteforce`, `wallet init`, `wallet import-seed` создавался не как v3, а через legacy-конструктор (`to_wallet_yaml_with_metadata` + `save_wallet_yaml`), который выдавал v1/v2 в зависимости от protection mode.
- На момент этого снимка для цели «все новые сохранения = v3» write-path не соответствовал цели.

## 4) Практическая рекомендация (на момент снимка): что менять для default latest schema

Если цель: **любое новое сохранение wallet сразу в v3**, то нужно менять именно create/write-path, а не только read migration.

Рекомендуемый вектор:
1. Перевести `addr-bruteforce`, `wallet init`, `wallet import-seed` на создание v3-структуры при первичной записи (единый v3-конструктор).
2. Сохранить dual-mode security semantics:
   - `plaintext_dev` остаётся явным opt-in;
   - `encrypted` остаётся default, как сейчас.
3. Оставить `--upgrade-wallet` для legacy-файлов v2 (обратная совместимость), но новые файлы уже не должны идти через v1/v2-конструктор.
4. Обновить docs/тесты:
   - ожидания по schema для новых файлов;
   - smoke/CLI-тесты для обоих режимов (`plaintext_dev`/`encrypted`) в v3;
   - regression, что legacy v2 всё ещё корректно читается/мигрирует.

## Итоговый ответ на вопросы пользователя (исторический)

- Почему новые файлы всё ещё могут быть schema 1 (addr-bruteforce) и schema 2 (wallet init)?
  - Потому что текущий конструктор записи (`to_wallet_yaml_with_metadata` + `apply_protection`) специально выставляет v1 для `plaintext_dev` и v2 для `encrypted`; команды создания wallet используют именно этот путь.
- Должны ли уже сейчас все новые сохранения быть v3?
  - По текущей реализации и документации — нет, это не текущий контракт; v3 сейчас в основном через миграцию/`--upgrade-wallet` и v3 account-flow.
  - Если целевая политика изменилась на «default latest schema for all new saves», это отдельная продуктовая доработка write-path.
