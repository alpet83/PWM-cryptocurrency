# Обзор: attester CY — sync, catch-up и сохранение `pwm-data.json`

## Scope

- Тикет: расследование наблюдения оператора — cluster **attester** ведёт себя как узел только RFC16-attest, без явных признаков **скачивания** истории и **persist** в `pwm-data.json`.
- Сравнение лаунчеров и трассировка пути: seed steady-session → `send_sync_tip` / входящие sync-кадры → `route_sync_stub` → `sync_live::on_tip` / apply → `periodic_snap_*` в `lifecycle.rs`.
- Ограничение: только чтение кода и лаунчеров; правки prod Rust не выполнялись.

## Findings

### 1. Лаунчеры (`cy-cluster-*.ps1`)

| Параметр | Proposer | Attester | Follower |
|----------|----------|----------|----------|
| `--cluster-enabled` | да | да | нет |
| `--cluster-role` | `proposer` | `attester` | (нет) |
| `--debug-disable-seal-loop` | нет | да | да |
| `--data-file` | `.../pwm-data.json` | то же | то же |
| Seeds | attester + follower | proposer + follower | proposer + attester |

Итог: attester и follower в CY-лабе оба в режиме **без локального seal-loop**; отличие attester от follower — включённый cluster (роль + кворум), а не отключение sync.

### 2. Steady-session и sync v1 (attester ≠ отдельная ветка)

В `run_seed_steady_session` (`steady_session.rs`) для любого seed после handshake:

- Учитываются `sync_v1 = peer_sync_v1(&remote)` и лимиты `sync_live::sync_caps`, catch-up флаг `can_cup` из hello.
- В цикле вызываются `sync_live::send_sync_tip`, затем при чтении sync-сообщений (включая `SyncTipAnnounce`) — `route_sync_stub` с теми же флагами, что и у follower.

В **`route_sync_stub`** (`mod.rs`, около 839–913) фильтры: совпадение шарда, `same_shard`, согласованность профиля `full_v1` (из capabilities удалённого узла). **Проверок `ClusterRole` нет** — роль attester/proposer/follower сама по себе sync не отключает.

В **`sync_live.rs`** нет ссылок на `ClusterRole` — catch-up (`maybe_start_cup` / `on_tip`) и применение блоков не завязаны на RFC16-роль.

### 3. `send_cluster_prop` (proposer → attester только)

`send_cluster_prop` (`mod.rs`, ~531–567):

- Выходит сразу с `Ok(())`, если локальный узел **не** `Proposer` или удалённый hello **не** `Attester`, либо `node_instance_id` не в members.
- На **attester** вызов из того же steady-loop — no-op (кластерные кадры **не** шлёт); это не блокирует sync-send/receive.

Входящие `ClusterPropose` на attester обрабатываются `route_cluster_stub` → `mk_cluster_attest` (тот же цикл, что у «обычного» attester-сценария).

### 4. Обработка `SyncTipAnnounce` — паритет с follower

- **Seed sessions** (attester дозванивается до proposer/follower): паттерн `SyncTipAnnounce` → `route_sync_stub` → `sync_live::on_tip` — см. `steady_session.rs` ~183–213 и `mod.rs` ~936–956.
- **Inbound** (`inbound.rs` ~314–342): тот же `route_sync_stub` → `on_tip`, отличие только `seed_key: None` при маршрутизации (влияет на cooldown при divergence, не на применение tip).

Семантически attester обрабатывает входящий tip так же, как follower при том же hello/capabilities.

### 5. Persist `pwm-data.json` после sync

- `apply_blk_batch` (`sync_live.rs` ~793–816) зовёт `periodic_snap_save` → при успехе `periodic_snap_finish` (`lifecycle.rs` ~208–248).
- `periodic_snap_save` срабатывает **только** если `autosnap_hit(height)`: `h > 0 && h % AUTOSNAPSHOT_BLOCK_INTERVAL == 0` (`lifecycle.rs` ~31–34), в тестах интервал зафиксирован как **100** блоков.

Следствие для оператора: цепь может **уже догоняться в памяти**, но файл `pwm-data.json` **может долго не меняться**, пока tip не дойдёт до кратного 100 (и при отсутствии graceful shutdown с полным flush). Наблюдение «файл не обновляется» **не доказывает** отсутствие sync при низкой высоте.

Для JsonFile `SealPersistMode` в `save_seal_persist` фактически не меняет путь записи (`snapshot/store.rs` ~96–98) — важен именно **вызов** сохранения, который с sync приходит только через этот autosnap gate (плюс shutdown RPC/путь из `handlers_shutdown.rs`).

### 6. Логирование: почему «тишина» в консоли

`logging.rs`:

- Консоль (`stdout` до INFO, `stderr` с WARN+) фильтрует **все** события с `target`, начинающимся с `pwmd::peer` (~319–325, ~373–375).
- События peer/sync (`info!`, `warn!`, … с `target: "pwmd::peer"`) уходят в **отдельный** peer file sink (`peer_file_template`, имя по умолчанию вокруг `pwmd-peer`, ~347–362).

Итог: отсутствие sync-сообщений в консоли **ожидаемо**; смотреть peer-лог в `log_dir`, либо поднимать детализацию через `RUST_LOG` для **неконсольных** таргетов не поможет для `pwmd::peer` на stdout — они отфильтрованы по target, а не по уровню.

## Requirements fit

- Заявленное поведение «attester участвует в RFC16 и при этом должен иметь возможность sync v1 / catch-up» **согласуется с кодом**: отдельной блокировки по `cluster-role` для sync не найдено.
- Ожидание оператора «вижу скачивание и рост pwm-data.json» **может не выполняться при текущей политике persist** (редкие checkpoint-и + логи не в консоли) **даже при исправной работе sync**.

## Style / safety / tests (кратко)

- Идентификаторы новым ревью не вводились; отдельный прогон `check_rust_fn_name_segments.py` по диффу не применим.
- Безопасность: рассмотрение уровня «где trust boundary» — sync по-прежнему зависит от peer hello / same-shard / verified blocks в `apply_blk`; это не регрессия данного вопроса.
- Тест `batch_cross_ckpt_writes_snap` в `sync_live.rs` подтверждает запись epoch manifest после apply batch на границе 100 блоков (путь `sync_apply`).

## Verdict

**Approve with nits (PASS для целей расследования):** поведение **(A)** в основном соответствует реализации; наблюдение оператора объясняется сочетанием **фильтра консольных логов** (`pwmd::peer`), **редкого autosnapshot (каждые 100 блоков)** и того, что attester **не шлёт** `ClusterPropose` (это нормально), но **шлёт и принимает** sync-кадры на общих основаниях с follower.

**Не исключён (B)** реальный сбой sync (handshake, divergence, stall), но он потребует доказательств из **peer-лога** или метрик snapshot в handshake, а не из консоли или mtime json при tip &lt; 100.

## Recommendations

1. Оператору CY: смотреть файл peer-log в каталоге логов (шаблон `peer_log_file` / `pwmd-peer`), искать `peer sync apply ok`, `peer sync catchup`, `sync_tip_seen_total` / предупреждения divergence.
2. Проверять **высоту цепи** attester (RPC / debug), а не только размер `pwm-data.json`, пока tip не пересёк кратное 100.
3. Для желаемого «чаще сбрасывать на диск после sync» — отдельный продуктовый тикет для `pwm-coding`: например, снижение интервала или отдельный режим flush после sync batch (оценить IO и согласовать с epoch manifest).
4. При подозрении на баг: снять фрагмент `pwmd-peer` вокруг первого соединения с proposer и сравнить с follower при том же genesis/network.

## Participation / token estimate

```yaml
agent: pwm-review
result: PASS
artifacts:
  - docs/reviews/20260513-attester-sync-persistence-review.md
token_usage:
  source: estimate
  input: null
  output: null
  total: 9000
  confidence: low
```

**GLOSSARY.md:** без изменений (нового жаргона не появилось).
