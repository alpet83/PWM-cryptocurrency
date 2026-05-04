# GENESIS_BLOCK: практический гайд для QA

Этот документ показывает рабочую последовательность для тестера:

1. подготовить wallet (с account rows для premine);
2. собрать `genesis` JSON через `pwm genesis-build`;
4. запустить `pwmd` с `--genesis-file`;
5. проверить, что нода стартовала на нужном genesis;
6. быстро диагностировать типовые ошибки.

## 0) Предусловия

- Рабочая директория: корень репозитория `PWM-cryptocurrency`.
- Rust/Cargo установлены.
- Команды ниже выполняются в `PowerShell`.

Проверка доступности бинарей:

```powershell
cargo run -p pwm-cli -- --help
cargo run -p pwmd --bin pwmd -- --help
```

## 1) Подготовить wallet

```powershell
cargo run -p pwm-cli -- wallet init `
  --country CY `
  --wallet-out ./tmp/genesis-wallet.yaml `
  --wallet-passphrase "dev-pass"
```

При необходимости добавьте ещё аккаунты (каждый станет premine row):

```powershell
cargo run -p pwm-cli -- wallet account add `
  --wallet ./tmp/genesis-wallet.yaml `
  --derivation-index 1 `
  --wallet-passphrase "dev-pass"
```

## 2) Сформировать custom genesis JSON

Новый основной путь:

```powershell
cargo run -p pwm-cli -- genesis-build `
  --wallet ./tmp/genesis-wallet.yaml `
  --out ./tmp/genesis-devnet-custom.json `
  --wallet-passphrase "dev-pass" `
  --genesis-passphrase "genesis-pass"
```

`genesis-build` читает `wallet accounts` и генерирует только v4 JSON (`schema_version=4`):
- `gen_cfg.funding.accounts[*]` (hex-строки `acct_hex`, `pubkey_hex`, `der_idx`, `bal`);
- `gen_cfg.validators.set[*]` (producer set для PoA rotation/signature);
- `gen_cfg.reward_policy` (default: `to_producer_account`);
- `validator_keys[*].enc_seed` (encrypted seed envelope `kdf + aead`);
- plaintext `validator_seeds_hex` в формате больше не поддерживается.

## Validator key roles (operator guide)

Этот раздел для оператора/пользователя. Цель: не перепутать роли seed, аккаунта в genesis и runtime identity узла.

### 1) Роль validator seed/key

- `validator_keys[i].enc_seed` используется для ключа производителя блоков (block production signing).
- Validator key — это отдельная криптографическая сущность для роли подписи блоков, а не "публичный адрес пользователя" для wallet UX.
- В текущем контракте путь деривации фиксирован: `m/1000000'/1'`.
- Важно: запись пути деривации (`m/...`) может создать ложное UX-ожидание "обычного кошелькового адреса"; в genesis это служебный ключ роли валидатора.
- Это поведение не настраивается: `pwmd` hard-fail при отличии `validator_keys[*].derivation_path`.

### 2) Роль строки funding в `gen_cfg.funding.accounts`

- `gen_cfg.funding.accounts[i]` - это on-chain запись стартового состояния (`acct`, `pubkey`, `der_idx`, `bal`).
- `acct` в этой строке - владелец стартового баланса (`bal`) в genesis-state.
- `gen_cfg.validators.set[i]` - это producer identity для PoA rotation/signature.
- На старте `pwmd` проверяет согласованность `validator_keys[i]` и `gen_cfg.validators.set[i]` (derived `pubkey/acct` должны совпасть с validator row).
- Для `reward_policy.mode=to_producer_account` действует инвариант: каждый `gen_cfg.validators.set[*].acct` обязан существовать в `gen_cfg.funding.accounts[*].acct`. Если нет — старт прерывается с явной ошибкой (silent reward-loss запрещён).

### 3) Premine ownership: кто реально может тратить

- Premine принадлежит конкретному аккаунту из `gen_cfg.funding.accounts[*].acct`, а не "роли валидатора" как абстракции.
- Трата идет по обычным правилам аккаунта/tx (корректная подпись соответствующим ключом аккаунта + корректный `nonce` + валидная tx-форма).
- Если seed/ключ не соответствует аккаунтной записи в state, средствами этого premine распоряжаться нельзя.

### 4) Runtime identity/domain-cluster policy vs key material

- `--network-id`, `--domain-hi`, `--cluster-id`, `--node-id` задают runtime identity узла (сетевой/операционный контур).
- Эти параметры влияют на local policy/маршрутизацию tx и сетевое поведение узла.
- Они не заменяют и не "переопределяют" key material из genesis (`validator_keys` + `gen_cfg.validators.set`).

### 5) Практический pre-launch checklist (devnet/testnet)

Перед запуском зафиксируйте следующие проверки:

1. Убедитесь, что понимаете разделение ролей:
   - validator seed/key = подпись блоков;
   - `gen_cfg.funding.accounts[*].acct` = владелец genesis-баланса;
   - runtime identity = сетевой/операционный профиль узла.
2. Проверьте, что `validator_keys` и `gen_cfg.validators.set` имеют одинаковую длину и порядок элементов.
3. Для каждого `i` проверьте, что row согласован с derive от seed:
   - v4: путь `m/1000000'/1'`.
4. Проверьте reward-инвариант: каждый `validators.set[*].acct` присутствует в `funding.accounts[*].acct`.
5. Не делайте предположение "любой address из `addr-derive` подходит как validator row" без явной проверки `pubkey/acct`.
6. Для чистого smoke запускайте с отдельным `--state-root` и явным `--data-file`, чтобы не подхватить старый snapshot.
7. После старта проверяйте `/v1/status`, `/v1/head` и `/v1/account/<genesis_account>` на ожидаемое состояние.

Старый PowerShell helper `docs/genesis_bundle_from_seed.ps1` оставлен только как legacy fallback (см. пометку deprecate в самом файле).

### Где хранить genesis-файл

Для QA удобно хранить в:

- `./tmp/genesis-devnet-custom.json` (локальные эксперименты, не коммитить), или
- `./docs/examples/genesis-*.json` (если нужен эталонный фиксированный сценарий).

## 3) Запуск `pwmd` с custom genesis

### Devnet (локально)

```powershell
cargo run -p pwmd --bin pwmd -- `
  --listen 127.0.0.1:3030 `
  --state-root ./tmp/state-devnet `
  --data-file ./tmp/state-devnet/pwm-data.json `
  --genesis-file ./tmp/genesis-devnet-custom.json `
  --genesis-passphrase "genesis-pass"
```

> В текущем `pwmd` нельзя задавать только `--network-id` без `--domain-hi`, `--cluster-id`, `--node-id`: это считается partial identity config и процесс завершится с ошибкой.

### Testnet-профиль (тот же genesis, другой runtime identity)

```powershell
cargo run -p pwmd --bin pwmd -- `
  --listen 127.0.0.1:3040 `
  --state-root ./tmp/state-testnet `
  --data-file ./tmp/state-testnet/pwm-data.json `
  --genesis-file ./tmp/genesis-devnet-custom.json `
  --genesis-passphrase "genesis-pass" `
  --network-id testnet-qa `
  --domain-hi 0x11 `
  --cluster-id test-cluster-a `
  --node-id test-node-01
```

## 4) Проверка, что сеть поднялась на нужном genesis

### Минимум (readiness)

Используйте тот же порт, что задан в `--listen` при запуске `pwmd`:
- devnet-пример выше: `3030`;
- testnet-пример выше: `3040`.

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:<listen_port>/v1/status"
Invoke-RestMethod -Uri "http://127.0.0.1:<listen_port>/v1/head"
```

Ожидание:

- `/v1/status`: `ready = true`, `phase = "ready"`;
- `/v1/head`: возвращает текущую высоту и tip.

### Проверка привязки к вашему genesis

В текущем API нет прямого поля `genesis_hash` в `/v1/status`, поэтому используйте проверку через genesis-аккаунт:

1. Возьмите `account_id_hex` аккаунта из вашего `gen_cfg.funding.accounts[*].acct` (32-byte hex).
2. Запросите:

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:<listen_port>/v1/account/<account_id_hex>"
```

Ожидание:

- аккаунт существует;
- `initialized = true`;
- `balance_pwm` не меньше `bal` из genesis-строки (баланс может быть выше из-за награды за уже произведенные блоки);
- при clean state (`--state-root` в новом каталоге) состояние соответствует вашему файлу, а не старому снапшоту.

## 5) Troubleshooting (типовые ошибки)

### `validator_keys length must match gen_cfg.validators.set`

Причина: количество `validator_keys` не равно числу строк валидаторов.

Что делать: выровнять длины массивов `validator_keys` и `gen_cfg.validators.set`.

### `derived pubkey does not match gen_cfg.validators.set[i].pubkey`

Причина: `pubkey` в `rows` не соответствует ключу, который `pwmd` выводит из расшифрованного `validator_keys[i].enc_seed`.

Что делать:

- пересобрать `pubkey/acct` для этой seed;
- проверить, что вы используете корректную пару `row` + `validator_keys[i]`.

### `derived account id does not match gen_cfg.validators.set[i].acct`

Причина: `acct` не согласован с `pubkey` и `der_idx` строки.

Что делать: пересчитать `acct` из фактических `pubkey + der_idx` для валидатора.

### Нода стартует, но "не тот" state

Причина: подхватился старый snapshot/data-файл.

Что делать:

- запускать с отдельным `--state-root`;
- явно задавать `--data-file`;
- для чистой проверки удалить старую тестовую папку состояния перед запуском.

### `partial identity configuration is not allowed`

Причина: задан только часть identity-флагов (например только `--network-id`).

Что делать:

- либо запускать без identity-флагов (neutral/dev smoke);
- либо задавать полный набор: `--network-id` + `--domain-hi` + `--cluster-id` + `--node-id`.

## 6) Рекомендованный smoke-чеклист для QA

1. Подготовили wallet для genesis (минимум один account).
2. Сгенерировали `genesis` JSON через `pwm genesis-build`.
4. Запустили `pwmd` с `--genesis-file` и отдельным `--state-root`.
5. Проверили `/v1/status`, `/v1/head`, `/v1/account/<genesis_account>`.
6. Зафиксировали артефакты: wallet (тестовый контур), genesis JSON, лог старта и ответы API.
