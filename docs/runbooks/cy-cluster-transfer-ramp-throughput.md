# Runbook: CY cluster transfer-ramp throughput soak

**Audience:** operator / performance investigation  
**Ticket:** `tasks/20260617-cy-cluster-transfer-ramp-throughput.json`  
**Harness:** `scripts/cy_cluster_transfer_ramp_soak.py` (Python)

Оценка выносливости CY-кластера под нарастающей нагрузкой same-domain `Transfer` между аккаунтами `tmp/demo-genesis-wallet.yaml` через **proposer RPC** (`http://127.0.0.1:3030`).

---

## Prerequisites

1. CY cluster live: proposer `127.0.0.1:3030`, attester synced.
2. `PWM_BLOCK_TIMING_ENABLED=1` и `tmp/cy-lab-block-timing.jsonl` (по умолчанию в `cy-cluster-common.ps1`).
3. Собранный `pwm` CLI (`PWM_BIN` или `F:\pwm-test\pwm-protocol\debug\pwm.exe`).
4. Python 3.10+ и `PyYAML` (`pip install pyyaml`).
5. Demo wallet с ≥2 аккаунтами `expected_flags_u32=0`, инициализированными на CY.

> **Cosign-nd:** многие demo-аккаунты требуют активную cosign-policy для исходящих Transfer. Harness по умолчанию делает **probe** и использует только send-capable аккаунты как senders; все funded аккаунты остаются recipients. Для полного кольца multi-sender — заранее настроить policy или добавить в wallet аккаунты с `flags_mask` без bit0 (см. `v6-owner-stability-soak-50k.md` § addr-bruteforce).

---

## Быстрый smoke (3–5 блоков)

```bash
python scripts/cy_cluster_transfer_ramp_soak.py \
  --rpc http://127.0.0.1:3030 \
  --wallet tmp/demo-genesis-wallet.yaml \
  --pwm-bin F:/pwm-test/pwm-protocol/debug/pwm.exe \
  --max-blocks 5 \
  --start-txs-per-block 1 \
  --step-txs-per-block 1
```

Артефакты: `tmp/cy-transfer-ramp-<UTC>.client.jsonl`, `tmp/cy-transfer-ramp-<UTC>.md`.

Пост-анализ с join block_timing:

```bash
python scripts/_analyze_transfer_ramp.py \
  --client-jsonl tmp/cy-transfer-ramp-<UTC>.client.jsonl \
  --block-timing tmp/cy-lab-block-timing.jsonl
```

---

## Модель ramp

| Режим | Флаг | Поведение |
|-------|------|-----------|
| **block** (default) | `--ramp-mode block` | После warm-up каждый новый sealed block: `+step` txs к предыдущему уровню |
| **window** | `--ramp-mode window` | Каждые `--window-blocks` (10): +step txs за всё окно |

Параметры по умолчанию:

| Параметр | Default | Смысл |
|----------|---------|--------|
| `--start-txs-per-block` | 1 | Стартовый burst на блок |
| `--step-txs-per-block` | 1 | Прирост на блок/окно |
| `--max-txs-per-block` | 64 | Потолок burst |
| `--warmup-blocks` | 2 | Baseline seal_slip без эскалации |
| `--amount` / `--fee` | 1000 / 1 | Минимальный churn |
| `--max-reject-pct` | 5 | Стоп при превышении |
| `--stall-timeout-ms` | 5000 | Стоп при отсутствии head |
| `--slip-mult-stop` | 3.0 | Стоп: seal_slip > 3× warm-up p95 |

Паттерн переводов: кольцо `acc[i] → acc[i+1]` (round-robin по derivation_index).

---

## Метрики

**Client JSONL** — каждый submit: `batch_height`, `level`, `from_index`, `rpc_latency_ms`, `ok`, `nonce_before`.

**block_timing JSONL** — per height: `seal_slip_ms`, `pending_ticks_at_seal`, `d_ms.prop_seal_commit`, `nominal_seal_ms`.

**Отчёт harness** — таблица per-block, sustained tx/block (последний уровень с reject < 5%).

**Analyzer** — p50/p95 по уровням ramp, blocks/sec estimate.

---

## Интерпретация throughput

- **sustained_tx_per_block** — последний уровень ramp с приемлемым reject rate; практический потолок burst на блок.
- **blocks_per_sec_est** — из block_timing `t0_ms` delta; падает при seal_slip / pending_ticks росте.
- **Деградация** — рост `seal_slip_p95` и `rpc_latency_ms` p95 при фиксированном nominal 1000ms grid.

Полный прогон до `max_tx_level` или `reject_rate` / `seal_slip_degradation` — operator session, не CI.

---

## Stop reasons

| reason | Действие |
|--------|----------|
| `max_blocks` | Нормальное завершение smoke |
| `max_tx_level` | Достигнут потолок ramp |
| `reject_rate` | Mempool/RPC отвергает >5% — зафиксировать уровень |
| `head_stall` | Кластер перестал seal — RCA (logs, quorum) |
| `seal_slip_degradation` | Seal path деградировал vs warm-up |

---

## Ссылки

- `docs/FEATURES.md` § block_timing
- `docs/pwm-cli.md` § tx-send
- `scripts/cy_cluster_mass_burn_soak.ps1` (предыдущий soak-паттерн)
