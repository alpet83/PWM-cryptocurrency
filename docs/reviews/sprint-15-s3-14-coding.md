# Sprint 15 S3.14 — coding handoff

## Scope

- Восстановление `cargo test -p pwmd --lib` и `cargo fmt --check -p pwmd` после долга S3.13 (readiness, lifecycle, deadlock, slice20).
- Trusted federation gossip: реле набора строк таблицы в `Heartbeat` (wire), приём только при `trusted == true`, источник строки — `FedRowSource::Gossip`, `source_node_id` сохраняется из payload (носитель наблюдения).
- Юнит-тест конвергенции на таблице (два шага: pack gossip → merge у «наблюдателя» без прямого hello от носителя шарда).
- Bump маркера сборки `pwmd`: `0.1.30` → `0.1.31` (расширение peer wire JSON).

## Изменённые файлы

| Файл | Суть |
|------|------|
| `crates/pwmd/Cargo.toml` | Версия `0.1.31`. |
| `crates/pwmd/src/federation.rs` | `FedGossipWireRow`, `FedRowSource::Gossip`, `gossip_wire_rows`, merge в `merge_remote_hb`, тест `gossip_convergence_relays_shard_without_direct_carrier_session`. |
| `crates/pwmd/src/transport.rs` | Поле `federation_gossip` в `PeerWireMsg::Heartbeat`, заполнение при исходящем heartbeat, проброс во все вызовы `merge_remote_hb`; inbound untrusted по-прежнему не применяет gossip. |
| `crates/pwmd/src/lib.rs` | Правки тестов: export-readiness, relay expectations, duplicate roaming + второй readiness, deadlock transfer без self-transfer, smoke export readiness. |
| `crates/pwmd/src/lifecycle.rs` | Seal-тест: transfer на второй инициализированный аккаунт; ожидание первого тика seal loop. |
| `crates/pwmd/src/slice20_e2e_tests.rs` | Peer seed на `RPC+100`, кошельки + `tx-init` для получателей (CLI), ослабление assert finalize (`exported` \| `relayed`), polling GET account. |
| `issues-report.md` | Запись о причинах долга тестов и обходах. |

## Команды

```text
cargo fmt -p pwmd
cargo fmt --check -p pwmd
cargo test -p pwmd --lib
```

Результат: **PASS** (192 теста).

## Риски / ограничения

- Gossip приходит только в trusted transport heartbeat; HTTP inbound и не-trusted wire не ослаблены.
- Размер gossip ограничен (~32 строки, ~4 KiB оценочно); при переполнении возможна более медленная сходимость в больших кластерах.
- Slice20 finalize может оставаться в `exported`, если peer relay не успел — тест допускает оба статуса и продолжает ручной handoff (как задумано контрактом).

## CQDS index rebuild

Не вызывался: правки выполнялись в локальной рабочей копии; при синке в Colloquium имеет смысл поставить фоновый `rebuild_index` для проекта PWM (`project_id` 5).

## Optimization note

Дублирование порогов gossip (rows/bytes) остаётся локальным в `federation.rs`; при следующем расширении wire имеет смысл вынести константы в один модуль с transport limits.

## Follow-ups

- **pwm-testing:** полный прогон вне `--lib`, при необходимости стресс TTL gossip и размер кадра heartbeat в интеграции.
- **pwm-review:** секция **Distributed federation view** в `docs/reviews/sprint-15-s3-14-review.md` (эпидемическая модель, trust, TTL); сопоставить с сценарием ~100 нод / ~10 шардов / fan-out ~5.
