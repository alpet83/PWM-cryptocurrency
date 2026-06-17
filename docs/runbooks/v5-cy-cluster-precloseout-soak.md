# V5 pre-closeout: CY cluster soak — операторский runbook

Цель: несколько часов живого кластера **CY** (RFC16 proposer+attester) на **V5** билде перед финальным closeout. Автоматизация — тикеты `20260529-v5-cy-e2e-s*`.

## Старт кластера (корень репо)

Порядок из `cy-cluster-common.ps1`:

1. **Attester** (и при необходимости follower): `.\cy-cluster-attester.ps1`  
2. **Proposer (sealer)**: `.\cy-cluster-proposer.ps1`

Опционально третий узел: `.\cy-cluster-follower.ps1` — только если нужен replay-only наблюдатель.

Перед чистым прогоном:

- Genesis: `tmp\genesis-custom.json` или `$env:PWM_DEMO_GENESIS_PATH`
- **Бэкап state (рекомендуется перед экспериментами):** `.\scripts\devnet_state_backup.ps1 -Profile CyCluster -Label nightly_cy` → zip в `tmp\archives\`
- Harness с `-CleanState` **архивирует, затем** удаляет `tmp\state-cy-*` (см. `scripts\_devnet_clean_state.ps1`; `-SkipArchive` только если осознанно)
- Стабильные id: `cy-quorum-proposer`, `cy-quorum-attester` (не менять между рестартами в одной сессии soak)
	- Это именно `--node-instance-id` / `capabilities.node_instance_id` (cluster member id), **не** wire `--node-id` (`cy-proposer` / `cy-attester`).

RPC proposer: `http://127.0.0.1:3030` — все автотесты бьют сюда.

## Что смотреть визуально (первые 15–30 мин)

### Block timing JSONL (опционально для RCA)

- В CY launchers общий путь включается через `cy-cluster-common.ps1`:
	- `PWM_BLOCK_TIMING_ENABLED=1`
	- `PWM_BLOCK_TIMING_PATH=tmp/cy-lab-block-timing.jsonl`
- Ожидаем: append-only JSONL, по одной строке на sealed block (`schema_v=1`).
- Ключевые поля: `t0_ms`, `d_ms.prop_first_wire_send`, `d_ms.att_rx_propose`, `d_ms.att_wire_send`, `d_ms.prop_rx_attest`, `d_ms.prop_gate_ready`, `d_ms.prop_seal_commit`, `pending_ticks_at_seal`, `seal_slip_ms`.
- Для proposer+attester путь должен быть одинаковым, иначе кросс-нодовая корреляция по `(height, round)` ломается.

### Sync-ready preflight (V5)

- Proposer считает кворум по `live_synced_attesters`, а не по просто TCP-connected peers.
- Sync-ready для preflight теперь **не** жёстко привязан к `sync_live.tip_h` lag: при живом attester кворум не должен флапать из-за pipeline-лага apply/announce на 1–2 блока.
- `PWM_CLUSTER_ATTEST_MAX_TIP_LAG` оставлен как диагностический/операторский параметр, но не как primary gate критерий готовности attest path.
- При отставшем attester ожидаем `cluster_attest_waiting_sync ...` (однократно на height), а не поток `seal_suppressed...` на каждом poll.
- Когда attester догнался, появляется `cluster_attest_ready live_synced_attesters=...` и proposer возвращается к normal seal cadence.
- Если attester застрял на одном restore-boundary, в peer-логе теперь ожидается `reason=continuity_break ... local_hash=... peer_prev_hash=... streak=N`; после повторов узел fail-closed разрывает peer по `sync_hdr_divergence` вместо бесконечного 50% sync.
- Для proposer timeout path якорится к фактическому wire-send `ClusterPropose` (а не к локальному tick): при `got=0` допускается ограниченный reopen раунда (`cluster_gate_round_reopen ... retry=1/2,2/2`) перед обычным `quorum_timeout`.
- Операторский ответ для такого кейса: выровнять state (общий snapshot/`-CleanState`) и перезапустить пару, а не ждать auto-heal live sync.

### Manual seal lab RPC

Для ручного шага seal в lab-профиле доступен только loopback surface на proposer'е:

- `GET /v1/lab/seal/status`
- `POST /v1/lab/seal/control`
- `POST /v1/lab/seal/step`

Правила доступа:

- По умолчанию surface открыт только при `PWM_SEAL_CONTROL=manual-rpc` или `--seal-control manual-rpc`.
- Вне cluster-proposer surface не должен быть включён, кроме явного lab override `PWM_LAB_SEAL_API=1` / `--lab-seal-api`.
- Запросы принимаются только с loopback `127.0.0.1` / `::1`.
- `step=propose` работает через тот же proposer wake path, но не включает auto seal обратно.

Примеры:

```powershell
Invoke-RestMethod http://127.0.0.1:3030/v1/lab/seal/status
```

```powershell
Invoke-RestMethod -Method Post http://127.0.0.1:3030/v1/lab/seal/control `
	-ContentType 'application/json' `
	-Body '{"mode":"manual_rpc","verbose_default":true}'
```

```powershell
Invoke-RestMethod -Method Post http://127.0.0.1:3030/v1/lab/seal/step `
	-ContentType 'application/json' `
	-Body '{"step":"step_all","timeout_ms":5000}'
```

Ожидаемая семантика ответов:

- `status` возвращает текущий `mode`, `tip_h`, `target_h`, sync-ready snapshot и активный verbose window.
- `control` переключает runtime mode между `auto` и `manual_rpc` (JSON snake_case; CLI флаг по-прежнему `--seal-control manual-rpc`).
- `step_all` выполняет `preflight -> lease -> propose -> gate_poll -> gate_wait -> seal_commit` и останавливается на первом неготовом шаге с его snapshot.
- Если preflight видит неподходящую роль или sync-ready gap, `step_all` возвращает `ok=false` на `preflight`, а не пытается добить seal вслепую.

### Штатная остановка узла

- Для proposer и attester используйте `POST /v1/shutdown` или Ctrl+C в терминале процесса.
- Ожидаем одна запись оператора в логах: `#INFO: pwmd остановлено оператором reason=<rpc|SIGINT|SIGTERM|debug_stop> node_id=<id>`.
- На Linux/Unix `SIGTERM` проходит по тому же graceful path; на Windows в консоли Ctrl+C соответствует `SIGINT`.
- Если используется `cy_lab_seal_console.py`, остановку всё равно делайте через тот же локальный RPC, а не через `taskkill`.

| Область | Где смотреть | Здорово | Тревога |
|--------|----------------|---------|---------|
| **Кворум / seal** | stderr proposer **и** файл `logs/{date}/pwmd-{node_id}-*.log` (общий `init_logging`, шаблон по умолчанию) | Периодически `sealed height=N`, рост N; `cluster_gate_pending_summary pending_ticks_since_last_sealed=N sealed_h=H` каждые 10 sealed heights; каждые ~100s wall-clock `seal_suppression_summary window_sec=100 slots=S slots_waited_att=A slots_timeout=B slots_struck=C suppression_pct=P sealed_in_window=Y …` — `suppression_pct` считается **только как `C/S`** (strike ratio), при этом `A` показывает «ждали attest, но в timeout не вошли», `B` — реальные `quorum_timeout`; `INFO` при `suppression_pct ≤ 1.0`, `ERROR` при `>1.0` + `last_reason=lease_fence|cluster_gate|slot_skipped`. **`slots` — grid-deadline попытки (~100/100s при `bph=3600`)**, `slots_struck` — не более 1 strike на слот (interval-rule), поэтому healthy wait-path отделён от strike-path. До первого подключённого attester'а proposer пишет только периодическую (раз в 5s) `info waiting_for_attester ...`; после quorum — однократная `info cluster_attest_ready ...` | `ERROR seal_suppression_summary` несколько окон подряд (устойчиво `suppression_pct >1%` после `cluster_attest_ready`), `slots_timeout` растёт в steady режиме (признак реального timeout-path), `slots_struck` порядка тысяч/100s (регресс per-poll инфляции), `slots ≪ ~100` при `bph=3600`, WARN `missing_round_state`/`reason=quorum_timeout` в установившемся режиме |
| **Seal cadence** | stderr proposer | `seal_cadence genesis_blocks_per_hour=N seal_interval_ms=M`; `seal_scheduler mode=deadline_poll poll_ms=10 nominal_ms=M grid=multiples_of_nominal next_seal_time_ms=…` (стартовая строка proposer'a — подтверждает variant C scheduler: нет `sleep(nominal)` на верху loop'а, попытки seal привязаны к сетке `multiples_of_nominal_ms`); `seal_cadence_drift blocks=100 nominal_ms=M effective_ms=E actual_ms=A expected_ms=X adjust_pct=P envelope_pct=Q clamp_applied=true|false` (наблюдаемое, **не** управляющее поведение — `effective_ms` больше не cadence driver); `cluster_attest ... seal_interval_ms=M ... attest_timeout_ms=T ... heartbeat_interval_ms=H`; `H <= M` (инвариант **на всех CY-нодах** — и proposer, и attester; например при `bph=3600` ожидаем `H=1000` в `cluster_attest` обоих процессов), `T > M`; **owner invariant**: `|envelope_pct| <= 1.0` всегда (±1% от `nominal_ms`); `adjust_pct` — это per-step adjust fraction (ppm/10 000), **не** envelope offset; `clamp_applied=true` означает, что effective был прижат к ±1%-envelope (норма для случайной просадки/всплеска); процесс при старте re-anchor’ит `effective_ms = nominal_ms`; **inter-seal wall** на здоровом кластере группируется на second-ticks (для `M=1000ms`) — расхождения внутри окна означают gate-suppression, а не cadence drift | `H > M` хоть на одной CY-ноде (например attester показывает `heartbeat_interval_ms=1500` при `seal_interval_ms=1000` — будет ~50% suppression на proposer), `|envelope_pct| > 1.0`, длительный `clamp_applied=true` в одну сторону без стабилизации, нет стартовой строки `seal_scheduler mode=deadline_poll`, cadence не соответствует genesis, `blocks_per_hour=0` |
| **Seal cadence** | stderr proposer | `seal_cadence genesis_blocks_per_hour=N seal_interval_ms=M`; `seal_scheduler mode=deadline_poll poll_ms=10 nominal_ms=M grid=multiples_of_nominal next_seal_time_ms=…` (стартовая строка proposer'a — подтверждает variant C scheduler: нет `sleep(nominal)` на верху loop'а, попытки seal привязаны к сетке `multiples_of_nominal_ms`); `seal_cadence_drift blocks=100 nominal_ms=M effective_ms=E actual_ms=A expected_ms=X adjust_pct=P envelope_pct=Q clamp_applied=true|false` (наблюдаемое, **не** управляющее поведение — `effective_ms` больше не cadence driver); после deadline при `gate=OK` seal теперь выполняется в **том же poll-iteration** (без дополнительного `poll_pause`) — это снижает ACK→seal tail lag без нарушения grid-инварианта «не seal раньше deadline»; `cluster_attest ... seal_interval_ms=M ... attest_timeout_ms=T ... heartbeat_interval_ms=H`; `H <= M` (инвариант **на всех CY-нодах** — и proposer, и attester; например при `bph=3600` ожидаем `H=1000` в `cluster_attest` обоих процессов), `T > M`; **owner invariant**: `|envelope_pct| <= 1.0` всегда (±1% от `nominal_ms`); `adjust_pct` — это per-step adjust fraction (ppm/10 000), **не** envelope offset; `clamp_applied=true` означает, что effective был прижат к ±1%-envelope (норма для случайной просадки/всплеска); процесс при старте re-anchor’ит `effective_ms = nominal_ms`; **inter-seal wall** на здоровом кластере группируется на second-ticks (для `M=1000ms`) — расхождения внутри окна означают gate-suppression, а не cadence drift | `H > M` хоть на одной CY-ноде (например attester показывает `heartbeat_interval_ms=1500` при `seal_interval_ms=1000` — будет ~50% suppression на proposer), `|envelope_pct| > 1.0`, длительный `clamp_applied=true` в одну сторону без стабилизации, нет стартовой строки `seal_scheduler mode=deadline_poll`, cadence не соответствует genesis, `blocks_per_hour=0` |
| **Кластер wire** | proposer + attester stderr | `ClusterPropose` / attest без `binding_mismatch` | `cluster attest dropped`, `binding_mismatch` |
| **Propose churn / wake** | `pwmd-peer-cy-proposer-*.log` + proposer main log | `cluster propose sent` к `sealed height` обычно <~5x в steady (coalesce on `(height,round)`); после `cluster attest accepted` seal-path просыпается без ожидания полного heartbeat sleep/poll-window; `pending_ticks_since_last_sealed` медиана заметно ниже resumed baseline | `propose_sent/sealed` двузначный множитель (10x+), длинные плато `pending_ticks` в сотнях при живом attester, повторные propose на тот же `(height,round)` без роста head |
| **Attester sync** | attester stderr | После догона — **тишина** `Sync progress` (standby short-tail) | Бесконечный `%` catch-up, `sync_tip_divergence`, `TipDivergence` |
| **HTTP** | `GET /v1/status` | `head_height` растёт, стабильный `chain_id` / network | 5xx, зависший `head_height` |
| **Marks (V5)** | `pwm account-info` или smoke | `marks_last_block` двигается к head; `marks_sat_pct` → 100 при stake | `marks_last_block` замёрз при растущем head; effective < stored без touch |
| **Память/диск** | Диспетчер задач / размер `tmp\state-cy-*` | Плавный рост, без скачков GB/мин | runaway RAM, jsonl/snapshot раздувается быстрее блоков |

Быстрый скан логов после окна:

```powershell
.\scripts\scan_pwmd_log_counters.ps1 -LogDir .\tmp\cy-soak-<timestamp> -PerFile
```

Healthy soak guidance: `head_delta` should grow; `suppressions/head_delta` is more useful than raw `sealed height=` line count because `sealed height` is sparse. Startup-only `detail=missing_round_state` is acceptable; repeated `reason=quorum_timeout`, `binding_mismatch`, or `cluster attest dropped` needs inspection. Pre-timeout `detail=attestations_missing` is DEBUG-only in normal console output; watch the periodic `cluster_gate_pending_summary` while head advances and timeout stays near zero.

## Cluster Prep Visibility

After CY restart and before sealed height advances, use these operator signals:

- `pwm status --rpc http://127.0.0.1:3030` prints `cluster_prep phase=... ready_for_seal=... sync_n=... live_n=... blocks_behind_max=...`.
- `GET /v1/status` includes `cluster_prep.phase`, `ready_for_seal`, `sync_n`, `live_n`, `peer_tip_max`, `local_tip`, `blocks_behind_max`, `waiting_since_ms`, and `blocked_reason`.
- Proposer logs `cluster_prep_summary ... waiting_sec=...` at least every 30s while waiting for attester quorum.
- Attester catch-up logs `sync_catchup_progress ... blocks_behind=... percent_complete=...` about every 10s while lag remains positive.
- During CY restart with snapshot trust-load, proposer may stay in `cluster_prep phase=loading_snapshot blocked_reason=loading_snapshot` for 1-3+ minutes before first seal; this is expected while `loading_sec` grows and `local_tip` stabilizes.
- In this startup phase, periodic proposer INFO now includes `cluster_prep_summary phase=loading_snapshot ... loading_sec=... blocked_reason=loading_snapshot ...` and `seal_suppression_summary ... blocked_reason=loading_snapshot` when `sealed_in_window=0`.

## Что смотреть визуально (soak 2–6+ часов)

| Область | Действие | Ожидание V5 |
|--------|----------|-------------|
| **Итеративное насыщение марок** | Каждые 30–60 мин: `account-info` на 2–3 staked кошельках | `marks_effective` → `4294967295`, `marks_sat_pct=100`, `marks_last_block` ≈ текущий head−ε |
| **Инфляция / seal** | Сравнить `head_height` и баланс premine-аккаунтов | Block reward и cadence по genesis; без отрицательных балансов |
| **Deferred (если в genesis)** | При достижении `activate_at_height` | `stored_active_policies` / API policy flags меняются один раз |
| **Массовые burn** | Во время slice-3: всплеск tx | Seal не останавливается; нет лавины `InsufficientMarks` после touch |
| **Стабильность процессов** | Число `pwmd.exe` | 2 (proposer+attester) или +1 follower; нет зомби после taskkill |

TUI marks operator path: see `docs/runbooks/v5-tui-marks-operator-path.md`.

Снимки для отчёта: `tmp/devnet_v5_*` / `tmp/cy-soak-*` markdown с `PASS_EVIDENCE` строками.

## Связанные скрипты

| Скрипт | Назначение |
|--------|------------|
| `cy-cluster-proposer.ps1` | Sealer, RPC :3030 |
| `cy-cluster-attester.ps1` | Standby attester |
| `cy-cluster-follower.ps1` | Опциональный follower |
| `scripts/cy_cluster_two_node_smoke.ps1` | Короткий bootstrap smoke (E2E s1) |
| `scripts/cy_cluster_marks_soak.ps1` | Live marks saturation soak (E2E s2-rerun) |
| `scripts/cy_cluster_marks_soak.py` | REST-only вариант s2 soak (без PowerShell harness) |
| `scripts/cy_cluster_mass_burn_soak.ps1` | Mass `BurnMark` batch burst (E2E s3) |
| `scripts/cy_cluster_policy_matrix_e2e.ps1` | Policy + brute wallets (V4 matrix) |
| `scripts/devnet_v5_operator_smoke.ps1` | V5 operator lanes (single-node devnet) |
| `scripts/scan_pwmd_log_counters.ps1` | Счётчики по логам soak |

## Ограничения модели (для интерпретации «миллиардов марок»)

- Одна транзакция `BurnMark`: `mark_amount: u32` (макс. ~4.29e9 за tx **после touch**).
- «Миллиарды» в soak = **суммарно** по серии tx / нескольким аккаунтам (имитация оффчейн-батча), не одним полем tx.
- Оффчейн Merkle batch (`docs/OFFCHAIN_STUB.md`) в этом gate **не** обязателен — только on-chain `burn_mark` поток.

## Gate перед V5 closeout

1. `20260529-v5-cy-e2e-s1` — bootstrap + стабилизация PASS — `tmp/cy-e2e-s1-20260528_220256.md`  
2. `20260531-v5-cy-e2e-s2-marks-saturation-soak-rerun` — marks soak PASS (PARTIAL: 2 staked) — `tmp/cy-e2e-s2-20260530_082418.md`  
3. `20260529-v5-cy-e2e-s3` — mass burn batches PASS — `tmp/cy-e2e-s3-20260530_141317.md`  
4. Doc alignment: [20260530-v5-precloseout-cy-e2e-docs-version-review.md](../reviews/20260530-v5-precloseout-cy-e2e-docs-version-review.md) PASS_WITH_NITS (ниты закрыты).  
5. Sprint-final closeout: [20260530-v5-sprint-final-closeout-review.md](../reviews/20260530-v5-sprint-final-closeout-review.md) PASS — owner sign-off complete (2026-06-02).
