# `pwm-cli`: техническая документация

`pwm-cli` (бинарник `pwm`) — developer CLI-обвязка для локального кошелька и отправки транзакций в `pwmd`.

Security-режимы wallet и базовая operational hygiene: [WALLET_SECURITY_MODES.md](WALLET_SECURITY_MODES.md).

## Роль и границы

**`pwm-cli`**
- формирует и подписывает `SignedTx` на стороне клиента;
- дергает HTTP RPC (`/v1/account/:id`, `/v1/tx`);
- не хранит chain-state и не валидирует полные правила консенсуса.

**`pwmd`**
- принимает tx, делает `validate_tx_shape`, кладет в mempool и запечатывает блоки;
- возвращает `nonce` и статусы HTTP.

**`pwm-core`**
- криптография, derivation (`hd`), типы tx, сериализация и `sign_body`;
- проверка структуры tx и применение к состоянию.

Итого: `pwm-cli` — thin client поверх API `pwmd` и библиотек `pwm-core`.

## Глобальный `--rpc` и `PWM_RPC`

Во всех подкомандах доступен глобальный флаг:
- `--rpc <url>`

Источник значения:
1. `--rpc`, если указан явно;
2. `PWM_RPC`, если флаг не задан;
3. fallback по умолчанию: `http://127.0.0.1:3030`.

Поведение:
- завершающий `/` в URL удаляется перед сборкой endpoint;
- отправка идет в `POST {rpc}/v1/tx`, чтение nonce — `GET {rpc}/v1/account/{from_hex}`.

## Карта команд

## `wallet init`
Назначение: создать wallet-файл user-profile (country label + brute hit).

Ключевые аргументы:
- `--country <label>`
- `--master <hex32>` (optional; если не задан, seed генерируется случайно)
- `--wallet-out <path>`
- `--wallet-passphrase <text>` или env `PWM_WALLET_PASSPHRASE` (по умолчанию wallet шифруется)
- `--plaintext-dev` (явный opt-in для незашифрованного dev-режима)

## `wallet import-seed`
Назначение: как `wallet init`, но с обязательным импортом существующего seed.

Ключевые аргументы:
- `--country <label>`
- `--master <hex32>` (required)
- `--wallet-out <path>`
- `--wallet-passphrase <text>` или env `PWM_WALLET_PASSPHRASE`
- `--plaintext-dev` (явный opt-in)

## `wallet show`
Назначение: прочитать wallet-файл и показать metadata.

Ключевые аргументы:
- `--wallet <path>`
- `--unsafe-show-secrets` (явный unsafe/debug opt-in для вывода `master_seed_hex`/`signing_key_hex`/`verifying_key_hex`)
- `--wallet-passphrase <text>` или env `PWM_WALLET_PASSPHRASE` (нужен только когда включен `--unsafe-show-secrets` для encrypted wallet)

## `wallet backup`
Назначение: создать backup-копию wallet с предварительной проверкой читаемости payload.

Ключевые аргументы:
- `--wallet <path>` (исходный файл)
- `--out <path>` (путь backup-копии)
- `--wallet-passphrase <text>` или env `PWM_WALLET_PASSPHRASE` (обязательно для encrypted wallet; проверяется до копирования)

## `wallet recover`
Назначение: восстановить wallet из backup с предварительной проверкой целостности payload.

Ключевые аргументы:
- `--backup <path>` (источник backup)
- `--out <path>` (путь восстановленного wallet)
- `--wallet-passphrase <text>` или env `PWM_WALLET_PASSPHRASE` (обязательно для encrypted wallet; проверяется до копирования)

## `key-gen`
Назначение: сгенерировать random master seed (32 байта) и вывести hex.

Аргументы: нет.

## `addr-derive`
Назначение: подобрать derivation index для домена и вывести `account_id`, `pubkey`, `derivation_index`.

Ключевые аргументы:
- `--master <hex32>`
- `--domain <hex-u16>`
- `--max-try <u32>` (default `500000`)

## `tx-init`
Назначение: подписать и отправить `TxBody::Init`.

Ключевые аргументы:
- `--wallet <path>` (основной путь подписи)
- `--master <hex32>` (dev-override; при использовании требует `--domain`)
- `--domain <hex-u16|label>` (только вместе с `--master`)
- `--index <u32>` (default `0`)
- `--flags <u32>` (default `0`)

Особенность: nonce фиксированно `0` (инициализация аккаунта).

## `tx-send`
Назначение: подписать и отправить `TxBody::Transfer`.

Ключевые аргументы:
- `--wallet <path>` (основной путь подписи)
- `--master <hex32>` (dev-override; при использовании требует `--domain`)
- `--domain <hex-u16|label>` (только вместе с `--master`)
- `--to <hex32>`
- `--amount <u128>`
- `--fee <u128>` (default `1`)

## `tx-stake`
Назначение: подписать и отправить `TxBody::Stake`.

Ключевые аргументы:
- `--wallet <path>` (основной путь подписи)
- `--master <hex32>` (dev-override; при использовании требует `--domain`)
- `--domain <hex-u16|label>` (только вместе с `--master`)
- `--amount <u128>`

## `tx-unstake`
Назначение: подписать и отправить `TxBody::Unstake`.

Ключевые аргументы:
- `--wallet <path>` (основной путь подписи)
- `--master <hex32>` (dev-override; при использовании требует `--domain`)
- `--domain <hex-u16|label>` (только вместе с `--master`)
- `--amount <u128>`

## `tx-burn-mark`
Назначение: подписать и отправить `TxBody::BurnMark`.

Ключевые аргументы:
- `--wallet <path>` (основной путь подписи)
- `--master <hex32>` (dev-override; при использовании требует `--domain`)
- `--domain <hex-u16|label>` (только вместе с `--master`)
- `--mark-amount <u128>`
- `--beneficiary <hex32>` (optional)

## `off-demo`
Назначение: демо оффчейн-утилит из `pwm-core` (Merkle root + Ed25519 подпись batch), вывод JSON в stdout.

Аргументы: нет.

## Signing + send flow

Для `tx-*` CLI выбирает источник подписи так:
1. По умолчанию используется `--wallet`: чтение wallet YAML и загрузка `signing_key/domain_u16/derivation_index/account_id`.
2. Если передан `--master`, это dev-override: парсинг `master + domain`, затем `brute_cluster_address(seed, domain, 500000)`.
3. Для `tx-send`, `tx-stake`, `tx-unstake`, `tx-burn-mark` запрашивается nonce через `GET /v1/account/{from_hex}`.
4. При неуспешном чтении nonce используется `0`.
5. Формируется `SignedTx::sign_body(...)`.
6. Отправка `POST /v1/tx`.
7. В stdout печатается HTTP status (`204`, `400`, `507`, и т.д.).

Для `tx-init` путь такой же по источнику подписи, но nonce всегда `0`.

## Валидация и допущения по входу

## `hex32`
- ожидается ровно 32 байта в hex;
- используется для `--master`, `--to`, `--beneficiary`;
- при неверном формате команда падает с `expect(...)`.

## `domain`
- парсится как hex `u16`, допускается префикс `0x`;
- при ошибке парсинга команда падает.

## `beneficiary`
- опционален только в `tx-burn-mark`;
- если передан, обязан быть `hex32`;
- при отсутствии сериализуется как `null`/`None`.

## Допущения
- CLI не делает локальный pre-check доменных правил tx;
- окончательная shape/state-валидация выполняется в `pwmd`/`pwm-core`;
- недоступный или частично совместимый RPC endpoint приводит к ошибкам HTTP/панике `expect("http")`.

## Эксплуатационные заметки и частые сбои

- **Неверный RPC URL**: ошибка сети (`http`) или неожиданный статус. Проверить `--rpc`/`PWM_RPC`, доступность `pwmd`.
- **`400 BAD_REQUEST` на `POST /v1/tx`**: tx не прошла `validate_tx_shape` (например, domain mismatch).
- **`507 INSUFFICIENT_STORAGE`**: mempool переполнен, повторить позже.
- **Nonce как `0` при сбое чтения аккаунта**: если `GET /v1/account/:id` неуспешен, CLI подпишет с `nonce=0`; обычно это приведет к reject на ноде.
- **`expect("no match")` в derive**: не найден account для домена в `max_try`; увеличить `--max-try` (для `addr-derive`) и проверить корректность `domain`.
- **Пустая печать только кода статуса**: это норма текущего UX CLI; подробности смотреть в логах `pwmd`.

## Примеры

```bash
# 1) seed
pwm key-gen

# 2) derive адреса домена 0x0A0B
pwm addr-derive --master <seed_hex32> --domain 0x0A0B

# 3) tx-init от wallet (основной путь)
pwm tx-init --wallet ./tmp/wallet-cy.yaml --index 0 --flags 0

# 4) перевод от wallet (основной путь)
pwm tx-send --wallet ./tmp/wallet-cy.yaml --to <to_hex32> --amount 100 --fee 1

# 5) показать секреты wallet (unsafe/debug только по явному флагу)
pwm wallet show --wallet ./tmp/wallet-cy.yaml --unsafe-show-secrets

# 6) dev-override: master + domain
pwm tx-send --wallet ./tmp/wallet-cy.yaml --master <seed_hex32> --domain CY --to <to_hex32> --amount 100 --fee 1
```
