# CY relay / `tx-burn-mark` via standby RPC — manual report (pwm-testing)

## Повтор с паролем `1234` (оркестратор)

- **`wallet show --unsafe-show-secrets` + `PWM_WALLET_PASSPHRASE=1234`:** OK.
- **`tx-burn-mark`** (`--mark-amount 1`, `--rpc http://127.0.0.1:3031`, passphrase **1234**): **204**, burn submitted.
- После seal: `marks=12999`, `nonce=10` на **3030**; `v1/head` **18352** на 3030/3031/3032.

---

## Verdict

- **Первичный прогон pwm-testing:** **PARTIAL** (ниже — из‑за passphrase `12345`).
- **Релей + burn через :3031 с паролем `1234`:** **PASS** (см. раздел «Повтор» выше).

### История pwm-testing (PARTIAL)

- **Готово в среде агента:** preflight `target/debug`; проверка живости **3030/3031/3032** (`GET /v1/head`); согласованность **состояния аккаунта** premine `2cfb1e1d…` по трём RPC (`marks`, `nonce`).
- **Не завершено:** подписанная **`tx-burn-mark`** не выполнена — файл `tmp/cy-wallet.yaml` (encrypted) не расшифровывается с **`12345`**; корректная фраза для лабы: **`1234`** (подтверждено оператором и повторным прогоном).

## Topology (на момент проверки)

- **3 узла:** proposer **3030**, attester **3031**, follower **3032** отвечали; `head`: proposer/attester совпали (высота **18160**), follower отставал на **1** блок (типичная краткая задержка догонки).
- Альтернатива **2-node** (без follower): достаточно релея на **3031**; ниже команды остаются теми же для RPC attester.

## Preflight

```text
powershell -NoProfile -ExecutionPolicy Bypass -File tools\dev\preflight_target_debug.ps1
→ PASS: target/debug 226464982 bytes (under 4096 MiB threshold)
```

## Оператор: burn через ведомый RPC (минимум attester)

Из корня репо `P:\opt\docker\PWM-cryptocurrency`, с genesis/wallet, согласованными с `cy-cluster-*.ps1` / `tmp/genesis-custom.json`:

1. Убедиться, что кошелёк **реально** открывается (обязательная проверка passphrase):

   ```powershell
   $env:PWM_WALLET_PASSPHRASE="1234"
   cargo run -p pwm-cli --bin pwm -- wallet show --wallet tmp\cy-wallet.yaml --unsafe-show-secrets
   ```

2. Зафиксировать «до» (опционально):

   ```powershell
   Invoke-RestMethod http://127.0.0.1:3030/v1/head
   Invoke-RestMethod http://127.0.0.1:3031/v1/head
   Invoke-RestMethod http://127.0.0.1:3032/v1/head   # если поднят follower
   ```

3. Отправить **небольшой** burn через **ведомый** RPC:

   ```powershell
   cargo run -p pwm-cli --bin pwm -- `
     --rpc http://127.0.0.1:3031 `
     --wallet-passphrase $env:PWM_WALLET_PASSPHRASE `
     tx-burn-mark `
       --wallet tmp\cy-wallet.yaml `
       --mark-amount 1 `
       --purpose "relay-burn-smoke"
   ```

   Глобальные флаги (`--rpc`, `--wallet-passphrase`) можно разместить **до** подкоманды (рекомендуется для однозначного разбора clap).

4. Подтвердить включение или дельту:
   - `GET /v1/head` на **3030** (proposer): высота должна расти после seal кворумом proposer+attester;
   - при **3 узлах** — опрос `head`/`account` на **3032** после короткой паузы;
   - точечно: `GET /v1/account/<sender_hex>` — ожидание **`marks`** меньше на `mark_amount`, **`nonce`** +1.

## Артефакты

- Тикет: `tasks/20260509-cy-relay-burn-via-standby-cli.json`
- Отчёт: этот файл

## Заметка по MCP `cq_process_ctl` (host)

`spawn` с `pwsh` на MCP-хосте в этой сессии вернул **WinError 2** («файл не найден») — локальный `powershell`/preliminary команды выполнялись из интерактивной оболочки репозитория успешно.
