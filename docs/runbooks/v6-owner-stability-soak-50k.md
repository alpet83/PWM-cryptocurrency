# Runbook: V6 owner stability soak (≥50k blocks)

**Audience:** operator (владелец)  
**Ticket:** `tasks/20260603-v6-prepublication-umbrella.json` (phase `v6-prepub-stability-50k`)  
**Prerequisite:** V6-11 sprint closeout PASS (`tasks/20260615-v6-sprint11-closeout.json`)

Короткий CY soak (V6-10) проверяет сценарии на lab genesis. Этот прогон — **длительная стабильность** живого кластера до публикации MVP v6.

---

## Цели

| # | Критерий |
|---|----------|
| 1 | Высота цепи **≥ 50_000** блоков без потери кворума |
| 2 | Proposer rotation / failover без зависаний (RFC16) |
| 3 | Epoch boundaries + stake admission отрабатывают на длинной цепи |
| 4 | **Address flags** spot-check в процессе: `COSIGN_NON_DISABLEABLE`, `CONSERVATION` delay, emergency `activation_target` |
| 5 | Логи: нет ERROR-шторма; snapshot/head согласованы между peers |

---

## Подготовка

1. Бинарь и genesis согласованы с runtime `main` (post `d251fb5`).
2. CY launchers: proposer + attester(s), профиль как V6-10 soak (или production-like devnet).
3. **Cold start:** trust-load должен завершаться за секунды (не 15–20 мин) — в логе `snapshot startup load ok` с малым `validate_ms`; см. [guide-node-storage-and-snapshot.md](../guide-node-storage-and-snapshot.md) §Design alignment (`20260619` fastpath).
4. Стартовый отчёт: `tmp/v6-stability-50k-<UTC>_start.md` (head, peers, genesis hash).
5. Подготовить тестовые кошельки (см. § **Тестовые адреса с флагами** ниже) **до** spot-check или сразу после старта кластера.

---

## Тестовые адреса с флагами (`addr-bruteforce`)

Флаги кодируются в байтах адреса (`id[2..5]`, ADR 0006). Runtime читает их через `address_flags()` в `pwm-core` — **не** через поле аккаунта.

| Бит | Значение `expected-flags` | Константа | Для какого spot-check |
|-----|---------------------------|-----------|------------------------|
| 0 | `1` | `COSIGN_NON_DISABLEABLE` | `tx-policy-set` / deactivate cosign → reject |
| 1 | `2` | `CONSERVATION` | исходящий `Transfer` → pending queue |
| 0+1 | `3` | оба | комбинированный сценарий (опционально) |
| — | `0` | без флагов | rescue, funding peer, emergency victim без conservation |

**Маска:** для ускорения перебора задавайте **узкую** `--flags-mask` (только нужные биты), а не дефолт `1023`:

```text
(flags & flags_mask) == expected_flags
```

Ожидаемое число попыток (домен CY, high-byte mode): ~`2^(8 + popcount(mask))` — при `mask=1` или `mask=2` обычно хватает `--max-try 500000`; при `mask=3` — до `1000000`.

### Общие параметры

| Параметр | Рекомендация |
|----------|----------------|
| `--domain CY` | regulatory label кластера (как V6-10 soak) |
| `--max-try` | `500000` (1 бит) / `1000000` (2 бита или `mask=1023`); это бюджет попыток от resume-start, не абсолютный ceiling индекса |
| `--overwrite-wallet` | при первом создании файла кошелька |
| `--rpc` | **`offline`** — формальный offline-режим brute **без** auto-`tx-init`; затем ручной `tx-init` на живой `3030`. Либо сразу `--rpc http://127.0.0.1:3030`, если кластер уже стабилен и нужен auto-init |
| master seed | **разный** на каждый кошелёк (последний байт `01`…`05`), чтобы не пересекаться в одном YAML |

Детерминированный master для кошелька `k` (1…255): 31 нулевой байт + `k` в последнем:

```text
0000000000000000000000000000000000000000000000000000000000000001  ; k=1
0000000000000000000000000000000000000000000000000000000000000002  ; k=2
```

Справка по CLI: [pwm-cli.md](../pwm-cli.md) §`addr-bruteforce`.

### Команды (Git Bash / Linux, из корня репо)

**Базовый адрес (flags=0)** — funding, emergency victim, обычный peer:

```bash
cargo run -p pwm-cli --bin pwm -- \
  --rpc offline \
  addr-bruteforce \
  --master 0000000000000000000000000000000000000000000000000000000000000001 \
  --domain CY \
  --flags-mask 3 --expected-flags 0 \
  --max-try 1000000 \
  --wallet-out tmp/v6-soak-wallet-baseline.yaml \
  --overwrite-wallet
```

> `mask=3` фиксирует биты 0 и 1 в ноль (без cosign-nd и без conservation). Строго все 10 phase1-битов: `--flags-mask 1023 --expected-flags 0` (ещё медленнее).

**`COSIGN_NON_DISABLEABLE` (bit 0):**

```bash
cargo run -p pwm-cli --bin pwm -- \
  --rpc offline \
  addr-bruteforce \
  --master 0000000000000000000000000000000000000000000000000000000000000002 \
  --domain CY \
  --flags-mask 1 --expected-flags 1 \
  --max-try 500000 \
  --wallet-out tmp/v6-soak-wallet-cosign-nd.yaml \
  --overwrite-wallet
```

**`CONSERVATION` (bit 1):**

```bash
cargo run -p pwm-cli --bin pwm -- \
  --rpc offline \
  addr-bruteforce \
  --master 0000000000000000000000000000000000000000000000000000000000000003 \
  --domain CY \
  --flags-mask 2 --expected-flags 2 \
  --max-try 500000 \
  --wallet-out tmp/v6-soak-wallet-conservation.yaml \
  --overwrite-wallet
```

**Оба флага (bit 0+1, опционально):**

```bash
cargo run -p pwm-cli --bin pwm -- \
  --rpc offline \
  addr-bruteforce \
  --master 0000000000000000000000000000000000000000000000000000000000000004 \
  --domain CY \
  --flags-mask 3 --expected-flags 3 \
  --max-try 1000000 \
  --wallet-out tmp/v6-soak-wallet-both-flags.yaml \
  --overwrite-wallet
```

**Rescue (flags=0, отдельный master `k=5`):**

```bash
cargo run -p pwm-cli --bin pwm -- \
  --rpc offline \
  addr-bruteforce \
  --master 0000000000000000000000000000000000000000000000000000000000000005 \
  --domain CY \
  --flags-mask 3 --expected-flags 0 \
  --max-try 1000000 \
  --wallet-out tmp/v6-soak-wallet-rescue.yaml \
  --overwrite-wallet
```

### PowerShell (как в `cy_cluster_policy_matrix_e2e.ps1`)

```powershell
$deadRpc = 'offline'
$k = 3
$cm = ('0' * 62) + $k.ToString('x2')
cargo run -p pwm-cli --bin pwm -- --rpc $deadRpc addr-bruteforce `
  --master $cm --domain CY --max-try 500000 `
  --flags-mask 2 --expected-flags 2 `
  --wallet-out "tmp/v6-soak-wallet-conservation.yaml" --overwrite-wallet
```

### После brute: `tx-init` и проверка флагов

Если brute шёл с `--rpc offline`, инициализируйте на живом proposer (`3030`):

```bash
# conservation-кошелёк (пример)
cargo run -p pwm-cli --bin pwm -- \
  --rpc http://127.0.0.1:3030 \
  tx-init --wallet tmp/v6-soak-wallet-conservation.yaml --upgrade-wallet

# emergency victim: init + rescue + dormant emergency policy (см. V6-10 s4)
cargo run -p pwm-cli --bin pwm -- \
  --rpc http://127.0.0.1:3030 \
  tx-init --wallet tmp/v6-soak-wallet-baseline.yaml --upgrade-wallet \
  --rescue-address <RESCUE_ID_PRETTY_OR_BECH32> \
  --initial-policy routing.emergency_redirect:dormant \
  --owner-kind individual --owner-name soak --owner-country CY \
  --metadata-commitment 0000000000000000000000000000000000000000000000000000000000000000
```

Проверка декодированных флагов после init:

```bash
cargo run -p pwm-cli --bin pwm -- account-info --wallet tmp/v6-soak-wallet-conservation.yaml
# или GET /v1/account/<hex> — flags должны совпадать с expected-flags brute
```

**Funding:** premine из genesis или `tx-send` с уже инициализированного аккаунта; для conservation-теста в genesis должен быть разумный `conservation_delay_blocks` (короткий для lab — как в V6-10 s3).

---

## Emergency routing: полная последовательность CLI

Сценарий soak: **CONSERVATION** (`flags=2`) + **rescue** + **dormant** `routing.emergency_redirect` → (опционально) отложенный `Transfer` → **активация** сохранённой `ActivatePolicy` (отмена pending, эвакуация `balance_pwm` на rescue).

Primary path: **один wallet v3 файл** (например, `tmp/demo-genesis-wallet.yaml`) с несколькими `accounts[]`; выбор owner/rescue делается через `--index` и `--rescue-account-index`.  
Split wallet-файлы (`victim.yaml` + `rescue.yaml`) остаются только как optional fallback.

**Предусловия**

| # | Условие |
|---|---------|
| 1 | CY proposer на `http://127.0.0.1:3030`, head растёт |
| 2 | Бинарь post-V6-7 (`activation_target`, prepared activation) |
| 3 | Funding-аккаунт уже `tx-init` (premine из genesis wallet) |
| 4 | Для lab: короткий `conservation_delay_blocks` в genesis **или** ожидание полного delay (~86400 блоков по дефолту) |

Удобные переменные (Git Bash, из корня репо):

```bash
export RPC=http://127.0.0.1:3030
export PWM='cargo run -q -p pwm-cli --bin pwm --'
export META0=0000000000000000000000000000000000000000000000000000000000000000
export ACT_JSON=tmp/v6-soak-emergency-act.json
export WAL=tmp/demo-genesis-wallet.yaml
export VICTIM_IDX=<victim_derivation_index_from_wallet>
export RESCUE_IDX=<rescue_derivation_index_from_wallet>
```

### Шаг 1 — rescue-аккаунт (`flags=0`) и victim (`flags=2`) в одном wallet

Если адреса уже есть в `accounts[]`, шаг пропустить. Иначе добавьте/импортируйте нужные индексы в тот же wallet.

```bash
$PWM --rpc offline addr-bruteforce \
  --master 0000000000000000000000000000000000000000000000000000000000000003 \
  --domain CY --flags-mask 2 --expected-flags 2 \
  --max-try 500000 \
  --wallet-out "$WAL" --overwrite-wallet

$PWM --rpc offline addr-bruteforce \
  --master 0000000000000000000000000000000000000000000000000000000000000005 \
  --domain CY --flags-mask 3 --expected-flags 0 \
  --max-try 1000000 \
  --wallet-out "$WAL"
```

Запишите из вывода brute `derivation_index` для victim/rescue (или возьмите из `wallet account list --wallet "$WAL"`).

### Шаг 2 — `tx-init` rescue на живом RPC

```bash
$PWM --rpc "$RPC" tx-init \
  --wallet "$WAL" --upgrade-wallet \
  --index "$RESCUE_IDX" --flags 0
```

Получите `id_pretty` / hex rescue:

```bash
$PWM account-info --wallet "$WAL"
# export RESCUE='<id_pretty из вывода>'
```

### Шаг 3 — `tx-init` victim: rescue + dormant emergency + сохранённая activation

Подставьте `--index` victim из wallet, `flags=2`:

```bash
$PWM --rpc "$RPC" tx-init \
  --wallet "$WAL" --upgrade-wallet \
  --index "$VICTIM_IDX" --flags 2 \
  --rescue-address "$RESCUE" \
  --initial-policy routing.emergency_redirect:dormant \
  --owner-kind individual --owner-name soak-victim --owner-country CY \
  --metadata-commitment "$META0" \
  --save-activation-tx "$ACT_JSON"
```

Ожидаемый stderr: `tx-init prepared activation: policy=routing.emergency_redirect target=...`

**Cosign в prepared JSON:** при one-wallet path (`$WAL`) `tx-init` добавляет rescue cosign автоматически, **только если** `--rescue-address` указывает на account, который **уже есть** в `accounts[]` того же wallet (поиск по `id_hex`). Если rescue добавили в wallet **после** `tx-init`, или `$ACT_JSON` собран до появления rescue в YAML — в файле **нет** cosign → при submit будет `E_POLICY_EMERGENCY_COSIGN_REQUIRED` (см. §Rescue cosign ниже).

### Rescue cosign — обязательное условие (V6-7)

Активация `routing.emergency_redirect` — **2-of-2**: подпись **владельца victim** + **rescue cosign** (`CosignRole::Rescue`). Это не опция CLI, а preflight pwmd (RFC 6 / ADR 0011).

| Кто подписывает | Откуда в CLI |
|-----------------|--------------|
| Owner (victim) | `--wallet "$WAL"` + `--index "$VICTIM_IDX"` |
| Rescue | **`--rescue-account-index "$RESCUE_IDX"`** (тот же `$WAL`) **или** `--rescue-wallet` + index |

**Без** `--rescue-account-index` / `--rescue-wallet` live-команда шлёт tx только с owner-подписью → отказ:

```text
HTTP 400 … code=E_POLICY_EMERGENCY_COSIGN_REQUIRED
msg=… policy emergency cosign required
```

Это **не** `bad nonce` и **не** conservation delay — просто забыли вторую подпись.

#### Предусловия rescue (on-chain)

| # | Проверка | Как |
|---|----------|-----|
| 1 | Rescue **инициализирован** (`tx-init` шаг 2) | `account-info` / RPC: `initialized=true` |
| 2 | Victim `rescue_address` == цель эвакуации | `account-info` victim; совпадает с `$RESCUE` |
| 3 | `--activation-target` == тот же id, что `rescue_address` | Обычно `"$RESCUE"` (pretty или hex) |
| 4 | Rescue pubkey в wallet совпадает с on-chain | тот же `--index "$RESCUE_IDX"` при cosign |

#### Типичные ошибки активации

| Симптом | Причина | Действие |
|---------|---------|----------|
| `E_POLICY_EMERGENCY_COSIGN_REQUIRED` | Нет rescue cosign в tx | Добавить `--rescue-account-index "$RESCUE_IDX"` (live 7b) или пересобрать `$ACT_JSON` |
| `E_POLICY_EMERGENCY_COSIGN_REQUIRED` | Rescue не `tx-init` on-chain | Шаг 2, дождаться блока |
| `E_POLICY_ACTIVATION_TARGET_REQUIRED` / mismatch | Нет или неверный `--activation-target` | `--activation-target "$RESCUE"` |
| `E_POLICY_MISSING_COSIGN` | Cosign есть, но не тот ключ | Проверить `$RESCUE_IDX` vs фактический rescue account |
| `HTTP 409 bad nonce` | Устарел prepared JSON после stake/send | Live 7b с `--index` (не 7a) |

#### Проверка prepared JSON (`$ACT_JSON`) перед 7a

```bash
# Должен быть непустой массив cosigns с role rescue (поле в JSON зависит от сериализации SignedTx)
python -c "import json; t=json.load(open('$ACT_JSON')); print('cosigns', len(t.get('cosigns',[])))"
```

- `cosigns == 0` → **не использовать 7a**; только live **7b** с `--rescue-account-index`.
- После любого **исходящего** tx victim (stake, burn, …) → nonce сдвинулся → снова **7b**, не 7a.

#### Эталонная live-команда (one wallet)

```bash
$PWM --rpc "$RPC" tx-policy-activate \
  --wallet "$WAL" --upgrade-wallet \
  --index "$VICTIM_IDX" \
  --policy routing.emergency_redirect \
  --fee 0 \
  --activation-target "$RESCUE" \
  --rescue-account-index "$RESCUE_IDX"
```

`--rescue-account-index` **обязателен** для emergency; `--activation-target` должен совпадать с `rescue_address` victim on-chain.

### Шаг 4 — проверка on-chain

```bash
$PWM account-info --wallet "$WAL"
```

Ожидание: у victim `flags` (из id) == 2, `rescue_address` совпадает с RESCUE, политика `routing.emergency_redirect` в dormant до шага 8.

### Шаг 5 — funding victim

С premine / baseline wallet (уже инициализирован):

```bash
$PWM --rpc "$RPC" tx-send \
  --wallet tmp/demo-genesis-wallet.yaml --upgrade-wallet \
  --to "<victim_id_pretty>" \
  --amount 1000000000000 --fee 1
```

(Подставьте `--to` вручную: pretty id victim; amount в raw units.)

### Шаг 6 — (опционально) отложенный CONSERVATION transfer

Проверяет отмену pending при emergency (шаг 8):

```bash
$PWM --rpc "$RPC" tx-send \
  --wallet "$WAL" --upgrade-wallet \
  --index "$VICTIM_IDX" \
  --to "$RESCUE" \
  --amount 100 --fee 1
```

Ожидание: баланс victim **не** уменьшается сразу; в state/RPC появляется `pending_conservation` до `execute_at_height`. Nonce victim **не** увеличивается (остаётся `1` после init) — совпадает с nonce в prepared activation.

### Шаг 7a — активация **сохранённой** политики (`--activation-tx`) — только если cosign уже в файле

Использовать **только когда** в `$ACT_JSON` есть rescue cosign **и** nonce victim не менялся с момента сохранения (нет stake/send после init):

```bash
$PWM --rpc "$RPC" tx-policy-activate --activation-tx "$ACT_JSON"
```

Если `HTTP 409 bad nonce`, prepared tx устарел → **шаг 7b**.

Если `E_POLICY_EMERGENCY_COSIGN_REQUIRED` → в JSON нет rescue cosign → **шаг 7b** с `--rescue-account-index` (не повторять 7a).

### Шаг 7b — live activation (**рекомендуется**; обязателен после stake / без cosign в JSON)

Собрать tx на лету: owner = victim (`--index`), **плюс** rescue cosign (`--rescue-account-index`). Без второго флага получите `E_POLICY_EMERGENCY_COSIGN_REQUIRED`.

```bash
$PWM --rpc "$RPC" tx-policy-activate \
  --wallet "$WAL" --upgrade-wallet \
  --index "$VICTIM_IDX" \
  --policy routing.emergency_redirect \
  --fee 0 \
  --activation-target "$RESCUE" \
  --rescue-account-index "$RESCUE_IDX"
```

> **Чеклист перед Enter:** `$RESCUE_IDX` ≠ `$VICTIM_IDX`; rescue `tx-init` в блоке; `--activation-target` = on-chain `rescue_address`; оба индекса из `wallet account list --wallet "$WAL"`.

Split-wallet optional fallback:

```bash
$PWM --rpc "$RPC" tx-policy-activate \
  --wallet tmp/v6-soak-wallet-conservation.yaml --upgrade-wallet \
  --index 0 \
  --policy routing.emergency_redirect --fee 0 \
  --activation-target "$RESCUE" \
  --rescue-wallet tmp/v6-soak-wallet-rescue.yaml \
  --rescue-account-index 0
```

Если между init и активацией прошли другие исходящие tx с тем же nonce, prepared JSON из шага 3 устаревает — используйте live path **7b**.

### Шаг 8 — oracle PASS

| Проверка | Ожидание |
|----------|----------|
| Victim `balance_pwm` | `0` (эвакуировано) |
| Victim `staked` / marks | **V6:** остаются on-chain ([ADR 0011](../adr/0011-policy-activation-target.md) non-goal). **V7:** stake эвакуируется на rescue ([ADR 0012](../adr/0012-emergency-stake-evacuation.md)); marks не переносятся |
| Rescue `balance_pwm` | ≥ funding − fees (liquid portion; **V6:** без victim stake; **V7:** включая evacuated stake) |
| `pending_conservation` | пусто для victim |
| Emergency policy | active, account `finalized` |
| Activation tx | `fee=0` |

После `finalized` victim **не может** `Unstake` / `Transfer` / обычные policy-tx. **V6:** stake на victim заблокирован до [ADR 0012](../adr/0012-emergency-stake-evacuation.md) (V7-3). **Порядок soak на V6:** activate **до** stake; stake на rescue после emergency. **V7:** activate после stake допустим — oracle включает stake на rescue.

```bash
$PWM account-info --wallet "$WAL"
curl -s "$RPC/v1/head" | jq .
# при доступности: pending conservation в state dump / pwm-tui
```

**Порядок vs delay:** activation — обычная tx в mempool/seal, **без** ожидания `conservation_delay_blocks`. Достаточно включить tx в блок **до** `execute_at_height` pending transfer; на высоте исполнения сначала apply tx в блоке, затем `drain_conservation` (см. ADR 0009).

### Привязка кошельков к spot-check

| Spot-check | Кошелёк | Дальнейшие шаги |
|------------|---------|-----------------|
| Cosign non-disableable | `tmp/v6-soak-wallet-cosign-nd.yaml` | `tx-policy-set --policy cosign_required:dormant` → activate → попытка deactivate/weaken → `E_POLICY_FLAG_NON_DISABLEABLE` |
| Conservation delay | `tmp/v6-soak-wallet-conservation.yaml` | fund → `tx-send` → pending до `execute_at_height` → drain на seal |
| Emergency sweep | один `$WAL` + индексы | § **Emergency routing**; **7b** с `--index` + **`--rescue-account-index`** (cosign обязателен) |
| Mode B (опц.) | baseline flags=0 | `tx-export` + wait `unlock_height` (короткий timeout в genesis) |

---

## Наращивание высоты

- Предпочтительно: естественный seal rate кластера.
- Мониторинг каждые ~5k блоков: `/v1/head` на всех узлах (delta ≤ 1), proposer identity, disk/CPU.

**Checkpoint heights (минимум):** 10k, 25k, 50k.

---

## Spot-check флагов

Кошельки — § **Тестовые адреса с флагами** выше. До и после середины прогона:

| Сценарий | Ожидание |
|----------|----------|
| `COSIGN_NON_DISABLEABLE` | Ослабление cosign → `E_POLICY_FLAG_NON_DISABLEABLE` |
| `CONSERVATION` | Transfer pending до `unlock_height`; credit после drain |
| Emergency activation | fee=0, `activation_target == rescue`, spendable эвакуирован |
| Mode B (опц.) | EXPORT refund на `unlock_height` |

Сценарии: [v6-cy-cluster-precloseout-soak.md](v6-cy-cluster-precloseout-soak.md).

---

## PASS / FAIL

**PASS:** `tmp/v6-stability-50k-<UTC>_closeout.md` — `head_height ≥ 50000`, checkpoints, флаги проверены, ERROR=0 или объяснены.

**FAIL:** потеря кворума, state_root drift, crash loop.

---

## После soak

1. Umbrella phase `v6-prepub-stability-50k` → done.
2. Rust code audit + docs refresh (см. umbrella ticket).
3. Mirror publish — только после owner sign-off на весь pre-publication umbrella.
