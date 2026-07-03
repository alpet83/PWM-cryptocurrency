# V7-S2 Ramp Results

Date: 2026-06-26 (coding gate) / 2026-06-27 (live soak)

## Coding Gate

| metric | value |
|--------|-------|
| determinism_1_vs_n_workers | cargo test PASS |
| DoS 512 POST saturation | cargo test PASS |
| ramp script | scripts/cy_cluster_transfer_ramp_soak.py |
| ramp CLI aliases | --url, --duration, --target-tps |

## Live Soak — 2026-06-27

- build: **debug** (no compiler optimisation; release ожидаемо +10–15%)
- cluster: proposer + attester, filesystem state, height ≈ 278k блоков
- filter fix: `filter_plain_accounts` обновлён для отсева незапополненных аккаунтов по `min_balance_raw`

### Прогон 1 — до фикса фильтрации

```
stop_reason: reject_rate
height=278241  block_idx=7  level=20  ok=7  fail=13  slip=76ms
```

Провал: незапополненные аккаунты (40/100) не отфильтровывались → 422 `insufficient balance`.

### Прогон 2 — после фикса (чистый результат)

```
stop_reason: block_dt_overrun
height=278616  block_idx=15  level=52  ok=52  fail=0  slip=503ms
```

**✅ V7-S2 цель ≥50 tx/block достигнута при 0% reject.**

### Per-block latency (client JSONL)

| level | ok | fail | rpc_p50_ms | rpc_p95_ms |
|-------|----|------|-----------|-----------|
| 4     | 16 |    0 |       156 |       515 |
| 8     |  8 |    0 |       194 |       215 |
| 12    | 12 |    0 |       226 |       262 |
| 16    | 16 |    0 |       283 |       334 |
| 20    | 20 |    0 |       314 |       382 |
| 24    |  4 |    0 |       309 |       361 |

rpc_p50 растёт ~35ms на каждые +4 tx.
Baseline seal_slip p50=792ms (фоновая нагрузка, высота 278k блоков).

## Анализ ботлнеков

### Очередь воркеров (основной ботлнек)

`WorkerPool::new(1, 1)` = **2 OS-потока** на precheck.
52 tx прилетают параллельно → очередь глубиной ~26 на воркер →
HTTP-хендлеры блокируются на oneshot → rpc_p50 растёт линейно.
На 16-ядерной машине утилизация CPU ≈ 12%.

Увеличение до `max(2, num_cpus / 2)` воркеров должно дать
пропорциональный прирост throughput при тех же одноблочных tx.

### seal loop (вторичный ботлнек)

52 × `apply_tx_with_ctx` под write lock — время растёт O(N).
Следующий шаг: hot-path balance/nonce index для O(1) precheck
без full state simulation в воркерах.

### Debug build

Release build ожидаемо даст +10–15%, особенно на `precheck_apply_with_ctx`
и hash computation (критичный путь в seal).

## Live Soak — 2026-06-27 (V7-S3: hot index + 8 workers + rotation fix)

- build: **debug**
- cluster: proposer + attester, filesystem state, height ≈ 300k блоков
- изменения: WorkerPool(1,7)=8 workers, ArcSwap HotIndex, round-robin sender rotation

### Прогон 3 — после eviction DDoS (до rotation fix)

```
stop_reason: head_stall (DDoS)
height=299406  block_idx=10  level=40  ok=40  fail=0
```

Нода уходила в бесконечный eviction loop: `seal skip: evicting bad nonce,
requeueing 63 others` × ~30 раз. Причина: pick_senders cursor=0 всегда →
одни и те же 40 сендеров в каждом блоке → два tx с одним nonce в пуле.

### Прогон 4 — после rotation fix

```
stop_reason: block_dt_overrun
height=300048  block_idx=18  level=68  ok=68  fail=0  slip=1256ms
```

**✅ Новый потолок: 68 tx/block, 0% reject, без DDoS.**
Рост: +31% к Прогону 2 (52→68), +70% к DDoS-потолку (40→68).

Стоп по slip=1256ms (block_dt_overrun) — это I/O bottleneck filesystem
при записи snapshot, не CPU/nonce проблема.

---

## Анализ ботлнеков

### Очередь воркеров (частично устранён)

`WorkerPool::new(1, 7)` = **8 OS-потоков** на precheck (было 2).
При 68 tx параллельно очередь ≈ 9 на воркер — приемлемо.
CPU utilisation вырос с 12% до ~40-50%.

### seal loop + filesystem snapshot (текущий потолок)

68 × `apply_tx_with_ctx` + snapshot write → slip=1256ms при target 1000ms.
Hot Index (ArcSwap HotIndex) снизил стоимость worker precheck,
но bottleneck сместился в само `Chain::seal` и fsync snapshot.

Следующий шаг: ClickHouse как async write backend (Тир 2) — убирает
fsync из критического пути seal.

### sender rotation (исправлено)

`pick_senders` cursor=0 → cursor round-robin (+actual_target после каждого батча).
Устранён DDoS-паттерн нодовского eviction loop из-за дублей nonce.

### Debug build

Release build ожидаемо даст +10–15% на hash computation и apply_tx.

---

## Следующие шаги

1. **✅ Выполнено:** worker count → 8 OS-потоков (`WorkerPool(1,7)`)
2. **✅ Выполнено:** HotIndex (`ArcSwap<HashMap<AccountId, AccountHot>>`)
3. **✅ Выполнено:** sender rotation (round-robin cursor в бенчмарке)
4. **Следующий:** node-side eviction cascade fix (см. `docs/tickets/grok-eviction-loop-investigation.md`)
5. **Долгосрочно:** ClickHouse async write backend — убирает fsync из критического пути
