# CY cluster policy matrix E2E (live)

Живой прогон политики V4 против локального двухнодового кластера (`cy-cluster-proposer.ps1` + `cy-cluster-attester.ps1`), с офлайн-подбором CY-адресов и транзакциями через REST/RPC узла.

## Предварительные условия

- Rust/Cargo, Windows PowerShell 5.1+.
- По умолчанию RPC узла `http://127.0.0.1:3030` и «мёртвый» RPC для брута `http://127.0.0.1:59999`, чтобы при `addr-bruteforce` не срабатывало автоподключение к живому узлу.
- После `./scripts/demo-devnet-start.ps1` в `tmp/` должны появиться `genesis-custom.json` и `demo-genesis-wallet.yaml`.

## Что делает скрипт

Файл: `scripts/cy_cluster_policy_matrix_e2e.ps1`.

1. (Опционально) `./scripts/demo-devnet-start.ps1` — генерация генезиса и демо-кошелька.
2. Очистка state при `-CleanState`: **сначала zip-бэкап** в `tmp/archives/devnet-state_<timestamp>_<label>.zip`, затем удаление `tmp/state-cy-*`, `tmp/cy-lab-peers.yaml`, … (`-SkipArchive` отключает бэкап).
3. Для каждого `k` в `1..CyWalletCount`: `addr-bruteforce` с детерминированным master (32 нулевых байта + последний байт `k`), домен CY, вывод отдельного YAML в `tmp/cy-matrix-{k}.yaml`.
4. Старт proposer и attester, ожидание `GET …/v1/status`.
5. `tx-init` премайна (индекс `287292` как в других demo-скриптах).
6. Инициализация CY-кошельков: №1 — V4 мета + `--rescue-address` (premine pretty) + `routing.emergency_redirect:dormant`; №2 — V4 без блокирующей политики; №3 — plain init.
7. Пополнение с премайна; затем `tx-policy-set default_behavior immediately` для №2 (после кредита, иначе премайн не сможет зачесть средства); `routing.same_domain_only` для №3.
8. Негатив: перевод на №2 после `default_behavior`.
9. `tx-policy-activate routing.emergency_redirect` с `--rescue-wallet` / `--rescue-account-index` на премайне; перевод на №1; смотреть изменение `balance_pwm_raw` премайна и лог `pwmd`.

## Пример запуска

Из корня репозитория:

```powershell
./scripts/cy_cluster_policy_matrix_e2e.ps1 -CleanState
```

Отладка без перезапуска кластера: `-SkipGenesis -SkipCluster` (генезис и ноды должны уже быть подготовлены вручную).

### Долгий прогон: делегировать субагенту pwm-testing + MCP `cq_process_ctl`

Скрипт может идти десятки минут (брут CY + холодный `cargo`). По канону **`docs/AGENT_PROMPT_testing.md`** и **`docs/AGENT_PROMPT_orchestrator.md`** оркестратор **не** эмулирует CQDS из PowerShell: он формулирует тикет/handoff для **`pwm-testing`**, а тот использует MCP **`user-cqds_mcp_mini`** **`cq_process_ctl`** в **`host: true`**:

1. Перед вызовами — skill **`colloquium-cqds-mcp`**, затем **`cq_help`** с нужными **`tool_ref`** (например `cq_process_ctl#spawn`, `#wait`, `#status`, `#io`, `#kill`).
2. **`spawn`**: `cwd` = абсолютный корень репозитория на Windows (например `P:\opt\docker\pwm-protocol`), **`command`** — массив вроде `powershell.exe`, `-NoProfile`, `-ExecutionPolicy`, `Bypass`, `-File`, `<repo>\scripts\cy_cluster_policy_matrix_e2e.ps1`, далее аргументы (`-CleanState` и т.д.). В **`env`** при необходимости передать **`PWM_TEST_TARGET_ROOT`** / **`CARGO_TARGET_DIR`** вне тома клона (см. **`docs/AGENT_PROMPT_testing.md`** §Windows).
3. Длинный **`wait`** и/или цикл **`status`** + **`io`** (хвост вывода), таймаут исходя из **`BruteMaxTry`** и холодной сборки (ориентир **900–3600 s** и выше при необходимости).
4. По завершении или при зависании — **`kill`** при необходимости и **принудительная зачистка** процессов **`pwmd`** на хосте (внутренний скрипт уже делает `taskkill` в **`finally`**; субагент дополнительно проверяет, что **`pwmd`** не остался).

Контракт аргументов **`spawn`/`wait`** — только из **`cq_help`**, не из интуитивных обёрток.

Параметры по смыслу: `-CyWalletCount 3`, `-BruteMaxTry 1000000`, `-SmokeSeconds`, `-RpcUrl`, `-CleanState`.

## Отчёт

Markdown-отчёт пишется в `tmp/cy_policy_matrix_e2e_<timestamp>.md`. Детальные логи proposer/attester и брута — в `tmp/cy-matrix-e2e-<timestamp>/`.

## Ограничения

- **PowerShell 5.1:** не используйте `cargo … 2>&1 | Tee-Object` при `$ErrorActionPreference = 'Stop'`: сообщения rustc/cargo на stderr превращаются в **NativeCommandError** и рвут сценарий; после пайпа **`$LASTEXITCODE`** может быть уже не кодом **cargo**. Скрипт пишет лог через **`Invoke-CargoRunLog`** (снимок потока + корректный exit).
- Брут по CY может занять заметное время при неудачном random walk; см. `-BruteMaxTry`.
- Семантику политик и отклонений см. код в `pwm-core` (`evaluate_policy`, emergency redirect).
