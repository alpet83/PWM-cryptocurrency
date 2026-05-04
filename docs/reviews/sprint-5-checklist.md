# Sprint 5 Checklist (RFC-8 runtime identity)

## Scope

- [x] `pwmd` launch identity slice #1: explicit config fields wired into runtime.
- [x] Backward compatibility path via `--shard A|B` alias mode preserved.
- [x] Deterministic alias mapping documented and implemented (no range heuristics).
- [x] Startup log prints effective identity tuple + alias/explicit mode.
- [x] Guard rail: partial identity config rejects startup.
- [x] Unit/integration tests cover identity parsing, validation, and alias path.
- [x] Existing tx guard behavior remains green under `cargo test -p pwmd`.
- [x] `pwmd` handshake groundwork slice #2: NodeHello envelope + serde/signature utilities.
- [x] Added local anti-spoof/replay validation utilities (mandatory/signature/skew/replay checks) without p2p networking.
- [x] Added reason-coded reject enum + stable reason labels for future metrics/logs.
- [x] Added testable API gate for future handshake integration (`validate_node_hello`).
- [x] Unit tests cover positive sign/verify and negative reject reasons (bad_signature, replay_nonce, timestamp_skew, network_mismatch, malformed).
- [x] `pwmd` slice #3: wired dev-only handshake probe endpoint (`POST /v1/peer/hello`) integrated with slice #2 validators.
- [x] Added lightweight in-memory peer registry (`node_id`, `domain_hi`, class, last_seen/status) with deterministic class assignment by `domain_hi` equality only.
- [x] Added class-aware/dev observability endpoint (`GET /v1/dev/peers`) with reason-coded reject counters and class counters.
- [x] Added wire-path negative tests for bad_signature, replay_nonce, network_mismatch, genesis_mismatch, malformed.
- [x] `cargo test -p pwmd` remains green after slice #3.
- [x] `pwmd` slice #4: added native-first in-memory policy config/state (targets, native_min_live, class weights, class-specific backoff envelopes).
- [x] Added deterministic policy evaluators/helpers without dialer implementation (native-first candidate order, class-based backoff selection, native degraded-state failover signal).
- [x] Extended `GET /v1/dev/peers` with policy snapshot (`config`, `counters`, `native_live`, `native_degraded_state`) for dev observability.
- [x] Added policy tests for ordering, class-specific backoff envelopes, native degraded-state on/off switching, and no range heuristics.
- [x] `cargo test -p pwmd` remains green after slice #4.
- [x] `pwmd` slice #5: added minimal async transport loop wiring policy->scheduler->state transitions (stub dial path, no sockets).
- [x] Scheduler periodically selects candidates from registry in deterministic native-first order and applies class-specific backoff envelopes.
- [x] In-memory reconnect attempt state added (`attempts`, `next_due_ms`) and used by tick scheduler to respect backoff windows.
- [x] `GET /v1/dev/peers` extended with `transport` snapshot (dial result/class counters, class last-attempt timestamps/result, backoff skips, underflow/degraded transitions).
- [x] Added tests for transport ordering + backoff respect, retry/backoff transitions over ticks, and persistent-native-underflow degraded behavior.
- [x] `cargo test -p pwmd` remains green after slice #5.
- [x] `pwmd` slice #6: real socket transport wiring added in controlled scope (seed connect loop + handshake-on-connect frame path).
- [x] Incoming `NodeHello` on real transport path is validated with existing `validate_node_hello`; reason-coded accept/reject counters/logs updated on real path.
- [x] Dev safety preserved: when real transport is disabled or seed list is empty, legacy stub transport loop behavior remains unchanged.
- [x] CLI/PwmdConfig extended with minimal transport profile (`enable`, seed peers, connect/handshake timeout, retry base/max knobs).
- [x] Added handshake-on-connect tests (positive + bad signature + timeout/backoff retry path).
- [x] `cargo test -p pwmd` remains green after slice #6.
- [x] `pwmd` slice #7: hardened multi-peer churn behavior in real transport path (multiple seeds per tick with deterministic seed rotation + class-aware native-first ordering).
- [x] Added explicit peer transport state transitions (`connected`/`retrying`/`disconnected`) and bounded reconnect guard rails (retry cap + cooldown + deterministic jitter + tick attempt budget).
- [x] Extended `GET /v1/dev/peers` with additive churn/reconnect snapshot fields (`churn.*`, `transport.seed_rotation_cursor`, `transport.tick_attempt_budget`, `transport.last_tick_attempts`).
- [x] Added tests for seed rotation fairness under repeated ticks and reconnect stability with bounded cooldown transitions.
- [x] `cargo test -p pwmd` remains green after slice #7.
- [x] `pwmd` slice #8: added controlled long-run soak hooks for runtime transport loop (bounded rollups/counters + optional periodic health aggregation + runaway reconnect safety stop).
- [x] Extended `GET /v1/dev/peers` with additive soak confidence fields (`soak.*`, `transport.soak_*`, reconnect streak/stability markers) without breaking existing contract.
- [x] Added long-run tick behavior tests (emulated extended cycles with bounded counters and periodic aggregation) and runaway safety-stop tests.
- [x] `cargo test -p pwmd` remains green after slice #8.

## Non-goals in this sprint slice set

- [x] No p2p wire networking stack implementation.
- [x] No production-grade peer dial/reconnect networking stack (slices #6/#7 keep controlled seed-connect scope, without discovery/full mesh).
- [x] No full production p2p stack in slice #8 (only controlled soak observability/guards over existing transport loop).
- [x] No range heuristics introduced in identity/validation logic.

## Verification commands

- [x] `cargo test -p pwmd`

## Post-Sprint Optimization Audit

- [ ] После финального closeout Sprint 5 запустить `pwm-optimus` на принятом коде (включая `crates/pwmd/src/lib.rs`) для отчета по декомпозиции/уплотнению модулей и архитектурным оптимизациям.
- [ ] Добавить в артефакты ссылку на optimization report и список non-blocking технического долга на следующий цикл.
