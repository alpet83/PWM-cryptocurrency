# S15-S3.16 cycle2 — pwm-testing (live two-node + TUI step 5)

**Тикет:** `tasks/20260430-s15-slice3-16-cycle2-xshard-credit-tui-step5.json`  
**Дата прогона:** 2026-05-01  

## Окружение

- Ноды: `http://127.0.0.1:3030` (shard **CY**, `test-node-CY`), `http://127.0.0.1:3031` (shard **DO**, `local-node-DO`).
- Переменные для TUI (при несовпадении портов): `PWM_RPC` — source, `PWM_TUI_TARGET_RPC` — target; эвристика `3030` ↔ `3031` в `crates/pwm-tui/src/main.rs` (`cross_shard_target_rpc_base`).
- Скрипты `node-1.ps1` / `node-2.ps1` в этом прогоне не из дерева репозитория; ноды подтверждены по HTTP.

## Команды

```text
curl.exe -sS http://127.0.0.1:3030/v1/status
curl.exe -sS http://127.0.0.1:3031/v1/status
curl.exe -sS http://127.0.0.1:3030/v1/accounts
curl.exe -sS http://127.0.0.1:3031/v1/accounts
curl.exe -sS http://127.0.0.1:3031/v1/account/8b156ec0000ab8efd52949577c1a965d495b9cc7b767c85f771a2c2b5a674dab
cargo test -p pwmd --lib
cargo check -p pwm-tui
```

## Наблюдения по балансам и мосту

| Узел | Поле | Значение (выдержка) |
|------|------|------------------------|
| CY `/v1/status` | `cross_shard_summary` | `total_exported_amount`: **10000**, `total_imported_amount`: **0**, `pending_count`: **1** |
| CY `/v1/accounts` | `2cfb1e1d…` | `balance_pwm` / `spendable_on_this_shard`: **990000**, `nonce`: **1** (относительно премайна 1 000 000 — списано **10000** raw) |
| DO `/v1/accounts` | `8b156ec0…` | `local_state_balance`: **6500**, `balance_pwm`: **0** (локальная трактовка «иностранного» вида) |

**Сверка debit = credit:** по статусу CY импорт в мосту **не завершён** (`imported_count` = 0 при `exported` = 10000). Подтвердить зачисление **ровно** `amount` на домашнем шарде получателя и отсутствие fee в кредите в этом прогоне **нельзя** — end-to-end не дошёл до `imported`.

## Шаг 5 TUI (приёмка сценария)

- Интерактивный **pwm-tui** в этой сессии не запускался (headless-агент; см. `docs/AGENT_PROMPT_testing.md` — визуальный step 5 без оператора не фиксируется).
- По коду: после `imported` шаг 5 формируется в `format_balance_verify_step5` — сравнение дельты `local_state_balance` на **target RPC** с `expected_credit` = сумме экспорта; в строке явно учитывается, что комиссия снимается на source и **не** входит в кредит получателя (`crates/pwm-tui/src/main.rs`).

**Вердикт по шагу 5 в прогоне:** **N/A (не наблюдался)** — нужен оператор с TUI после свежего межшардового send.

## Вердикт по автотестам

| Команда | Результат |
|---------|-----------|
| `cargo test -p pwmd --lib` | **PASS** (после исправления флейка `snapshot_rejects_invalid_prev_hash_chain`: мутация `ff`+хвост давала no-op при `prev_hash`, уже начинающемся с `ff`) |
| `cargo check -p pwm-tui` | **PASS** |

## Итог приёмки тикета

| Критерий | Итог |
|----------|------|
| Живые две ноды | **OK** |
| Cross-shard через TUI + шаг 5 OK/FAIL | **Не выполнено** (TUI не запускался; на RPC — незавершённый импорт) |
| Debit на source vs credit на target | **FAIL по данным статуса** (export есть, imported = 0) |

**Общий вердикт:** **FAIL** (live E2E не закрыт; unit/lib pwmd — зелёные после правки теста).

## Round 3 (после pwm-coding: логи + межшард) — 2026-05-01

### Предпроверка git

- **BLOCKED: coding not landed** — **нет**: в истории есть свежие коммиты по `pwmd`/межшарду, например `01b57dc` (mirror roaming + throttle logs), `c907201`, `054683d` (TUI), `f03518b` (флейк snapshot test).
- Референс `docs/reviews/sprint-15-s3-16-do-snapshot-root-cause.md` в дереве **не найден**; рекомендация по чистому стейту DO — `docs/reviews/sprint-15-s3-16-cycle2-relay-journal-review.md` §3 (снять битый `pwm-data.json` / `state-root` на DO или поднять с genesis).

### Live / оператор

- В терминалах зафиксирован **согласованный рестарт** (~14:25:35): Ctrl+C, затем `./node-1.ps1` (пересборка `pwmd`) и `./node-2.ps1`.
- **DO** при старте: `snapshot load failed … block[16] state_root does not match` → лог **`ready_degraded`**; далее сеть и seal идут (genesis в памяти); после успешных autosnapshot текущий **`curl /v1/status`** может показывать `phase=ready` (см. `apply_snapshot_init_state` в `lifecycle.rs`).
- **Запрос на следующий прогон:** при необходимости чистого E2E — остановить оба скрипта, **очистить `./tmp/state-testnet2`** (или весь каталог по политике оператора), поднять ноды снова и выполнить **новый** cross-shard send (старый pending в сводке не закроется сам).

### Команды (Round 3)

```text
git status
git log -5 --oneline
curl.exe -sS http://127.0.0.1:3030/v1/status
curl.exe -sS http://127.0.0.1:3031/v1/status
cargo test -p pwmd --lib
```

### Логи DO/CY (handoff / import)

- **CY:** периодический **`#INFO: export/import summary: …`** (в т.ч. после рестарта на height 1000) — сводка моста; в **ранее** записанном хвосте (до рестарта) есть полная цепочка **`relay: …`** для `op=handoff` (`select_target` → `export-provenance ok (handoff delivered)`). В хвосте **после** 14:25 новых **`relay: POST /v1/tx (import)`** / **`relay: import delivered`** не искали — новый межшард не запускался.
- **DO:** **`peer cross-shard facts merged`**, **`peer account views merged`**; отдельного **info** на входе `POST /v1/export-provenance` в коде по-прежнему нет (очередь `registered:export_provenance` — внутренняя); в доступном хвосте нет **`tx commit delta: kind=import`**.
- **Сводка по `curl`:** как в Round 2 — `total_exported_amount=10000`, `total_imported_amount=0`, `pending_count=1` на обоих шардах в поле `cross_shard_summary`.

### Вердикт Round 3

| Критерий | Итог |
|----------|------|
| `cargo test -p pwmd --lib` | **PASS** (194 tests) |
| Новые info по handoff на target при сценарии | **Частично** — relay/handoff на source по старым логам есть; на target вход handoff по-прежнему без отдельного `info!`; импорт в seal не подтверждён |
| Debit = credit live | **FAIL** (импорт по сводке не завершён; нужен чистый DO + новый E2E или расследование импорта) |

```yaml
participation:
  agent: pwm-testing
  round: 3
  verdict: FAIL
  note: "Кодинг в git есть; pwmd --lib PASS; live сводка без imported; DO стартовал с snapshot mismatch; рекомендован чистый state-testnet2 + новый cross-shard."
```

## Round 4 (после `0800b14` observability + рестарт нод) — 2026-05-01

### Предпроверка git

- `git log -1 --oneline`: **`0800b14`** `pwmd: cross-shard observability — handoff register, local import, relay flow ids` (ancestor of `HEAD`).

### Команды (Round 4)

```text
git log -1 --oneline
cargo test -p pwmd --lib
cargo check -p pwm-tui
curl.exe -sS http://127.0.0.1:3030/v1/status
curl.exe -sS http://127.0.0.1:3031/v1/status
```

| Команда | Результат |
|---------|-----------|
| `cargo test -p pwmd --lib` | **PASS** (194 tests) |
| `cargo check -p pwm-tui` | **PASS** |

### Live / оператор

- В терминалах зафиксирован **рестарт** обоих скриптов (~**16:51:00** CY, ~**16:51:03** DO): Ctrl+C, затем `./node-1.ps1`, `./node-2.ps1`.
- **DO:** снапшот **`snapshot loaded`** из `./tmp/state-testnet2/pwm-data.json`, фаза **`ready (snapshot loaded)`** — в этом прогоне **нет** строки `snapshot load failed` / `state_root does not match` (см. `docs/reviews/sprint-15-s3-16-do-snapshot-root-cause.md` для класса ошибок при «битом» persisted state).
- **Новый** cross-shard сценарий (отдельная отправка после чистого моста) **не выполнялся**: субагент без интерактивного **pwm-tui** и оператора; шаг 5 TUI **не наблюдался** (`docs/AGENT_PROMPT_testing.md`).

### `curl` / debit = credit

- **CY** и **DO** `cross_shard_summary`: `total_exported_amount` = **10000**, `total_imported_amount` = **0**, `pending_count` = **1** (как в предыдущих раундах по тому же pending export).
- **Вердикт по приёмке debit = credit:** **FAIL** — импорт по сводке не завершён; без нового E2E или починки цепочки импорта сумма зачисления на target не подтверждена.

### Логи CY / DO (маркеры observability)

Поиск по буферам терминалов: **`rg`** по `handoff_register|import_provenance|relay:|genesis_state0_digest|relay_failed`.

| Маркер | Наблюдение |
|--------|------------|
| `genesis_state0_digest` | **CY** `[16:51:41.686]` `snapshot load: cross-shard bridge counters after apply \| ... genesis_state0_digest=9ab080cbfc8a9216fc274e3f4c29ee7e4a9da56c076835d7ad1325f22935453d`; **DO** `[16:51:36.686]` — тот же digest после load. |
| `handoff_register` | Отдельной строки `handoff_register:...` в хвосте после рестарта **нет**; в **WARN** на CY есть отсылка: `see handoff_register and v1_tx import logs` (`[16:51:41.686]`). |
| `relay:` | В буфере CY остаются **старые** строки успешного handoff (`[13:54:28.xxx]` … `relay: export-provenance ok`); после 16:51 новых `relay:` в захваченном хвосте нет (новый handoff не гонялся). |
| `import_provenance` / import path | Строк с подстрокой `import_provenance` в захваченных логах **не найдено** (`rg`). |
| `relay_failed` / peer relay errors | В хвосте DO после рестарта **не** искались активно; типичных `relay_failed` в показанном фрагменте нет. |

### Вердикт Round 4

| Критерий | Итог |
|----------|------|
| Коммит `0800b14` в дереве | **OK** |
| `cargo test -p pwmd --lib` | **PASS** |
| DO snapshot без degraded на load | **OK** (в этом рестарте) |
| Новый cross-shard + TUI шаг 5 | **Не выполнено** (ограничение субагента) |
| Debit = credit (live) | **FAIL** |

```yaml
participation:
  agent: pwm-testing
  round: 4
  verdict: FAIL
  note: "0800b14 на HEAD; pwmd --lib 194 PASS; DO/CY ready после рестарта, digest совпадает; сводка моста без imported; новый межшард/TUI step5 не запускались; relay/handoff в буфере — старые строки; genesis_state0_digest в snapshot load — есть."
```

## Рекомендации оператору

1. Пересобрать/перезапустить ноды и клиент с актуальным `pwm-tui` из коммита с автоматическим Import после `relayed`, затем повторить cross-shard send в TUI и зафиксировать строку шага 5 (OK/FAIL).
2. При смене портов задать `PWM_RPC` и `PWM_TUI_TARGET_RPC` явно.

```yaml
participation:
  agent: pwm-testing
  round: 4
  verdict: FAIL
  note: "См. § Round 4: 0800b14; автотесты PASS; live debit≠credit по сводке; TUI step5 не выполнялся."
  token_usage:
    source: estimate
    total: null
    confidence: low
```
