# Sprint 5 Status Note

Дата: 2026-04-24

## Что сделано (implementation slices #1 + #2 + #3 + #4 + #5 + #6 + #7 + #8)

- В `crates/pwmd` добавлен explicit launch identity config:
  - `network_id`,
  - `cluster_domain_hi`,
  - `cluster_id`,
  - `node_id`.
- Добавлена строгая валидация startup identity-конфига:
  - полностью explicit или полностью alias,
  - partial-конфиги отклоняются до запуска сервера.
- Сохранена backward compatibility для `--shard A|B`:
  - включен детерминированный alias mapping;
  - mapping зафиксирован в документации и коде.
- Startup-лог расширен: печатает эффективный identity tuple и mode.
- Добавлены тесты под identity parsing/validation/alias path.
- В `crates/pwmd/src/handshake.rs` добавлен `NodeHello` envelope (serde + подпись/проверка подписи) как локальный groundwork для RFC-8 AC-2/AC-5.
- Добавлен тестируемый future gate API `validate_node_hello(...)` с reject reasons:
  - `bad_signature`,
  - `replay_nonce`,
  - `network_mismatch`,
  - `genesis_mismatch`,
  - `timestamp_skew`,
  - `malformed`.
- Добавлены anti-spoof/replay утилиты без сетевого стека:
  - mandatory fields check,
  - signature envelope check,
  - timestamp skew window,
  - in-memory replay nonce cache window.
- Добавлены unit-тесты для sign/verify и negative-cases по reject причинам.
- Встроен минимальный wire-level handshake path:
  - `POST /v1/peer/hello` (dev-only),
  - `GET /v1/dev/peers` (dev-only stats/registry readback).
- В `POST /v1/peer/hello` используется `validate_node_hello(...)` из slice #2:
  - обязательный `network_id`/`genesis_hash` mismatch reject,
  - signature/replay/timestamp/malformed reject path с reason labels.
- Добавлен lightweight in-memory peer registry/state:
  - `node_id`, `domain_hi`, `class`, `last_seen_ms`, `status`.
- Peer classification реализован строго по RFC-8:
  - `native`/`foreign` только через equality `peer.domain_hi == local.cluster_domain_hi`.
- Добавлен observability groundwork:
  - reason-coded reject counters,
  - class-aware accept/connected counters,
  - structured accept/reject logs в handshake path.
- Реализован implementation slice #4: native-first policy plumbing поверх handshake registry без full p2p scheduler:
  - добавлен in-memory policy config/state: outbound targets (`native`/`foreign`), `native_min_live`, class weights, class-specific backoff envelopes;
  - добавлены deterministic policy helpers: native-first candidate prioritization, class-based backoff envelope selection, `native_degraded_state` failover signal;
  - `GET /v1/dev/peers` расширен policy snapshot-ом (`config`, `counters`, `native_live`, `native_degraded_state`) для dev observability;
  - добавлены тесты на policy ordering, backoff envelope difference by class, degraded-state toggle, и explicit no-range-heuristics invariant.
- Реализован implementation slice #5: minimal transport-level dial/reconnect wiring (без socket stack):
  - добавлен async background transport loop в `pwmd`, который работает поверх registry/policy state;
  - scheduler на каждом tick выбирает candidates в deterministic native-first порядке и применяет class-specific backoff envelopes;
  - добавлен reconnect attempt state в памяти (`attempts`, `next_due_ms`) для backoff-respected retry behavior;
  - attempt connect реализован через stub abstraction (`success` для native, `retryable_fail` для foreign) для проверки policy->scheduler wiring;
  - `GET /v1/dev/peers` расширен `transport` snapshot-ом: dial counters by `class:result`, class last-attempt timestamps/results, backoff skip counter, native underflow ticks, persistent degraded transitions;
  - добавлены тесты на scheduler ordering/backoff respect, retry transition/backoff progression и degraded-state behavior при persistent native deficit.
- Добавлены unit/integration tests на новый wire-path и negative-сценарии:
  - `bad_signature`, `replay_nonce`, `network_mismatch`, `genesis_mismatch`, `malformed`.
- Реализован implementation slice #6: controlled real socket transport wiring:
  - добавлен минимальный outbound connect path к seed peers из runtime-конфига;
  - после connect выполняется handshake-on-connect в минимальном transport frame (`u32 len + json NodeHello`): send local hello + receive peer hello;
  - incoming peer hello на real path валидируется существующим `validate_node_hello` с обновлением reason-coded counters/logs;
  - при отключенном real transport (или пустом seed list) сохранен legacy behavior через старый stub transport loop;
  - добавлены минимальные transport profile knobs в `PwmdConfig`/CLI: enable flag, seed list, connect/handshake timeout, retry base/max;
  - добавлены тесты на real handshake success, bad signature reject и timeout/backoff retry path.
- Реализован implementation slice #7: hardened transport behavior для multi-peer churn/reconnect reliability в controlled RFC-8 scope:
  - real transport tick теперь обрабатывает несколько seed peers с deterministic fairness rotation (round-robin cursor) и class-aware native-first приоритизацией известных peers;
  - добавлен bounded tick budget (`transport.tick_attempt_budget`) против storm loops при churn;
  - добавлены явные peer transport state transitions в registry (`connected` -> `retrying` -> `disconnected`) при intermittent failures;
  - reconnect path усилен guard rails: bounded retry attempts per seed + cooldown window + deterministic jitter;
  - `GET /v1/dev/peers` расширен additive observability: `churn` snapshot (rotation/retrying/disconnected/cooldown counters) и новые transport snapshot поля (`seed_rotation_cursor`, `tick_attempt_budget`, `last_tick_attempts`);
  - добавлены тесты на fairness/rotation under repeated ticks и reconnect stability под intermittent failures/ bounded cooldown semantics.
- Реализован implementation slice #8: long-run soak profile hooks в controlled runtime transport scope:
  - добавлены bounded long-run transport/churn counters/rollups для детерминированной soak-observability без выхода в production p2p stack;
  - добавлена optional periodic health snapshot aggregation по transport ticks (`transport_soak_health_interval_ticks`);
  - добавлен safety stop/limit для runaway reconnect patterns (`transport_runaway_streak_limit` + cooldown), который временно останавливает dial attempts при повторяющемся retryable storm;
  - `GET /v1/dev/peers` расширен additive полями soak confidence: uptime-like bounded loop/stability counters, reconnect streak windows, churn stability markers, runaway guard status;
  - добавлены тесты на длительные transport циклы (эмуляция long-run ticks), periodic health aggregation, bounded rollups и safety stop/resume behavior.

## Что не входит в текущий скоуп

- Production-grade p2p networking/crypto-handshake over sockets и mesh/discovery engine (в slices #6/#7 реализован только controlled seed-connect path + churn hardening, без discovery/full mesh).

## Gate state

- coding: pass (slice #1 + #2 + #3 + #4 + #5 + #6 + #7 + #8 implemented)
- testing: pass (`cargo test -p pwmd`: 55 passed, 0 failed; regression + long-run/aggregation/runaway safety tests for slice #8)
- review: pass (independent pwm-review gate completed for slice #8; artifact coherence synced)
- orchestrator: ready_for_sprint_closeout

## Следующие шаги

- Slice #8 review gate закрыт (PASS); implementation трек Sprint 5 завершен.
- Post-sprint optimization audit через `pwm-optimus` выполнен; отчет добавлен: `docs/reviews/sprint-5-optimus-report.md`.
