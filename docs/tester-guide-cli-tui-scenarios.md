# Tester guide: CLI/TUI domain-first multi-node

Гайд для оператора demo-команды: как работать с **`pwm`** (CLI) и **`pwm-tui`** в актуальной модели Sprint 11: relay baseline по умолчанию и shard-enforced semantics только при explicit domain-конфиге. Базовый devnet-smoke по одной ноде остаётся в [tester-guide-devnet-smoke.md](./tester-guide-devnet-smoke.md).

## 1) Предпосылки

- Установлены Rust и `cargo`.
- Понимание: primary UX — explicit domain config (`--domain-hi` / `--domain-cluster`, `--cluster-id`, `--node-id`).
- Для двух процессов нужны разные `--listen` и разные `--state-root`.

## 2) Поднять ноду для конкретного домена

```powershell
cargo run -p pwmd --bin pwmd -- `
  --listen 127.0.0.1:3030 `
  --network-id devnet `
  --domain-hi 0x10 `
  --cluster-id local-cluster-a `
  --node-id local-node-a
```

Проверка:

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/status"
```

Ожидание:
- `phase=ready`;
- launch-режим `explicit`;
- state namespace соответствует `domain-hi-0x10`.

## 3) Поднять две ноды с разными `domain_hi` (два терминала)

Быстрый вариант (печатные команды): `tools/demo-two-shard.ps1` (Windows PowerShell) или `tools/demo-two-shard.sh` (bash).

**Терминал A — domain `0x10`, порт 3030:**

```powershell
cargo run -p pwmd --bin pwmd -- `
  --listen 127.0.0.1:3030 `
  --state-root state-a `
  --network-id devnet `
  --domain-hi 0x10 `
  --cluster-id local-cluster-a `
  --node-id local-node-a
```

**Терминал B — domain `0x20`, порт 3031:**

```powershell
cargo run -p pwmd --bin pwmd -- `
  --listen 127.0.0.1:3031 `
  --state-root state-b `
  --network-id devnet `
  --domain-hi 0x20 `
  --cluster-id local-cluster-b `
  --node-id local-node-b
```

Ожидание: в логе каждой ноды виден `mode=explicit`, `state_ns=domain-hi-0xNN` и `listen http://127.0.0.1:…`.

Проверка головы:

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/head"
Invoke-RestMethod -Uri "http://127.0.0.1:3031/v1/head"
```

## 4) Связать ноды между собой (peer seeds + real transport)

Перезапустите обе ноды с real transport и seed на противоположный порт.

```powershell
# Node A with seed to B
cargo run -p pwmd --bin pwmd -- `
  --listen 127.0.0.1:3030 `
  --state-root state-a `
  --network-id devnet `
  --domain-hi 0x10 `
  --cluster-id local-cluster-a `
  --node-id local-node-a `
  --transport-real `
  --transport-peer-seed 127.0.0.1:3031

# Node B with seed to A
cargo run -p pwmd --bin pwmd -- `
  --listen 127.0.0.1:3031 `
  --state-root state-b `
  --network-id devnet `
  --domain-hi 0x20 `
  --cluster-id local-cluster-b `
  --node-id local-node-b `
  --transport-real `
  --transport-peer-seed 127.0.0.1:3030
```

Smoke-check связности:

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/dev/peers"
Invoke-RestMethod -Uri "http://127.0.0.1:3031/v1/dev/peers"
```

Ожидание: у каждой ноды в `/v1/dev/peers` есть peer и счетчики handshake/transport не остаются пустыми.

## 5) CLI: переключение между нодами

Глобальный URL ноды задаётся **`--rpc`** или переменной **`PWM_RPC`** (см. `pwm --help`).
Таймаут RPC для CLI задаётся отдельно через **`PWM_CLI_RPC_TIMEOUT_MS`**:
- default: `10000` ms;
- max: `120000` ms;
- значения `<= 0`, нечисловые или `> 120000` игнорируются и берётся default.

Балансовая единица: RPC/API и CLI tx-флаги используют **raw units**; операторский scale фиксирован как **`1 PWM = 1_000_000 raw`**. TUI показывает публичные балансы в decimal `PWM`, но debug JSON и internal state сохраняют raw-точность.

**Пример — команда к ноде A (`domain_hi=0x10`):**

```powershell
$env:PWM_RPC="http://127.0.0.1:3030"
cargo run -p pwm-cli --bin pwm -- --help
```

**Переключение на ноду B (`domain_hi=0x20`):**

```powershell
$env:PWM_RPC="http://127.0.0.1:3031"
cargo run -p pwm-cli --bin pwm -- tx-init --help
```

Либо явно на одну команду:

```powershell
cargo run -p pwm-cli --bin pwm -- --rpc http://127.0.0.1:3031 tx-init ...
```

## 6) Burn quota (`BURN_MARK`) через CLI

Подкоманда **`tx-burn-mark`**: списание **marks quota** (не `balance_pwm`), комиссия **0** на стороне ядра (Sprint 8). Параметры включают `--mark-amount` и опционально `--beneficiary`.

Перед отправкой убедитесь, что `PWM_RPC` указывает на ту ноду/домен, чей аккаунт и nonce вы используете в подписи.

Типичный порядок (упрощённо, как в devnet-smoke): `key-gen` / кошелёк -> `tx-init` на выбранной ноде -> затем `tx-burn-mark` с тем же `--rpc`.

Точные флаги:

```powershell
cargo run -p pwm-cli --bin pwm -- tx-burn-mark --help
```

## 7) TUI

```powershell
$env:PWM_RPC="http://127.0.0.1:3030"
cargo run -p pwm-tui --bin pwm-tui
```

Для второй ноды смените `PWM_RPC` перед запуском **нового** экземпляра TUI (или перезапустите процесс с другим env).
У TUI свой env для таймаута: **`PWM_TUI_RPC_TIMEOUT_MS`** (это не `PWM_CLI_RPC_TIMEOUT_MS`).

Ожидание: TUI подключается к указанному RPC; выход — по подсказкам в интерфейсе (например `q` / `F10`).

> Примечание: перед `F5` (burn->CLI) и `F6` (send) TUI делает preflight выбранного адреса. Если в detail видно `init=false`, TUI сначала пытается выполнить auto-init sender (`tx-init`) и при успехе сразу продолжает исходное действие (`F6` открывает send-форму, `F5` показывает burn->CLI handoff). Если кошелёк заблокирован/нет signing material, остаётся блокирующий hint с командой `pwm --rpc <url> tx-init ...`.

## 7a) Негативные сценарии: что уже покрыто вручную и что дублировать через RPC

Ниже — расширение к §8 *Negative*: те же идеи можно проверить **без TUI**, отправляя JSON на `POST /v1/tx` или читая `GET /v1/status`, чтобы не гадать по UI.

| Сценарий (смысл) | Ожидание в TUI (кратко) | Параллель через RPC / CLI |
|------------------|-------------------------|---------------------------|
| **Остановленный `pwmd`** | ошибка подключения / таймаут при опросе | `Invoke-WebRequest` / `curl` на `http://127.0.0.1:<port>/v1/status` → соединение отклонено |
| **Заблокированный кошелёк / нет подписи** | `F5`/`F6` блокируются, hint про unlock / `tx-init` | Любая подписанная tx без разблокированного wallet в CLI — пользовательская ошибка до HTTP; на стороне ноды аналог — отправка без валидной подписи → `400` (`BadSignature`) |
| **Получатель не инициализирован** (`init=false`) | preflight / ошибка до submit | `POST /v1/tx` `TRANSFER` на stub → типично ответ про получателя (см. prefilter messages в [pwmd.md](pwmd.md)); для `IMPORT` — `recipient must run tx-init` |
| **Недостаточно средств / плохой nonce** | ошибка после submit | `POST /v1/tx` → `409 CONFLICT` с текстом insufficient / bad nonce (см. юнит-тесты `pwmd` и §5 [pwmd.md](pwmd.md)) |
| **Дубликат импорта** | редко через TUI напрямую | повтор `tx-import` с тем же `export_id` → `409` (см. §9 ниже) |
| **Bridge trust refusal** (мост в отказе) | «чужой» баланс недоступен / roaming gated | `GET /v1/status` → `bridge_federation_trust` = `bridge_federation_trust_refused`; `POST /v1/export-readiness` / roaming → `409 CONFLICT`; восстановление: `POST /v1/bridge-federation/reset` при необходимости (см. [pwmd.md](pwmd.md)) |

Минимальная проверка «нода жива» без TUI:

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/status" | ConvertTo-Json
```

Проверка отказа моста (при моделировании или реальном refusal):

```powershell
(Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/status").bridge_federation_trust
# ожидается `ok` в здоровом режиме; `bridge_federation_trust_refused` при latch refusal
```

## 8) Acceptance pack (операторский уровень)

- **Happy (1):** две explicit-ноды (`domain_hi=0x10` и `0x20`) слушают разные порты; `v1/head` на обеих успешен.
- **Happy (2):** после запуска с `--transport-real` и reciprocal seeds, `/v1/dev/peers` на обеих нодах показывает peer-связность.
- **Happy (3):** `pwm` с `--rpc` на A отправляет локальную tx на A без смены genesis вручную.
- **Happy (4):** cross-domain `pwm tx-send`/TUI `F6` запускается через native/source RPC; relay/handoff делает source `pwmd`; после **`relayed`** клиент отправляет **Import** (`POST /v1/tx` на source для relay на target). Для **pwm-tui** при необходимости задаётся **`PWM_TUI_TARGET_RPC`** для nonce/баланса получателя и **шага 5** (сверка кредита); см. [ROAMING_COMPLETION.md](./ROAMING_COMPLETION.md).
- **Negative (1):** `PWM_RPC` указывает на остановленный порт — CLI/TUI возвращают понятную ошибку подключения/таймаута (не паника процесса).
- **Negative (1a):** nonce fetch при HTTP/JSON ошибке не подставляет `0`: и CLI, и TUI завершают submit явной ошибкой (`nonce fetch`/`nonce`) вместо «тихой» подписи.
- **Negative (1b):** при `HTTP 404 /v1/account/<sender>` с `account not found` в ошибке CLI/TUI есть явный hint: sender не инициализирован на этом RPC, нужен `tx-init` на source-node и проверка RPC на source domain/shard.
- **Negative (1c):** в TUI при `init=false` и недоступном signing material (`wallet locked`/нет ключа) клавиши `F5`/`F6` блокируются и показывают actionable hint про `tx-init` (без позднего submit/nonce-фейла).
- **Negative (2):** попытка использовать **один и тот же** `--state-root` для двух одновременных `pwmd` — ожидаемы конфликты/коррупция; оператор **не делает так** (отдельные каталоги как в §2).
- **Negative (3):** при запуске без explicit domain-полей shard-enforced behavior не должен считаться активным; это relay baseline path.

## 9) One-window relay и manual fallback

Основной cross-domain flow: CLI `tx-send` или TUI `F6` обращается к native/source node, создаёт roaming intent; source `pwmd` доставляет handoff на target peer через trusted configured seed; затем клиент завершает поток **подписанным Import** (автоматически в актуальном CLI/TUI после `relayed`).

**Target HTTP** в happy path по-прежнему не обязателен для *отправки* перевода, но **pwm-tui** может опрашивать target для получателя и отображения шага подтверждения баланса — задайте **`PWM_TUI_TARGET_RPC`** при нестандартных портах.

Target RPC для ручных команд нужен в manual fallback/debug. В этом режиме target-side registration не open/no-seed: `tx-handoff-register` требует, чтобы target уже доверял source peer через configured seed context.

Проверка доступности команд:

```powershell
cargo run -p pwm-cli --bin pwm -- tx-export --help
cargo run -p pwm-cli --bin pwm -- tx-import --help
```

Минимальный маршрут (концептуально):
1. Happy path: на source-node выполнить `tx-send` на cross-domain address и дождаться lifecycle `relayed`/`imported` либо понятного `last_error`.
2. Если нужен fallback/debug: на source-node выполнить `tx-export`/finalize и сохранить signed handoff JSON.
3. На target-node, который уже доверяет source peer через configured seed context, выполнить `tx-handoff-register`.
4. На target-node выполнить `tx-import`.

Контракт target recipient: `tx-import` требует, чтобы `--to` уже был initialized на target shard. Если target account отсутствует или `initialized=false`, CLI/RPC отклоняют импорт до credit; получатель должен сначала выполнить `tx-init` на target-node.

Negative expectations:
- повтор `tx-import` с тем же `export_id` -> ожидаемый `409 CONFLICT` (idempotent duplicate reject);
- неверный/неизвестный `export_id` или mismatch provenance -> ожидаемый `400 BAD_REQUEST`;
- `wallet import-seed` по-прежнему только про локальный wallet и не заменяет roaming runtime flow.

Контракт зафиксирован в: [rfc/9-crossdomain-roaming.md](./rfc/9-crossdomain-roaming.md).
Операторский пошаговый runbook (happy + negative suite): [ROAMING-SAMPLE.md](./ROAMING-SAMPLE.md).
Простое объяснение модели geo-shard/roaming: [GEO-SHARDING-EXPLANATION.md](./GEO-SHARDING-EXPLANATION.md).

## 10) См. также

- [tester-guide-devnet-smoke.md](./tester-guide-devnet-smoke.md)
- `docs/reviews/sprint-9-checklist.md`
- `tools/demo-two-shard.ps1`
- `tools/demo-two-shard.sh`
