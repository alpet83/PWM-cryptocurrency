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

## Версии формата wallet-файла

- **v2 (текущая реализация):** один signing-аккаунт на файл; поля `derivation_*`, `account_id_hex`, `account_id_human` на корне YAML. Подробности по полям — [reviews/sprint-14-wallet-schema-audit.md](reviews/sprint-14-wallet-schema-audit.md).
- **v3 (Sprint 14, спецификация):** массив **`accounts[]`** (owned-адреса) и человеко-читаемое поле **`id_pretty`** вместо корневого `account_id_human`; wallet-level `active_account_id_hex` больше не обязателен и не является signing source. Нормативный пример и инварианты — [rfc/10-wallet-file-format-v3.md](rfc/10-wallet-file-format-v3.md). Реализация CLI/TUI идёт по чек-листу [reviews/sprint-14-checklist.md](reviews/sprint-14-checklist.md) с конвейером `pwm-coding` → `pwm-testing` → `pwm-review`.

## Глобальный `--rpc` и `PWM_RPC`

Во всех подкомандах доступен глобальный флаг:
- `--rpc <url>`
- `--genesis-passphrase <text>` (или env `PWM_GENESIS_PASSPHRASE`) для `genesis-build`
- `--upgrade-wallet` (явный opt-in на миграцию wallet schema `v2 -> v3` при чтении wallet-файла)

Источник значения:
1. `--rpc`, если указан явно;
2. `PWM_RPC`, если флаг не задан;
3. fallback по умолчанию: `http://127.0.0.1:3030`.

Специальное значение:
- `--rpc offline` (или `PWM_RPC=offline`) включает явный offline-режим: HTTP-запросы не выполняются; разрешены только локальные команды (`key-gen`, `genesis-build`, `addr-*`, `wallet *`, `off-demo`).

Поведение `--upgrade-wallet`:
- без флага загрузка wallet в read-path не перезаписывает файл;
- с флагом при чтении schema v2 выполняется миграция в schema v3 и запись на диск.

Поведение:
- завершающий `/` в URL удаляется перед сборкой endpoint;
- отправка идет в `POST {rpc}/v1/tx`, чтение nonce — `GET {rpc}/v1/account/{from_hex}`;
- для happy-path cross-domain `tx-send` пользователь указывает только native/source RPC; target peer достигается source `pwmd` через trusted configured seed;
- `tx-send`/`tx-import` по возможности заранее проверяют `GET {rpc}/v1/account/{to_hex}` и блокируют missing/`initialized=false` recipient: same-domain проверяется на текущем source RPC, а manual `tx-import` проверяется на target RPC.

## Карта команд

## `wallet init`
Назначение: создать wallet-файл user-profile.

Выходной wallet для нового создания пишется сразу в schema v3 (совместимость чтения v2/v3 сохраняется через read-path).

Два режима (альтернативы в смысле CLI, см. `--help` у подкоманды):

1. **Brute-force по стране** — задать `--country` и не указывать `--derivation-index` / `--derivation-path`. Подбирается индекс под high-byte домена и policy флагов user-profile.
2. **Явная деривация** — задать `--derivation-index` и/или `--derivation-path` (`m/0/N` только). `--country` не обязателен и **не** используется как фильтр домена. Отклонение только если derived адрес нарушает recipient domain policy (`pwm_core::address_book::validate_recipient_address_policy`: reserve, witness, неизвестный домен).

Ключевые аргументы:
- `--country <label>` (обязателен только без explicit derivation; см. выше)
- `--master <hex32>` (optional; если не задан, seed генерируется случайно)
- `--derivation-index <u32>` (optional; явный derivation index вместо brute-force)
- `--derivation-path <m/0/N>` (optional; эквивалент `--derivation-index`, поддерживается только canonical `m/0/<index>`)
- `--wallet-out <path>` (optional; default `~/.pwm-crypto/default-wallet.yaml`)
- `--wallet-passphrase <text>` или env `PWM_WALLET_PASSPHRASE` (по умолчанию wallet шифруется)
- `--plaintext-dev` (явный opt-in для незашифрованного dev-режима)

Поведение explicit derivation:
- если переданы и `--derivation-index`, и `--derivation-path`, они обязаны указывать один и тот же index;
- в лог CLI для explicit режима печатается `domain_match_mode explicit_recipient_domain_policy` (в brute-force режиме — `high_byte_only`).

## `wallet import-seed`
Назначение: как `wallet init`, но с обязательным импортом существующего seed; те же два режима (country+brute vs explicit derivation).

Выходной wallet для нового создания пишется сразу в schema v3.

Ключевые аргументы:
- `--country <label>` (обязателен только без `--derivation-index` / `--derivation-path`)
- `--master <hex32>` (required)
- `--derivation-index <u32>` (optional; явный derivation index вместо brute-force)
- `--derivation-path <m/0/N>` (optional; эквивалент `--derivation-index`, поддерживается только canonical `m/0/<index>`)
- `--wallet-out <path>` (optional; default `~/.pwm-crypto/default-wallet.yaml`)
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

## `wallet account list|add|use|remove` (schema v3)
Назначение: операторские команды multi-account для wallet schema v3.

- `wallet account list --wallet <path>`: печатает все `accounts[]`; маркер (`*`) показывает детерминированный CLI default (минимальный `derivation_index`, затем минимальный `id_hex`), а не persisted active account.
- `wallet account add --wallet <path> --derivation-index <u32>`: добавляет новый аккаунт из того же master seed; для `mode: encrypted` обязателен `--wallet-passphrase` (или env `PWM_WALLET_PASSPHRASE`), иначе команда завершится с явной ошибкой.
- `wallet account use --wallet <path> --id-hex <hex32>`: deprecated compatibility command; валидирует наличие account id, но не сохраняет wallet-level active marker.
- `wallet account remove --wallet <path> --id-hex <hex32>`: удаляет аккаунт; guardrails:
  - нельзя удалить последний оставшийся аккаунт;
  - после удаления CLI default снова вычисляется детерминированно из оставшихся `accounts[]`.

Ограничение совместимости:
- для schema v2 команды `wallet account *` отклоняются с явной ошибкой, что требуется schema v3 wallet.

## `genesis-build`
Назначение: собрать genesis JSON `schema_version=4` с decoupled funding/validators.

Ключевые аргументы:
- `--wallet <path>`
- `--out <path>`
- `--val-id <account_id>` (optional; если не задан, используется детерминированный wallet default как validator source)
- `--wallet-passphrase <text>` или env `PWM_WALLET_PASSPHRASE` (для чтения encrypted wallet)
- `--genesis-passphrase <text>` или env `PWM_GENESIS_PASSPHRASE` (для шифрования `validator_keys[*].enc_seed`)

Контракт вывода:
- пишется только `schema_version=4`;
- `gen_cfg.funding.accounts` берётся из всех wallet accounts (поддержан кейс `1 validator + N funding rows`);
- `gen_cfg.validators.set` + `validator_keys` по умолчанию содержит 1 validator (детерминированный wallet default или `--val-id`);
- если validator `acct_hex` отсутствует в funding rows, `genesis-build` детерминированно добавляет row с тем же `acct_hex/pubkey_hex/der_idx` и `bal=0`;
- `gen_cfg.reward_policy.mode` по умолчанию `to_producer_account`;
- plaintext `validator_seeds_hex` больше не используется;
- для каждого validator row фиксируется `derivation_path = m/1000000'/1'`.

## `key-gen`
Назначение: сгенерировать random master seed (32 байта) и вывести hex.

Аргументы: нет.

## `addr-derive`
Назначение: подобрать derivation index для домена и вывести `account_id`, `pubkey`, `derivation_index`.

Статус: **soft-deprecated**. Команда сохранена для обратной совместимости, но CLI печатает warning с рекомендацией перейти на `addr-bruteforce`.

Ключевые аргументы:
- `--master <hex32>`
- `--domain <hex-u16>`
- `--max-try <u32>` (default `500000`)
- `--wallet-out <path>` (optional): при явном флаге результат сохраняется в wallet:
  - если wallet уже существует, по умолчанию применяется безопасный add-режим (append нового account);
  - если файла нет, создается новый wallet;
  - родительский каталог создаётся автоматически.
- без `--wallet-out` команда остаётся stateless (wallet файл не пишется), но по совместимости stdout всё ещё содержит `wallet_path` (default path) и `wallet_write_mode stateless`.

Wallet protection при `addr-derive --wallet-out`:
- приоритет passphrase: `--wallet-passphrase` > `PWM_WALLET_PASSPHRASE`;
- если passphrase задан, новый/перезаписываемый wallet сохраняется в `encrypted`;
- если passphrase не задан и создается новый wallet, используется `plaintext_dev` с явным warning;
- при append в существующий wallet предупреждение про plaintext не печатается (запись идет в существующий файл без destructive replace).

## `addr-bruteforce`
Назначение: линейный brute-force для country-label и сохранение wallet YAML.

Ключевые аргументы:
- `--master <hex32>`
- `--domain <label>` (только label из `domain_index`, для user-profile принимаются только country/regulatory labels)
- `--flags-mask <u32>` (optional, default `1023`, policy: только low 10 bits)
- `--expected-flags <u32>` (required, policy: не выходит за `flags-mask`)
- `--max-try <u32>` (default `500000`): **количество попыток** (число проверяемых derivation index), начиная с `resume_start_index`
- `--wallet-out <path>` (optional; default `~/.pwm-crypto/default-wallet.yaml`)
- `--overwrite-wallet` (optional): явный destructive opt-in для обратной совместимости; без него используется безопасный add (append) в существующий wallet.

Resume-поведение:
- если `--wallet-out` уже существует и `--overwrite-wallet` не задан, поиск возобновляется с `max_derivation_index + 1` среди аккаунтов, совместимых с текущим target cluster/domain mode;
- если в wallet нет совместимых аккаунтов, используется fallback `global_max_derivation_index + 1`;
- если файла нет, поиск стартует с `0`.
- если задан `--overwrite-wallet`, resume принудительно отключается и поиск стартует с `0` (fresh start).
- эффективный диапазон поиска в resume-режиме: `start_index = resume_start_index`, `end_index = resume_start_index + max_try - 1` (saturating); при `--max-try 0` brute не выполняет попыток.

Wallet protection для `addr-bruteforce`:
- приоритет passphrase: `--wallet-passphrase` > `PWM_WALLET_PASSPHRASE`;
- если passphrase задан, новый/перезаписываемый wallet сохраняется в `encrypted`;
- если passphrase не задан ни флагом, ни env, новый/перезаписываемый wallet сохраняется в `plaintext_dev` и CLI печатает явный warning;
- при append в существующий wallet warning не печатается (по умолчанию нет destructive replacement).

Post-action UX для `addr-bruteforce`:
- после успешного brute-force и `wallet_out` save CLI автоматически пробует отправить `tx-init` на текущий RPC (`--rpc` / `PWM_RPC`, те же HTTP timeout правила `PWM_CLI_RPC_TIMEOUT_MS`);
- при `--rpc offline` auto-init **явно пропускается** без сетевых попыток; CLI печатает команду для ручного `tx-init`;
- если auto-init успешен, печатается `stderr`-сообщение об успехе;
- если auto-init не удался, результат brute-force не теряется (wallet уже сохранен), печатается диагностика и явная команда для ручного `tx-init` (обязательный шаг перед `tx-send` / `tx-burn-mark`);
- при недоступном RPC (`cannot connect` / timeout) выводится отдельный hint про offline-сценарий и ручную инициализацию адреса.

Терминология вывода:
- в user-visible строках для pretty account id используется ключ `id_pretty` (вместо legacy `account_id_human`).

Интерактивный запуск (без дублирования логики CLI, поверх `pwm addr-bruteforce`):
- Linux/macOS/Git Bash: `bash scripts/addr-bruteforce-interactive.sh`
- Windows launcher: `scripts\addr-bruteforce-interactive.cmd`
- Для sanity без реального brute-force: `bash scripts/addr-bruteforce-interactive.sh --dry-run`

## `tx-init`
Назначение: подписать и отправить `TxBody::Init`.

Ключевые аргументы:
- `--wallet <path>` (основной путь подписи)
- `--master <hex32>` (dev-override; при использовании требует `--domain`)
- `--domain <hex-u16|label>` (только вместе с `--master`)
- `--index <u32>` (default `0`; для `--wallet` это выбор derivation index `m/0/N` в schema v3)
- `--flags <u32>` (default `0`)
- V4 extension (опционально, включается при передаче хотя бы одного V4-флага):
  - `--owner-kind <text>`
  - `--owner-name <text>`
  - `--owner-country <text>`
  - `--metadata-commitment <hex32>`
  - `--verification-ref <text>`
  - `--requested-domain-lo <u8>`
  - `--rescue-address <account>`
  - `--initial-policy <kind[:dormant|immediately]>` (repeatable)

Особенность: nonce берётся из `GET /v1/account/{from_hex}`; при `404 account not found` используется `0`.

## `tx-policy-set`
Назначение: подписать и отправить `TxBody::Policy` с действием `SetPolicy`.

Ключевые аргументы:
- `--wallet <path>` (основной путь подписи)
- `--master <hex32>` + `--domain <hex-u16|label>` (dev-override)
- `--index <u32>` (default `0`; при `--wallet` выбирает signer `m/0/N` в wallet schema v3)
- `--policy <kind>` (`sender_filter`, `routing.emergency_redirect`, `routing.same_domain_only`, `default_behavior`, `cosign_required`)
- `--activation <dormant|immediately>`
- `--fee <u128>` (default `1`)

## `tx-policy-activate`
Назначение: подписать и отправить `TxBody::Policy` с действием `ActivatePolicy`.

Ключевые аргументы:
- `--wallet <path>` (основной путь подписи)
- `--master <hex32>` + `--domain <hex-u16|label>` (dev-override)
- `--index <u32>` (default `0`; при `--wallet` выбирает signer `m/0/N` в wallet schema v3)
- селектор policy: `--policy <kind>` или `--policy-id <u8>` (можно вместе, значения должны совпадать)
- `--fee <u128>` (default `1`)

Emergency rescue cosign (MVP путь):
- same-wallet: `--rescue-account-index <u32>` (берёт rescue-аккаунт из wallet v3 и добавляет `Cosignature { role: rescue }`);
- optional external wallet: `--rescue-wallet <path>` + `--rescue-account-index <u32>`;
- minimal external signer override: `--rescue-master <hex32> --rescue-domain <hex-u16|label>`;
- optional passphrase override: `--rescue-passphrase <text>` (иначе используется глобальный `--wallet-passphrase`/`PWM_WALLET_PASSPHRASE`).

Rescue cosign flags are accepted only when activating `routing.emergency_redirect`; for ordinary policies the CLI rejects them to avoid implying generic governance multisig. When `--rescue-wallet` is used without `--rescue-account-index`, the command falls back to that wallet's default signer, so production operators should pass the index explicitly.  
Для `--activation-tx` (prepared JSON) при reject `HTTP 409 bad nonce` CLI дополнительно печатает hint с `file nonce`, on-chain nonce для `target_account` и рекомендацией перейти на live activation с `--wallet ... --index ...` (+ `--rescue-account-index` для same-wallet rescue).

## `tx-policy-deactivate`
Назначение: подписать и отправить `TxBody::Policy` с действием `DeactivatePolicy`.

Ключевые аргументы:
- `--wallet <path>` (основной путь подписи)
- `--master <hex32>` + `--domain <hex-u16|label>` (dev-override)
- `--index <u32>` (default `0`; при `--wallet` выбирает signer `m/0/N` в wallet schema v3)
- селектор policy: `--policy <kind>` или `--policy-id <u8>`
- `--fee <u128>` (default `1`)

## `tx-send`
Назначение: one-window send через native/source node.
- same-domain: подписывает и отправляет `TxBody::Transfer` (как раньше);
- cross-domain: автоматически подписывает `TxBody::Export`, вызывает `POST /v1/roaming-intents` и печатает lifecycle polling (`queued/exported/relayed/imported/expired/failed`).
- same-domain recipient preflight идёт на текущий `--rpc` / `PWM_RPC` до чтения nonce и submit; missing/неинициализированный получатель блокируется без дебета sender.
- cross-domain `tx-send` в текущем one-window flow не требует target RPC от пользователя: source `pwmd` выбирает target peer по configured trusted seed и `cluster_domain_hi`. CLI всё ещё печатает ограничение: target recipient preflight недоступен из source-only окна, поэтому target import/relay отклонит missing/неинициализированного recipient.

Ключевые аргументы:
- `--wallet <path>` (основной путь подписи)
- `--master <hex32>` (dev-override; при использовании требует `--domain`)
- `--domain <hex-u16|label>` (только вместе с `--master`)
- `--index <u32>` (default `0`; при `--wallet` выбирает signer `m/0/N` в wallet schema v3)
- `--to <address|uri>`: pretty / canonical bech32DX / legacy hex / legacy `PWMv0-hex` / `pwm:<address>?amount=<u128>`
- `--amount <u128>`
- `--fee <u128>` (default `1`)

## `tx-stake`
Назначение: подписать и отправить `TxBody::Stake`.

Ключевые аргументы:
- `--wallet <path>` (основной путь подписи)
- `--master <hex32>` (dev-override; при использовании требует `--domain`)
- `--domain <hex-u16|label>` (только вместе с `--master`)
- `--index <u32>` (default `0`; при `--wallet` выбирает signer `m/0/N` в wallet schema v3)
- `--amount <u128>`

## `tx-unstake`
Назначение: подписать и отправить `TxBody::Unstake`.

Ключевые аргументы:
- `--wallet <path>` (основной путь подписи)
- `--master <hex32>` (dev-override; при использовании требует `--domain`)
- `--domain <hex-u16|label>` (только вместе с `--master`)
- `--index <u32>` (default `0`; при `--wallet` выбирает signer `m/0/N` в wallet schema v3)
- `--amount <u128>`

## `tx-burn-mark`
Назначение: подписать и отправить `TxBody::BurnMark`.

Ключевые аргументы:
- `--wallet <path>` (основной путь подписи)
- `--master <hex32>` (dev-override; при использовании требует `--domain`)
- `--domain <hex-u16|label>` (только вместе с `--master`)
- `--index <u32>` (default `0`; при `--wallet` выбирает signer `m/0/N` в wallet schema v3)
- `--mark-amount <u32>`
- `--beneficiary <hex32>` (optional)

## `tx-export`
Назначение: подписать и отправить `TxBody::Export` (source-shard шаг inter-shard flow).

Ключевые аргументы:
- `--wallet <path>` (основной путь подписи)
- `--master <hex32>` (dev-override; при использовании требует `--domain`)
- `--domain <hex-u16|label>` (только вместе с `--master`)
- `--to <address>` (pretty/canonical/legacy, как в `tx-send`)
- `--target-domain <hex-u16|label>`
- `--amount <u128>`
- `--fee <u128>` (default `1`)

## `tx-import`
Назначение: подписать и отправить `TxBody::Import` (target-shard шаг inter-shard flow).

Ключевые аргументы:
- `--wallet <path>` (основной путь подписи)
- `--master <hex32>` (dev-override; при использовании требует `--domain`)
- `--domain <hex-u16|label>` (только вместе с `--master`)
- `--to <address>` (pretty/canonical/legacy, как в `tx-send`)
- `--amount <u128>` (raw units; scale `1 PWM = 1_000_000 raw`; это не decimal display-значение из TUI)
- `--export-id <hex32>`

Контракт инициализации:
- `--to` — target-recipient на target-shard; recipient обязан заранее выполнить `tx-init` на target. Missing или `initialized=false` recipient отклоняется до credit/imported_set mutation.
- Перед auto-init import-signer CLI проверяет recipient на текущем target RPC; auto-init sender-side не должен маскировать recipient init failure, invalid/unknown `export_id` или mismatch `to/amount/target_domain`.

MVP operator note:
- `tx-export`/`tx-import` сохранены как backward-compatible manual handoff flow (fallback/debug).
- Основной пользовательский путь cross-domain: `tx-send` на native/source RPC через roaming-intent API; target RPC пользователю не нужен для happy path.

## `tx-handoff-register`
Назначение: зарегистрировать на target-node signed handoff, полученный после source finalize, перед выполнением `tx-import`.

Ключевые аргументы:
- `--handoff-json <path>`: JSON, сохранённый из ответа `POST /v1/roaming-intents/:id/finalize` на source-node.

Операторский контракт:
- команда отправляет handoff в `POST /v1/export-provenance` на target RPC;
- target проверяет trusted peer context from configured seed connectivity, подпись source-node, `network_id`, `target_domain`, `to`, `amount`, duplicate/imported state и только после этого регистрирует provenance;
- open/no-seed регистрация не поддерживается: inbound/dev `NodeHello` сам по себе не делает source доверенным provenance peer;
- без этого шага `tx-import` должен завершиться `400 invalid import: export_id is not known`;
- это MVP trust boundary: оператор переносит signed handoff между нодами, но не редактирует поля вручную.

## `off-demo`
Назначение: демо оффчейн-утилит из `pwm-core` (Merkle root + Ed25519 подпись batch), вывод JSON в stdout.

Аргументы: нет.

## Signing + send flow

Для `tx-*` CLI выбирает источник подписи так:
1. По умолчанию используется `--wallet` + `--index`: чтение wallet YAML и выбор signer `m/0/N` (`--index`) из wallet schema v3 accounts (для v2 сохраняется single-account fallback).
2. Если передан `--master`, это dev-override: парсинг `master + domain`, затем `brute_cluster_address(seed, domain, 500000)`.
3. Для `tx-send`, `tx-stake`, `tx-unstake`, `tx-burn-mark`, `tx-export`, `tx-import` запрашивается nonce через `GET /v1/account/{from_hex}`.
4. При неуспешном чтении nonce команда завершается с явной ошибкой (без silent fallback `nonce=0`).
5. Формируется `SignedTx::sign_body(...)`.
6. Отправка:
   - same-domain `tx-send` и прочие tx: `POST /v1/tx`;
   - cross-domain `tx-send`: `POST /v1/roaming-intents` + polling `GET /v1/roaming-intents/:id`.
7. В stdout печатается HTTP status (`204`, `400`, `507`, и т.д.).

Для `tx-init` при `--wallet` signer выбирается по `--index` (`m/0/N` в schema v3); nonce читается из account-view и падает в `0` только для `account not found`.

## Genesis roles note (operator)

- Validator key role и premine spend path - разные контуры.
- CLI-команды для пользовательских tx работают по аккаунтной модели (`signing key + derivation_index + nonce`) и не должны трактоваться как "управление validator key" в genesis.
- Подробно и с guardrails: [GENESIS_BLOCK.md#validator-key-roles-operator-guide](GENESIS_BLOCK.md#validator-key-roles-operator-guide).

## Валидация и допущения по входу

## `hex32`
- ожидается ровно 32 байта в hex;
- используется для `--master`, `--beneficiary` (а также legacy-формы адреса в `--to`);
- при неверном формате команда падает с `expect(...)`.

## `domain`
- парсится как hex `u16`, допускается префикс `0x`;
- при ошибке парсинга команда падает.

## `beneficiary`
- опционален только в `tx-burn-mark`;
- если передан, обязан быть `hex32`;
- при отсутствии сериализуется как `null`/`None`.

## Допущения
- CLI делает локальный pre-check policy для получателя в `tx-send` (`--to`) до отправки в RPC;
- окончательная shape/state-валидация выполняется в `pwmd`/`pwm-core`;
- недоступный или частично совместимый RPC endpoint приводит к ошибкам HTTP/панике `expect("http")`.

## Эксплуатационные заметки и частые сбои

- **Неверный RPC URL**: ошибка сети (`http`) или неожиданный статус. Проверить `--rpc`/`PWM_RPC`, доступность `pwmd`.
- **`400 BAD_REQUEST` на `POST /v1/tx`**: tx не прошла `validate_tx_shape` (например, domain mismatch).
- **Policy rejects (`400/409`)**: CLI выводит структурированный код из reject JSON (например `E_POLICY_NOT_INSTALLED`, `E_POLICY_MISSING_COSIGN`, `E_POLICY_EMERGENCY_COSIGN_REQUIRED`, `E_POLICY_ACCOUNT_FINALIZED`).
- **`400 BAD_REQUEST` на `tx-import`**: invalid/unknown provenance (неизвестный `export_id`, mismatch `to/amount/target_domain`, либо неверная target-нода) или recipient не сделал `tx-init` на target.
- **`507 INSUFFICIENT_STORAGE`**: mempool переполнен, повторить позже.
- **`409 CONFLICT` на `tx-import`**: duplicate import для уже использованного `export_id` (ожидаемый idempotent reject при повторе).
- **`404 account not found` при nonce fetch**: CLI теперь добавляет UX-hint: sender не инициализирован на текущем RPC, сначала сделать `tx-init` для sender на source-node и проверить `--rpc`/`PWM_RPC` (source domain/shard).
- **`addr-bruteforce` + `--rpc offline`**: brute-force и wallet save завершаются без HTTP; auto-init пропускается, CLI печатает явную команду ручного `tx-init` (с `--index`/`--flags` найденного адреса).
- **`expect("no match")` в derive**: не найден account для домена в `max_try`; увеличить `--max-try` (для `addr-derive`) и проверить корректность `domain`.
- **Пустая печать только кода статуса**: это норма текущего UX CLI; подробности смотреть в логах `pwmd`.

## Примеры

```bash
# 1) seed
pwm key-gen

# 2) derive адреса домена 0x0A0B
pwm addr-derive --master <seed_hex32> --domain 0x0A0B

# 2.1) восстановить funded адрес по seed + известному derivation index
pwm wallet import-seed --country CY --master <seed_hex32> --derivation-index 42 --wallet-out ./tmp/wallet-cy-funded.yaml
# эквивалентно:
pwm wallet import-seed --country CY --master <seed_hex32> --derivation-path m/0/42 --wallet-out ./tmp/wallet-cy-funded.yaml

# 3) tx-init от wallet (основной путь)
pwm tx-init --wallet ./tmp/wallet-cy.yaml --index 0 --flags 0

# 4) перевод от wallet (основной путь)
pwm tx-send --wallet ./tmp/wallet-cy.yaml --to <to_hex32> --amount 100 --fee 1

# 4.1) emergency activate в одном wallet v3 (victim + rescue в одном файле)
pwm tx-policy-activate \
  --wallet ./tmp/demo-genesis-wallet.yaml \
  --index <victim_idx> \
  --policy routing.emergency_redirect \
  --fee 0 \
  --activation-target <rescue_hex32> \
  --rescue-account-index <rescue_idx>

# 5) показать секреты wallet (unsafe/debug только по явному флагу)
pwm wallet show --wallet ./tmp/wallet-cy.yaml --unsafe-show-secrets

# 6) dev-override: master + domain
pwm tx-send --wallet ./tmp/wallet-cy.yaml --master <seed_hex32> --domain CY --to <to_hex32> --amount 100 --fee 1
```
