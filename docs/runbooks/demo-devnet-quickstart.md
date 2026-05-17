# Demo Devnet Quickstart (MVP V3 Sprint 3)

Этот runbook описывает reproducible public-devnet demo path для внешнего тестера:
clean clone -> генерация demo genesis -> проверка premine 21B PWM -> запуск 3 нод -> API smoke.

## Важно: demo/devnet, не production

- Это только demo/devnet контур.
- Passphrase/keys в примерах предназначены для локального теста и не являются production-grade security posture.
- Не публикуйте секреты и не используйте demo-passphrase для реальных средств.

## 1) Prerequisites

- Windows PowerShell 5.1+ или PowerShell 7+.
- Установлен Rust/Cargo.
- Репозиторий открыт в корне `PWM-cryptocurrency`.
- Для Windows build рекомендуется isolated target:

```powershell
$env:CARGO_TARGET_DIR="F:\pwm-test\PWM-cryptocurrency"
```

## 2) Premine math и raw scale

Канонический scale:

- `PWM_RAW_SCALE = 1_000_000`
- `1 PWM = 1_000_000 raw`

Whitepaper target:

- `21,000,000,000 PWM`
- raw target: `21_000_000_000 * 1_000_000 = 21_000_000_000_000_000`

Именно эта raw-сумма проверяется скриптом в шаге 4.

## 3) One-command / near-one-command: build demo genesis

Из корня репозитория:

```powershell
./scripts/demo-devnet-start.ps1
```

Что делает команда:

1. Генерирует wallet (если отсутствует): по умолчанию использует **детерминированный публичный demo profile** (`--master 000...001` + `--derivation-index 287292`), чтобы clean-clone path был быстрым и повторяемым.
2. Строит `tmp/genesis-custom.json` через `pwm genesis-build` с `--premine-bal 21000000000000000`.
3. Запускает проверку premine (`scripts/demo-genesis-verify.ps1`).
4. Печатает команды запуска proposer/attester/follower.

Параметры (опционально):

- `-GenesisPath` (по умолчанию `tmp/genesis-custom.json`)
- `-WalletPath` (по умолчанию `tmp/demo-genesis-wallet.yaml`)
- `-GenesisPassphrase` / `-WalletPassphrase` (если wallet-passphrase не задан, `demo-genesis-build` вызывает `wallet init` с `--plaintext-dev` — только для локального demo)
- `-PremineRaw` (по умолчанию `21000000000000000`)
- `-DerivationIndex` в `demo-genesis-build.ps1`: по умолчанию `-1` (использовать demo seed/index). Задайте `>=0`, если нужен явный `m/0/N` и он проходит policy
- `-DemoMaster` / `-DemoDerivationIndex`: override для детерминированного режима (demo-only public material)
- `-UseCountryBruteforce`: включает fallback random brute-force по `--country CY`
- `-MaxTry`: верхняя граница попыток в brute-force режиме (по умолчанию `120000`)
- `-SkipBuild` (если genesis уже готов)

> Demo material note: `DemoMaster` и `GenesisPassphrase=12345` в этом runbook являются публичными dev/demo значениями и не предназначены для production.

## 4) Явная проверка premine (fail-fast)

Можно запускать отдельно:

```powershell
./scripts/demo-genesis-verify.ps1 -GenesisPath "tmp/genesis-custom.json" -ExpectedPremineRaw 21000000000000000
```

Ожидание:

- `Premine verified ...` и код выхода `0`.
- При несовпадении суммы скрипт возвращает non-zero exit.

## 5) Запуск нод (CY cluster scripts)

После шага 3 откройте три терминала и выполните команды, которые выводит `demo-devnet-start.ps1`.

Эквивалент вручную:

```powershell
# Terminal 1
$env:PWM_DEMO_GENESIS_PATH="tmp/genesis-custom.json"
$env:PWM_DEMO_GENESIS_PASSPHRASE="12345"
./cy-cluster-proposer.ps1
```

```powershell
# Terminal 2
$env:PWM_DEMO_GENESIS_PATH="tmp/genesis-custom.json"
$env:PWM_DEMO_GENESIS_PASSPHRASE="12345"
./cy-cluster-attester.ps1
```

```powershell
# Terminal 3
$env:PWM_DEMO_GENESIS_PATH="tmp/genesis-custom.json"
$env:PWM_DEMO_GENESIS_PASSPHRASE="12345"
./cy-cluster-follower.ps1
```

## 6) API smoke (`/v1/*`)

Проверки public stable API описаны в `docs/api-v1.md`.

Минимальный smoke:

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/status"
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/head"
$resp = Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/accounts"
$id = $resp.accounts[0].id
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/account/$id"
```

### 6.1) Опционально: V4 policy (живой конвейер tx → state → GET account)

Интегрированный gate V4 закрывался в основном юнит-тестами и `cargo test`; **отдельного длительного policy soak в план не входило**.

Для операторской проверки политик на демо-стенде используйте скрипт (после шага 3 и запуска хотя бы proposer+attester):

```powershell
./scripts/devnet_v4_policy_e2e.ps1 -CleanState
```

Скрипт выполняет `tx-init`, жизненный цикл `PolicyTx` на обратимой политике (`routing.same_domain_only`), читает поля policy в JSON аккаунта и завершает `tx-policy-deactivate`. Ограничения и сценарии без второго аккаунта — см. шапку скрипта и `docs/reviews/20260517-v4-policy-devnet-e2e-notes.md`.

Отдельно, офлайн-проверка перебора адреса (долго, по умолчанию **до 1 000 000 попыток** под маску phase1):

```powershell
./scripts/devnet_v4_policy_e2e.ps1 -BruteDemoOnly
```

Для **pwm-testing** на хосте Windows предпочтительно запускать эту же команду через **`cq_process_ctl`** (**`spawn` + длинный `wait`**), см. `docs/AGENT_PROMPT_testing.md`.

**Другой операторский путь (рядом, не эквивалент интегрированному unit-gate):** живая **матрица** нескольких политик на **двухнодовом CY-кластере** — см. **`docs/runbooks/cy-cluster-policy-matrix-e2e.md`**, **`scripts/cy_cluster_policy_matrix_e2e.ps1`** и **`tasks/20260517-cy-cluster-policy-matrix-e2e-live.json`**.

## 7) Troubleshooting (коротко)

- `Missing genesis file`: сначала выполните `./scripts/demo-devnet-start.ps1` или `./scripts/demo-genesis-build.ps1`.
- `Premine mismatch`: проверьте `-PremineRaw` и что используется нужный genesis JSON.
- `wallet init no match`: используйте дефолтный детерминированный режим (не задавайте `-UseCountryBruteforce`) или задайте валидный `-DerivationIndex`.
- `address already in use`: освободите порты CY cluster (`3030`, `13030`) на соответствующих loopback IP.
- Если нода подхватывает старый state: удалите соответствующий `tmp/state-cy-*` и перезапустите.
