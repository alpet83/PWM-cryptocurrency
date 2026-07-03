# Sprint 15 S3.14 — testing handoff

Слайс: **S15-S3.14**. Контекст coding: `PeerWireMsg::Heartbeat` + federation gossip, `pwmd` **0.1.31**, артефакт `docs/reviews/sprint-15-s3-14-coding.md`.

Окружение: локальный репозиторий `P:\opt\docker\pwm-protocol`, Windows PowerShell. Hang-watchdog не требовался (все команды завершились штатно).

## Команды

| Команда | Wall-clock (прибл.) | Результат | Примечание |
|--------|---------------------|-----------|------------|
| `cargo fmt --check -p pwmd` | ~0.3 s | **PASS** | exit 0 |
| `cargo test -p pwmd --lib` | ~13.1 s | **PASS** | 192 passed, ~12.8 s CPU тестов |
| `cargo test -p pwmd federation` | ~8.0 s | **PASS** | 5 federation unit tests (+ пересборка ~7.75 s); binary target 0 tests |
| `cargo test -p pwmd` | ~20.5 s | **PASS** | lib 192 + main 3 + doc 0; краткий lock на build dir между параллельными запусками — без сбоя |

### Federation (отфильтрованный прогон)

Запускались тесты с именем/модулем, содержащим `federation`:

- `federation::tests::fallback_shard_key_maps_cluster`
- `federation::tests::gossip_convergence_relays_shard_without_direct_carrier_session`
- `federation::tests::merge_height_monotonic_and_seen_max`
- `federation::tests::sweep_drops_expired`
- `federation::tests::view_health_semantics`

## Smoke (опционально, живые быстрые ноды)

**Не выполнялся** отдельным ручным запуском нескольких `pwmd`. Покрытие peer wire / heartbeat в этом прогоне опирается на уже прошедшие автотесты (в т.ч. `transport::tests::*`, `stateful_transport_*`, `peer_only_micro_node_harness_survives_idle_and_heartbeats`, `production_seed_session_survives_repeated_idle_windows`). Для полевого smoke при необходимости — поднять два процесса с общим seed и проверить стабильность сессии глазами/логами.

## Cleanup

- Запущенных долгоживущих `pwmd` / `pwm-tui` не было (только `cargo test`).
- Проверка: `Get-Process pwmd,pwm-tui` — процессов нет.
- Целенаправленный `cargo clean` / удаление `target/debug/incremental` **не выполнялись** (типовой прогон тестов; при нехватке места на диске — по политике из `docs/AGENT_PROMPT_testing.md`).

## Итог

Все запрошенные проверки **`pwmd`**: fmt-check, lib, полный пакет тестов crate, выборка `federation` — **PASS**. Коммит не делался.

---

```yaml
agent: pwm-testing
result: PASS
artifacts:
  - docs/reviews/sprint-15-s3-14-testing.md
commands:
  - cmd: cargo fmt --check -p pwmd
    duration_ms: 318
    pass_fail: PASS
    hang_watchdog: false
  - cmd: cargo test -p pwmd --lib
    duration_ms: 13089
    pass_fail: PASS
    hang_watchdog: false
  - cmd: cargo test -p pwmd federation
    duration_ms: 7994
    pass_fail: PASS
    hang_watchdog: false
  - cmd: cargo test -p pwmd
    duration_ms: 20478
    pass_fail: PASS
    hang_watchdog: false
cleanup:
  cleaned: yes
  killed: []
  note: no long-lived pwmd/pwm-tui spawned; no artifact purge
token_usage:
  source: estimate
  input: null
  output: null
  total: 12000
  confidence: low
```
