# Roaming completion — итог отладки межшардового потока (S15-S3.16 / S3.17)

**Статус:** as-verified on two-node devnet (operator + TUI, 2026-05-01).  
**Связь:** [sprint-15-s3-17-closeout.md](reviews/sprint-15-s3-17-closeout.md), [ROAMING-SAMPLE.md](ROAMING-SAMPLE.md), [rfc/9-crossdomain-roaming.md](rfc/9-crossdomain-roaming.md).

## 1. Дизайн end-to-end (что должно происходить)

1. На **source** (native RPC): создаётся roaming-intent → `EXPORT` попадает в цепочку → после relay-доставки handoff статус доходит до **`relayed`**.
2. **Импорт на target не является следствием одного лишь polling статуса.** Пока пользователь не отправит **`POST /v1/tx` с телом `Import`**, target не зачислит средства. В one-window модели **эту транзакцию подписывает и шлёт клиент**; доставка на target peer по HTTP выполняет **source `pwmd`** (`relay_import` → `POST` на target RPC).
3. **Сумма кредита на target = `amount` экспорта**; **fee** удерживается на source и в кредит получателя **не входит** (как в TUI step 5).
4. Получатель на target должен быть **проинициализирован** (`tx-init`); иначе gate на `IMPORT` отклонит зачисление.

## 2. Типовые сбои, встреченные при отладке

| Симптом | Вероятная причина | Направление проверки |
|--------|-------------------|----------------------|
| Списание на source есть, на target баланс не растёт | После `relayed` **не был отправлен Import** с source (опрос intent ≠ submit tx) | TUI/CLI: автоматический Import после relayed; логи source `relay: POST /v1/tx (import)` |
| CY сводка: `exported` > 0, `imported` = 0, `pending` > 0 | Мост не закрыт — см. выше или отказ relay/target | `cross_shard_summary`, логи `relay: import HTTP error`, target `handoff_register` |
| На target «тишина» в консоли при успешном handoff | До S3.16+ на target не было **info** на входе `export-provenance`; события только в flow/memory | Обновить `pwmd`, искать `handoff_register:` / `tx commit delta: kind=import` |
| DO: `snapshot chain mismatch: block[N] state_root` при старте | Replay из файла не совпадает с **текущим** genesis/`state0()` (параметры вне `genesis_accounts`, другая сборка, битый файл) | [sprint-15-s3-16-do-snapshot-root-cause.md](reviews/sprint-15-s3-16-do-snapshot-root-cause.md); чистый `--state-root` / согласованный genesis |
| Деградация без явного запрета HTTP | `ready_degraded` всё ещё даёт `is_ready`; смешение «живой» ноды и невалидного persisted state | Диагностика `genesis_state0_digest` в логах (S3.16+), сверка конфигов |

## 3. Изменения продукта / наблюдаемости (якорные коммиты)

- **TUI:** после статуса `relayed` — подпись **Import** ключом получателя (кошелёк должен содержать `to`), запрос nonce/balance при необходимости с **target** RPC (`PWM_TUI_TARGET_RPC` или эвристика портов); **шаг 5** — явная сверка ожидаемого кредита с `local_state_balance` на target.
- **pwmd:** `relay_import` по-прежнему зеркалит завершение на source; усилены **логи** и **ошибки** relay (`relay:` / `peer relay`, корреляция `export_id`/`intent_id`), **handoff register** и локальный путь **Import** на target; при загрузке снапшота — **digest state0** и подсказки при mismatch.

## 4. Операторский минимум перед приёмкой

- Обе ноды: `phase=ready` (желательно без snapshot mismatch на target).
- Reciprocal `--transport-peer-seed`, `trusted_relay_peer_count` / `peer_relay_health` в норме.
- Для TUI: `PWM_RPC` = source; при нестандартных URL задать **`PWM_TUI_TARGET_RPC`** для target.
- После теста: сверить **`cross_shard_summary`** (`imported_amount` / счётчики) и визуальный шаг 5 в TUI.

## 5. Документы, обновлённые при closeout S3.17

См. [sprint-15-s3-17-closeout.md](reviews/sprint-15-s3-17-closeout.md) — полный список ссылок.
