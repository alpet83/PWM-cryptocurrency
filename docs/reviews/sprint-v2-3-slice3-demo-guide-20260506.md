# Sprint V2-3 Slice 3: devnet factory + demo guide (schema v5 / policy_v2)

**Дата:** 2026-05-06  
**Тикет:** `tasks/20260506-v2-sprint3-emission-whales.json`  
**Цель:** зафиксировать операторский путь для демонстрации V2-3 emission policy без изменения runtime-логики.

## 1) Что уже есть в коде (legacy-safe baseline)

- `pwm-cli genesis-build` уже пишет `schema_version=5`.
- Для новых полей в `gen_cfg` используются legacy-safe значения по умолчанию:
  - `policy_ver=1` (legacy path),
  - `pwm_stake_min=100000`,
  - `marks_stake_min=1`,
  - `season_enabled=false`,
  - `season_coeff_ppm=1000000`.
- `dev_net()` в `pwm-core` заполняет те же значения явно, что делает локальные фикстуры и запуск без `--genesis-file` предсказуемыми и совместимыми с legacy-веткой.

## 2) Сборка genesis schema v5 (legacy-safe)

```powershell
# 0) Подготовка wallet (пример)
cargo run -p pwm-cli --bin pwm -- wallet init --country CY --wallet-out .\tmp\v23-wallet.yaml

# 1) Генерация genesis schema v5
$env:PWM_GENESIS_PASSPHRASE="dev-pass"
cargo run -p pwm-cli --bin pwm -- genesis-build `
  --wallet .\tmp\v23-wallet.yaml `
  --out .\tmp\v23-genesis-v5.json `
  --premine-bal 1000000 `
  --block-reward 100 `
  --marks-coeff 10000
```

Проверка:

```powershell
$g = Get-Content .\tmp\v23-genesis-v5.json -Raw | ConvertFrom-Json
$g.schema_version
$g.gen_cfg.policy_ver
$g.gen_cfg.pwm_stake_min
$g.gen_cfg.marks_stake_min
$g.gen_cfg.season_enabled
$g.gen_cfg.season_coeff_ppm
```

Ожидаемо: `5`, `1`, `100000`, `1`, `False`, `1000000`.

## 3) Включение policy_v2 для demo (policy_ver != 1)

На текущем шаге CLI не имеет отдельных флагов `--policy-ver/--pwm-stake-min/...`, поэтому demo-путь: сгенерировать `schema_version=5`, затем править `gen_cfg` в JSON.

```powershell
$g = Get-Content .\tmp\v23-genesis-v5.json -Raw | ConvertFrom-Json
$g.gen_cfg.policy_ver = 2
$g.gen_cfg.pwm_stake_min = "200000"
$g.gen_cfg.marks_stake_min = "200000"
$g.gen_cfg.season_enabled = $true
$g.gen_cfg.season_coeff_ppm = "500000"
$g | ConvertTo-Json -Depth 12 | Set-Content .\tmp\v23-genesis-v5-policy2.json
```

Быстрая валидация:

```powershell
$g2 = Get-Content .\tmp\v23-genesis-v5-policy2.json -Raw | ConvertFrom-Json
"schema=$($g2.schema_version) policy=$($g2.gen_cfg.policy_ver) pwm_min=$($g2.gen_cfg.pwm_stake_min) marks_min=$($g2.gen_cfg.marks_stake_min) season=$($g2.gen_cfg.season_enabled) ppm=$($g2.gen_cfg.season_coeff_ppm)"
```

## 4) Запуск ноды с custom genesis и наблюдение PWM/marks после N блоков

```powershell
# Терминал A
$env:PWM_GENESIS_PASSPHRASE="dev-pass"
cargo run -p pwmd --bin pwmd -- `
  --listen 127.0.0.1:3030 `
  --genesis-file .\tmp\v23-genesis-v5-policy2.json
```

Снимок до/после:

```powershell
# Терминал B
$acct = "<ACCOUNT_ID_HEX_OR_PRETTY>"
$b0 = Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/head"
$a0 = Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/account/$acct"

# Подождать N блоков (пример: N=10), затем повторить
Start-Sleep -Seconds 12
$b1 = Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/head"
$a1 = Invoke-RestMethod -Uri "http://127.0.0.1:3030/v1/account/$acct"

"height: $($b0.height) -> $($b1.height)"
"balance_pwm: $($a0.balance_pwm) -> $($a1.balance_pwm)"
"marks:       $($a0.marks) -> $($a1.marks)"
"staked:      $($a0.staked)"
```

Интерпретация:
- если `policy_ver=1`, ожидается legacy path (reward/marks без v2 gates);
- если `policy_ver=2`, дельты PWM/marks зависят от `staked` относительно порогов и `season_coeff_ppm`.

## 5) Ожидаемые reward cases (операторская таблица)

| Case | policy_ver | Stake vs thresholds | season_enabled / coeff | Expected PWM delta | Expected marks delta |
|---|---:|---|---|---|---|
| Legacy baseline | `1` | не применяется | не применяется | legacy `block_reward` path | legacy `accrue_marks` path |
| V2 below threshold | `2` | `staked < pwm_stake_min` и `staked < marks_stake_min` | `false` / `1000000` | `0` | `0` |
| V2 at threshold | `2` | `staked == pwm_stake_min`, `staked == marks_stake_min` | `false` / `1000000` | `> 0` (получает reward) | `> 0` |
| V2 season coeff | `2` | `staked >= thresholds` | `true` / `< 1000000` | уменьшено пропорционально `coeff` | уменьшено пропорционально `coeff` |

## 6) Demo output template (в отчёт)

| Account | Height start/end | policy_ver | pwm_stake_min | marks_stake_min | season_enabled | season_coeff_ppm | balance_pwm (start->end) | marks (start->end) | Verdict |
|---|---:|---:|---:|---:|---|---:|---|---|---|
| validator A | `H0 -> HN` | `2` | `200000` | `200000` | `true` | `500000` | `... -> ...` | `... -> ...` | `PASS/FAIL` |

Этот шаблон нужен, чтобы в одном месте зафиксировать ожидаемые и фактические дельты после N блоков.
