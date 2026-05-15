# Wave A hash parity hotfix (deterministic seal-time mode)

**Ticket:** `tasks/20260508-wave-a-hash-parity-followup.json`  
**Scope:** узкий MVP hotfix для Wave A без изменения прод-семантики по умолчанию.

---

## 1) Что добавлено

- В `pwm-core::Chain` добавлен режим времени seal:
  - `WallClock` (default, текущее поведение),
  - `DeterministicHeight` (test/dev-only).
- В `DeterministicHeight` timestamp блока вычисляется как:
  - `ts = 1_700_000_000 + height`.
- В `pwmd` добавлен явный toggle:
  - CLI: `--debug-deterministic-seal-time`
  - ENV: `PWM_DEBUG_DETERMINISTIC_SEAL_TIME=1`
  - Config field: `debug_det_seal_time` (default `false`).

---

## 2) Почему это безопасно для MVP

- Default path остаётся прежним (`WallClock`), поэтому прод/обычный devnet не меняются.
- Режим детерминизма включается только явным операторским действием (flag/env).
- Сезонные/fee вычисления в deterministic mode используют искусственный `ts`; это допустимо только для Wave A parity/harness сценариев.

---

## 3) Как включить в Wave A

- Harness уже включает toggle явно:
  - `scripts/wave_a_same_shard_stop.py` запускает обе `pwmd` ноды с `--debug-deterministic-seal-time`.
- Ручной запуск:
  - добавить `--debug-deterministic-seal-time` к команде `pwmd`,  
  - либо выставить `PWM_DEBUG_DETERMINISTIC_SEAL_TIME=1`.

---

## 4) Acceptance checks

1. **Default unchanged**
   - Без toggle (`debug_det_seal_time=false`) `pwmd` использует `WallClock`.
2. **Deterministic parity path**
   - При включённом toggle независимые инстансы с одинаковым genesis/height дают одинаковый `BlockHdr.ts` и `hdr_hash` на той же высоте.
3. **Wave A gate**
   - `python scripts/wave_a_same_shard_stop.py` проходит с:
     - `tip_hash_equal=true`
     - `last_epoch_hash_equal=true`.
4. **Build/test smoke**
   - `cargo test -p pwmd <targeted>`
   - `cargo check -p pwmd`.

---

## 5) Ограничения и follow-up

- Режим не предназначен для постоянной прод-эксплуатации.
- Если deterministic time понадобится вне Wave A, нужен отдельный RFC по time semantics + proposer/replay политике.

---

## 6) Follow-up: follower без periodic seal-loop

- Добавлен второй test/dev toggle для Wave A residual-fix:
  - CLI: `--debug-disable-seal-loop`
  - ENV: `PWM_DEBUG_DISABLE_SEAL_LOOP=1`
  - Config field: `debug_disable_seal_loop` (default `false`)
- При включении toggle нода **не** выполняет локальный `spawn_seal_loop` (не вызывает periodic `chain.seal`), но продолжает работать как sync/catch-up follower через transport и apply входящих блоков.
- Default path без флага не изменён: periodic seal-loop работает как раньше.
- Harness `scripts/wave_a_same_shard_stop.py` обновлён: leader (`node1`) без нового флага, follower (`node2`) запускается с `--debug-disable-seal-loop`.

## 7) Harness stabilization follow-up (Wave A pre-parity gate)

- После fix2 обнаружен pre-parity фейл: `tx-send` через follower RPC мог не видеть `tx-init` получателя при `--debug-disable-seal-loop`.
- Для минимального и безопасного стабилизирующего шага в harness применён leader-only маршрут для отправок:
  - `tx-init` и все три `tx-send` выполняются через `node1` RPC (`3230`).
  - Добавлен явный stderr-лог плана и каждого `tx-send` (`wave-a tx-send rpc plan (leader-only)`).
- Поведение consensus/transport не меняется; меняется только операторская стратегия отправки транзакций в тестовом harness, чтобы Wave A стабильно доходил до финальных parity checks.

## 8) Follow-up: `debug-stop-height` на follower с `--debug-disable-seal-loop`

**Проблема:** проверка `debug-stop-height` выполнялась только после успешного **локального** `chain.seal` в `spawn_seal_loop`. У follower с отключённым seal-loop цепь могла расти только через sync/apply, но процесс **никогда** не доходил до graceful shutdown по stop-height и висел до таймаута harness.

**Исправление (pwmd):** в ветке `debug_disable_seal_loop` раз в тик интервала seal-loop читается `chain.tip_h()`; если задан `debug_stop_height` и `h >= stop_h`, вызывается тот же `req_graceful_stop`, что и при обычном seal.

**Важно:** нужна **свежая** сборка `pwmd` с этим патчем. Если `cargo build` падает с `failed to remove ... pwmd.exe (os error 5)`, остановите процессы, держащие бинарь, либо соберите в отдельный target-dir и задайте harness-окружение:

```powershell
$env:CARGO_TARGET_DIR='P:\opt\docker\PWM-cryptocurrency\.wave-build-target'
cargo build -p pwmd -p pwm-cli
$env:PWM_WORKSPACE_TARGET_ROOT='P:\opt\docker\PWM-cryptocurrency\.wave-build-target'
python scripts/wave_a_same_shard_stop.py --keep-artifacts
```

## 9) Harness: динамические порты и гонка peer-listen

- Фиксированные `3230/3330` давали `EADDRINUSE` на части машин.
- `pwmd` по умолчанию слушает peer на **`rpc_port + 100`**, поэтому выбираются **два** RPC-порта так, чтобы множества `{rpc, rpc+100}` не пересекались.
- Старт: поднять **только** `node1`, `wait_ready` по HTTP, `wait_tcp_accept` на **peer** лидера (`rpc1+100`), затем `node2`, снова `wait_ready`, затем `wait_tcp_accept` на peer второй ноды — снижает гонку «dial до listen».

## 10) Текущий остаточный риск (оркестратор, 2026-05-08)

На прогоне после п.8–9 в артефакте Wave A у **follower** по-прежнему может быть **пустой** `states/node2` (нет применённой цепи на диске), при том что **leader** доходит до `stop_h` и завершается. Это указывает на проблему **P2P/sync apply** (соединение, политика, apply путь), а не только harness/stop-height. Нужна отдельная сессия `pwm-coding`/`pwm-review` с peer-трейсом (например логи `--peer-log-file` в каталог артефактов волны или `RUST_LOG` на transport).
