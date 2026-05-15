# Wave A hash parity — pwm-testing (hotfix `d048bbe`)

**Тикет:** `tasks/20260508-wave-a-hash-parity-followup.json`  
**Коммит:** `d048bbecd33b74da2373cc413f15a62e03f1bd74` — *fix: add opt-in deterministic seal-time mode for Wave A parity*  
**Дата прогона:** 2026-05-08  

---

## Вердикт: **FAIL**

Цель **игнорируя** только локальную проблему «старый бинарь»: после сборки актуальных `pwmd`/`pwm` из workspace (см. `.cargo/config.toml` → `target-dir = "../rust-target-shared"`) сценарий Wave A **по-прежнему** завершается с `tip_hash_equal=false` и `last_epoch_hash_equal=false` при включённом `--debug-deterministic-seal-time` на обеих нодах (как в `scripts/wave_a_same_shard_stop.py`).

Остальные минимальные команды из задания (префлайт, целевые unit-тесты, `cargo check -p pwmd`) — **PASS**.

---

## Окружение и префлайт

| Шаг | Результат |
|-----|-----------|
| `bash tools/dev/preflight_target_debug.sh` | Недоступен (`execvpe(/bin/bash)` через WSL) |
| `powershell.exe -File tools/dev/preflight_target_debug.ps1` | **PASS** — `target/debug` **226464982** bytes (порог 4096 MiB) |

**Заметка про бинарники:** harness `wave_a_same_shard_stop.py` предпочитает `..\rust-target-shared\debug\{pwmd,pwm}.exe`, если они есть. Первый прогон с устаревшим `pwmd` дал `timeout waiting for ready` (в `pwmd --help` не было `--debug-deterministic-seal-time`). После `cargo build -p pwmd` и `cargo build -p pwm-cli` в общий `rust-target-shared` флаги появились; таймаут готовности ушёл.

---

## Wave A (`python scripts/wave_a_same_shard_stop.py --keep-artifacts`)

Команда: из корня репозитория, без `PWM_WORKSPACE_TARGET_ROOT`, после свежей сборки.

**Исход (stderr harness, финальная диагностика):**

```
tip_hash_equal=False
last_epoch_hash_equal=False
nodeA.tip_hash=ef74992a8376aac0865d272534facccadced2533f8fc87b7f36b259329a0feb9
nodeB.tip_hash=afae1cf39f58ed05ccb3deaa7cd4df78f247453f553f4a9e6e63517aabc690d6
nodeA.head_height=200
nodeB.head_height=200
nodeA.checkpoint=200
nodeB.checkpoint=200
```

Причина выхода: `wave-a hash divergence: tip_hash_equal=false, last_epoch_hash_equal=false`.

На успехе `--keep-artifacts` сохраняет каталог волны; при ошибке по расхождению хэшей артефакты harness не удаляет по ветке успеха — см. логи в `%TEMP%\pwm_wave_a_*` при необходимости.

---

## Поведение по умолчанию (toggle OFF)

| Тест | Результат |
|------|-----------|
| `pwmd` `config::tests::det_seal_time_default_off` | **PASS** (`PwmdConfig::default().debug_det_seal_time == false`) |

Дополнительно контракт hotfix’а на уровне цепочки без `pwmd`:

| Тест | Результат |
|------|-----------|
| `pwm-core` `chain::tests::det_mode_stable_hdr_hash` | **PASS** |

---

## Синхронизация / tip divergence (peer_session)

`cargo test -p pwmd tip_divergence` — **4 passed** (все `transport::peer_session::tests::tip_divergence_*`).

---

## Сборка

`cargo check -p pwmd` — **PASS** (`Finished dev profile`).

---

## Итог для владельца

1. **Wave A parity** после `d048bbe`: **не достигнут** — расходятся и `tip_hash`, и байты последнего epoch-файла ⇒ нужна следующая итерация (вероятно, источник недетерминизма шире, чем только `hdr.ts` при seal).
2. **Регрессий** в точечных автотестах на deterministic mode / default OFF / tip_divergence и в `cargo check -p pwmd` не выявлено.

---

## Команды (сводка)

```text
powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/dev/preflight_target_debug.ps1
python scripts/wave_a_same_shard_stop.py --keep-artifacts
cargo test -p pwm-core det_mode_stable_hdr_hash
cargo test -p pwmd det_mode_stable_hash_apps
cargo test -p pwmd det_seal_time_default_off
cargo test -p pwmd tip_divergence
cargo check -p pwmd
```

---

# Follow-up validation fix2 — 07817cae / 01ffc62 (follower seal-loop off)

**Тикет:** `tasks/20260508-wave-a-hash-parity-followup.json`  
**Коммиты:** `07817cae1b1ca2d68de5299c6d2c8f5888d5a945` (fix: follower seal-loop toggle + harness), `01ffc62d55e6889b289080f52b8991bdd15abf91` (task chore)  
**HEAD прогона:** `01ffc62d55e6889b289080f52b8991bdd15abf91`  
**Дата:** 2026-05-08  

## Вердикт: **PARTIAL**

Целевые unit-тесты и `cargo check -p pwmd` проходят. Сценарий Wave A **не доходит** до проверки `tip_hash_equal` / `last_epoch_hash_equal`: падает на **втором** `tx-send`, когда `--rpc` указывает на **node2** (`http://127.0.0.1:3231`). После отключения seal-loop только у follower состояние на node2 **отстаёт** от node1 к моменту round-robin отправки; preflight получателя на RPC node2 даёт 404 («recipient account not found»), хотя `tx-init` выполнялся через node1.

## Окружение и префлайт

| Шаг | Результат |
|-----|-----------|
| `powershell.exe -File tools/dev/preflight_target_debug.ps1` | **PASS** (`target/debug` в пределах порога) |

## Поведение harness: `--debug-disable-seal-loop` только node2

Подтверждено по `scripts/wave_a_same_shard_stop.py`: у **node1** только `--debug-deterministic-seal-time`; у **node2** добавлены `--debug-deterministic-seal-time` и `--debug-disable-seal-loop`.

## Wave A (`python scripts/wave_a_same_shard_stop.py --keep-artifacts`)

**Исход:** exit 1, `tx-send` на node2:

```text
wave-a failed: command failed: ... pwm.exe --rpc http://127.0.0.1:3231 tx-send ...
stderr=tx-send: recipient account not found on current RPC; recipient must run `tx-init` on the target shard first
```

До финальной диагностики harness (`tip_hash_equal`, `last_epoch_hash_equal`) выполнение не дошло.

## Целевые тесты

| Команда / тест | Результат |
|----------------|-----------|
| `cargo test -p pwmd disable_seal_loop_default_off` | **PASS** |
| `cargo test -p pwmd seal_loop_disable_no_seal` | **PASS** |
| `cargo test -p pwmd det_seal_time_default_off` | **PASS** |
| `cargo test -p pwmd det_mode_stable_hash_apps` | **PASS** |
| `cargo test -p pwmd tip_divergence` (×4) | **PASS** |
| `cargo test -p pwm-core det_mode_stable_hdr_hash` | **PASS** |
| `cargo check -p pwmd` | **PASS** |

## Вывод

1. **Parity target после fix2** в этом прогоне **не верифицирован** (harness останавливается раньше).
2. Вероятная **следующая правка harness** (не входила в этот коммит): дождаться синхронизации высоты/состояния node2 с node1 (или отправлять все `tx-send` через leader), прежде чем слать транзакции через RPC follower.

---

## Follow-up validation fix3 harness — commit `4f0ef1a2`

**Тикет:** `tasks/20260508-wave-a-hash-parity-followup.json`  
**Коммит:** `4f0ef1a2f9b5c440114efd37cf4ad6db1fee2a0e` (latest harness)  
**HEAD прогона:** `4f0ef1a2f9b5c440114efd37cf4ad6db1fee2a0e`  
**Дата:** 2026-05-08  

## Вердикт: **PARTIAL**

- **Цель без раннего `recipient-not-found`:** **PASS** — в журнал harness видно `wave-a tx-send rpc plan (leader-only)` с тремя отправками на `http://127.0.0.1:3230`; ошибок вида «recipient account not found» до стадии ожидания остановки **не было**.
- **Цель дойти до финальной диагностики parity:** **не выполнена** — оба прогона упали до вычисления `tip_hash_equal` / `last_epoch_hash_equal`: `wave-a failed: timeout waiting both nodes to stop by debug-stop-height`.
- **Строк вида `tip_hash_equal=…` и `last_epoch_hash_equal=…` в этом прогоне нет** (скрипт печатает их только после выхода обоих `pwmd`, см. `wait_children_exit` → `print_hash_divergence_diag`).

### Окружение и префлайт

| Шаг | Результат |
|-----|-----------|
| `powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/dev/preflight_target_debug.ps1` | **PASS** (`target/debug` 226464982 bytes, порог 4096 MiB) |

### Wave A

| Прогон | Команда | Исход |
|--------|---------|--------|
| A | `python scripts/wave_a_same_shard_stop.py --keep-artifacts` | После трёх `tx-send#` — **FAIL** через **~909 s** (`--max-wait-sec` по умолчанию **900**): `timeout waiting both nodes to stop by debug-stop-height` |
| B | то же с `--max-wait-sec 2700` | **FAIL** через **~2707 s**: та же ошибка |

**stderr (общий паттерн):**

```text
wave-a tx-send rpc plan (leader-only): http://127.0.0.1:3230, http://127.0.0.1:3230, http://127.0.0.1:3230
wave-a tx-send#1 via http://127.0.0.1:3230
wave-a tx-send#2 via http://127.0.0.1:3230
wave-a tx-send#3 via http://127.0.0.1:3230
wave-a failed: timeout waiting both nodes to stop by debug-stop-height
```

### Минимальный целевой тест (smoke)

`cargo test -p pwmd disable_seal_loop_default_off` — **PASS**.

### Вывод для владельца

1. На `4f0ef1a2` проблема **pre-parity recipient visibility**, зафиксированная после fix2, **не воспроизвелась** (все отправки через leader RPC).
2. Блокирующее событие в этом окружении — **нет выхода обоих процессов pwmd**, ожидаемых harness после достижения `--debug-stop-height`, в пределах 15–45 минут ожидания; без этого **нет** числовых `tip_hash_equal` / `last_epoch_hash_equal`.
