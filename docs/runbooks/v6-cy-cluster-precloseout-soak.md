# V6 pre-closeout: CY cluster soak — операторский runbook

Цель: многочасовой живой кластер **CY** на **V6** билде перед sprint-final closeout (V6-11). Покрытие сверх V5: **Mode B cross-shard**, **CONSERVATION delay**, **emergency sweep** (`activation_target`).

Umbrella-тикет: `tasks/20260608-v6-cy-e2e-umbrella.json`. Отчёты: `tmp/cy-e2e-v6-*.md` с `PASS_EVIDENCE` строками.

## Роли

| Роль | Кто | Когда |
|------|-----|-------|
| **Запуск нод** | Владелец (оператор) | Перед каждой волной s1…s4 |
| **Автотесты / отчёты** | `pwm-testing` (bridge или Cursor Task) | После сигнала «кластер стабилен, head растёт» |
| **Визуальный мониторинг** | Владелец | Первые 15–30 мин + при тревоге по таблицам ниже |
| **Оркестратор** | Cursor chat | Тикеты, `share_ticket` на testing-leg, merge метаданных |

**Важно:** субагенты **не** поднимают `pwmd` на CY — только RPC/CLI против уже запущенного proposer (`http://127.0.0.1:3030`).

**Bridge vs `tasks/`:** companion смотрит `.cqds/team-tasks/done/` для `depends_on`. Тикет в `failed/` (даже при `returncode=0` и отчёте PASS) **блокирует** следующую волну. Оркестратор: recovery `failed`→`done` **до** `share_ticket` на s2+.

## Старт кластера (владелец)

Порядок из `cy-cluster-common.ps1` (как [v5 runbook](v5-cy-cluster-precloseout-soak.md)):

1. **Attester:** `.\cy-cluster-attester.ps1`
2. **Proposer:** `.\cy-cluster-proposer.ps1`

Перед чистым прогоном:

- Genesis V6-aware: `tmp\genesis-custom.json` или `$env:PWM_DEMO_GENESIS_PATH` (должен включать V6 `GenCfg`: epoch, conservation_delay_blocks, mode_b при необходимости).
- Бэкап state: `.\scripts\devnet_state_backup.ps1 -Profile CyCluster -Label v6_soak`
- Чистый state (осознанно): harness с `-CleanState` или ручная очистка `tmp\state-cy-*`
- Стабильные instance id: `cy-quorum-proposer`, `cy-quorum-attester`

**Сигнал оркестратору:** «кластер поднят, `GET /v1/status` → `head_height` растёт ≥2 мин» → открывается testing-leg **s1**.

## Волны (child tickets)

| Id | Когда | Минимальный head / время | Агент |
|----|-------|--------------------------|-------|
| **s1** bootstrap | Сразу после старта | ≥10 sealed blocks, 5–15 мин | `pwm-testing`: `scripts/cy_cluster_two_node_smoke.ps1` + log scan |
| **s2c** Mode B refund | После s1 PASS | single-shard CY; **короткий** `cross_shard_lock_timeout_blocks` в genesis | `pwm-testing`: on-chain EXPORT → wait `unlock_height` → refund smoke |
| ~~**s2**~~ Mode B (legacy) | — | **superseded** | Harness FAIL: roaming intent TTL ≠ escrow refund; см. s2c |
| **s3** conservation | После s2c PASS | `execute_at_height` reachable (delay genesis or lab-seal) | `pwm-testing`: outgoing transfer pending → execute |
| **s4** emergency sweep | После s3 PASS или parallel if accounts ready | policy + rescue cosign на тестовом аккаунте | `pwm-testing`: `ActivatePolicy` + evac smoke |

Между волнами кластер **не гасить**, если нет регрессии. Долгий soak (2–6 ч) — опционально в s1/s3 окнах сэмплинга.

### s2c: Mode B timeout refund (single-shard, variant C)

Тикет: `tasks/20260603-v6-cy-e2e-s2c-mode-b-refund.json`. Заменяет некорректный s2 (`roaming intent expired` ≠ on-chain refund).

**Два разных таймаута (не путать):**

| Механизм | Параметр | Эффект |
|----------|----------|--------|
| Roaming intent (`pwmd`) | `DEFAULT_INTENT_TTL_BLOCKS` ≈ 12 | Статус intent → `expired`; баланс **не** восстанавливается |
| Mode B escrow (`pwm-core`) | `cross_shard_lock_timeout_blocks` (дефолт 604800) | На seal при `head >= unlock_height` → `refund_exp_locks`, lock `Refunded` |

**Оператор перед s2c (обязательно):**

1. Остановить CY; сгенерировать/указать genesis с **коротким** timeout, напр. `cross_shard_lock_timeout_blocks: 10` (`tmp/genesis-custom-s2c.json` или `$env:PWM_DEMO_GENESIS_PATH`).
2. Для CY lab без on-chain stake валидатора добавить **`min_validator_stake: "0"`** (иначе `pick_prod_idx` → пустой active set на чистом state).
3. Чистый state (`tmp/state-cy-*`) — старые EXPORT с дефолтным timeout ждут ~604800 блоков.
4. Поднять **только** attester + proposer; **целевую шарду / follower не обязательно**.
5. Сигнал testing: head растёт ≥2 мин.

**Testing oracle:** `pwm-cli tx-export` → spendable↓ → дождаться `unlock_height` → spendable ≈ baseline − fee; lock `Refunded`. Ускорение head: `/v1/lab-seal/*` на loopback proposer (cluster proposer mode). **Не** считать PASS по `roaming intent status=expired`.

**IMPORT happy-path** — отдельный сценарий (нужен target peer); в s2c не требуется.

### s3: CONSERVATION delayed transfer

Тикет: `tasks/20260608-v6-cy-e2e-s3-conservation-delay.json`. Gate после s2c PASS в bridge `done/`.

**Параметр задержки:**

| Источник | Значение | Примечание |
|----------|----------|------------|
| `DEF_CONSERV_DELAY_BLOCKS` (runtime default) | 86400 | ~24h при ~1 block/s |
| Genesis JSON `conservation_delay_blocks` | **пока не читается loader'ом** (как xshard до `de9ccb3`) | Для soak нужен короткий delay + clean restart **или** follow-up loader slice |

**Testing oracle:** sender с флагом `CONSERVATION` (bit 1 в address) → `Transfer` → `pending_conservation` / spendable без debit → после `execute_at_height = enqueue_height + conservation_delay_blocks` transfer исполняется. Ускорение head: `/v1/lab-seal/*` на loopback proposer (если delay короткий).

**Share:** `python scripts/_orchestrator_share_ticket_to_bridge.py 20260608-v6-cy-e2e-s3-conservation-delay --testing` (оркестратор; `--testing` обязателен).

## Что смотреть визуально (V5 baseline + V6)

Базовые таблицы seal/quorum/sync — см. [v5-cy-cluster-precloseout-soak.md](v5-cy-cluster-precloseout-soak.md).

Дополнительно для V6:

| Область | Где | Здорово | Тревога |
|---------|-----|---------|---------|
| **Epoch / stake admission** | `/v1/status`, логи | `epoch_counter` меняется на границе; active set без stake ниже порога inactive | Застывший epoch при растущем head |
| **Mode B escrow** | account-info / state | EXPORT lock → spendable↓; refund/import по timeout/success | Зависший lock без unlock |
| **Conservation** | account-info / pending API | Outgoing transfer pending; balance до execute; после height — исполнен | Немедленный debit с conservation-адреса |
| **Emergency** | CLI `tx-init` / activation file | fee=0 activation + evac на rescue | Cross-shard evac или fee>0 |
| **Evidence (stub)** | optional RPC/log | append-only при induced miss (analytics) | Balance seizure |

Быстрый скан логов:

```powershell
.\scripts\scan_pwmd_log_counters.ps1 -LogDir .\tmp\cy-soak-v6-<timestamp> -PerFile
```

## Связанные скрипты

| Скрипт | Назначение |
|--------|------------|
| `cy-cluster-proposer.ps1` / `cy-cluster-attester.ps1` | Запуск (владелец) |
| `scripts/cy_cluster_two_node_smoke.ps1` | s1 bootstrap |
| `scripts/cy_cluster_policy_matrix_e2e.ps1` | База для policy/cross-shard (адаптировать под V6) |
| `scripts/devnet_v5_operator_smoke.ps1` | Справочник RPC-паттернов (не замена CY) |

Новые V6 harness-скрипты (s2–s4) — артефакт `pwm-testing`; до появления — ручные шаги в child-тикетах.

## Gate перед V6-11 closeout

1. `20260608-v6-cy-e2e-s1` — bootstrap PASS  
2. `20260603-v6-cy-e2e-s2c` — Mode B timeout refund PASS (supersedes s2)  
3. `20260608-v6-cy-e2e-s3` — conservation delay PASS  
4. `20260608-v6-cy-e2e-s4` — emergency sweep PASS  
5. Umbrella `done` + owner sign-off на soak-логах
