# Divergence waves rerun — Wave A (+ planned B/C)

**Дата:** 2026-05-08  
**Контекст:** повтор тех же операторских same-shard волн, что ранее давали расхождение `tip_hash` / epoch body; проверка разрыва сессий при «несовпадении» и читаемых маркерах guard.

## Наличие скриптов

| Wave | Скрипт в репо | Запуск |
|------|----------------|--------|
| A | `scripts/wave_a_same_shard_stop.py` | выполнен |
| B (joiner, 3 ноды) | нет файла под `scripts/wave*_*.py` | **не runnable** |
| C (negative/chaos) | нет файла под `scripts/wave*_*.py` | **не runnable** |

Описание B/C есть в `docs/plans/mvp_v2.md`; автоматизации в `scripts/` нет.

---

## Wave A — команды

Preflight (`docs/AGENT_PROMPT_testing.md`):

```text
powershell -NoProfile -ExecutionPolicy Bypass -File tools/dev/preflight_target_debug.ps1
```

Прогон (из корня репо):

```text
python scripts/wave_a_same_shard_stop.py --keep-artifacts
```

Доп. верификация guard в изоляции (не wave):

```text
cargo test -p pwmd tip_divergence
```

---

## Wave A — вердикт gate

**Результат:** **FAIL по exit-коду скрипта** (ненулевой) — канонический strict gate после фикса «ложнозелёного» Wave A.

**Stderr диагностика harness:**

```text
=== Wave A hash divergence diagnostics ===
tip_hash_equal=False
last_epoch_hash_equal=False
...
wave-a failed: wave-a hash divergence: tip_hash_equal=false, last_epoch_hash_equal=false
```

**Отчёт JSON:** `F:\Temp\pwm_wave_a_h875i9t3\wave-a-report.json` (`artifacts_dir` в файле совпадает с этим каталогом).  
**Важно:** логи, которые пишет сам harness в `.../logs/node1.log|node2.log`, **не содержат** `pwmd::peer` — консольный слой pwmd фильтрует peer-target (`crates/pwmd/src/logging.rs`).

---

## Поведение peer / disconnect (артефакты pwmd)

Peer-события для этой сессии wave-node-1/2 попали в **файлы ротации** под `logs/2026-05-08/` (cwd при прогоне = корень репо), не в temp harness:

- `logs/2026-05-08/pwmd-peer-wave-node-1-150657.log`
- `logs/2026-05-08/pwmd-peer-wave-node-2-150657.log`

### Фазы

1. **Connect / handshake / runtime (раньше ~15:07):**  
   - `peer tcp connect succeeded`, `peer handshake completed`, `peer sync mode negotiated ... mode=full_v1`  
   - Затем **разрывы с `reason=protocol_error`** с деталями `wire_decode_failed: u128 is not supported` (heartbeat / sync frame) и **`peer reconnect decision ... reason=protocol_error`**, позже **`retry_after_close`**, **`peer reconnect skipped ... healthy_session_skip`**.

2. **Runtime sync при форке (после стабилизации сессии):**  
   - Долгая серия **`peer sync headers rejected ... reason=continuity_break`** (несовпадение цепочки на границе высот).  
   - **`peer sync catchup fail ... reason=chunk_order retry=N next_ms=...`** — видимый **backoff по времени** (`next_ms`).

3. **Shutdown (debug-stop-height):**  
   - `peer session close ... reason=eof ... early eof`  
   - далее `reason=protocol_error` / `os error 10053` на записи при остановке второй ноды.

### Маркер guard `SyncTipDivergence`

Строк вида **`peer sync divergence disconnect`** или **`reason=sync_tip_divergence`** в этих peer-логах **нет** (поиск по файлам — пусто).

Интерпретация: наблюдаемое «расхождение» в Wave A проявляется прежде всего как **fork / continuity_break / catchup chunk_order**, а не как срабатывание ветки `SyncTipAnnounce` с явным disconnect по политике same-height hash mismatch в этом прогоне.

---

## Юнит-тесты guard (доказательство disconnect + backoff)

`cargo test -p pwmd tip_divergence` → **4 passed** (`tip_divergence_disconnect_marks_backoff`, `tip_divergence_height_skip`, `tip_divergence_inbound_seed_cooldown`, `tip_divergence_prefers_settled_anchor`).  
Там зафиксированы `PeerCloseReason::SyncTipDivergence`, рост `sync_tip_divergence_disconnect_total` и **cooldown seed ≥ ~60s** — это **единственный автоматический сигнал** маркера guard в данном прогоне.

---

## Сводка PASS / PARTIAL / FAIL

| Элемент | Вердикт |
|---------|---------|
| **Wave A strict hash gate** | **FAIL (exit 1)** — ожидаемо при реальной дивергенции `tip_hash` / last epoch bytes |
| **Wave A peer disconnect / reconnect / backoff сигналы** | **PARTIAL** — разрывы и `retry_after_close` / `catchup fail ... next_ms` есть; **`sync_tip_divergence` в логах нет** |
| **Wave B / C** | **N/A (не runnable)** — скриптов нет |
| **`cargo test -p pwmd tip_divergence`** | **PASS** |

**Итог по цели «verify disconnect under mismatch»:** **PARTIAL** на уровне Wave A live-логов; **PASS** на уровне изолированного transport-теста guard.
