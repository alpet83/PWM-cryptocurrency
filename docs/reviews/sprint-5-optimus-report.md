# Sprint 5 Optimus Report (Post-Sprint Audit)

Date: 2026-04-24

## Verdict

`needs optimization pass` (non-blocking)

Sprint 5 functional acceptance remains unchanged. The optimization audit identifies technical debt and architecture hardening opportunities for the next cycle.

## Top Recommendations

1. **Decompose `crates/pwmd/src/lib.rs` by responsibility**  
   - Priority: `P1`  
   - Effort: `L`  
   - Risk: `medium`  
   - Why: One large module mixes API, snapshot/genesis, handshake, policy, transport, runtime bootstrap, and tests.  
   - Expected effect: Lower coupling, safer changes, faster review and debugging.

2. **Unify duplicated transport metrics/state transitions (stub + real paths)**  
   - Priority: `P1`  
   - Effort: `M`  
   - Risk: `low`  
   - Why: Similar dial-result and peer-state updates exist in multiple branches.  
   - Expected effect: Less copy-paste drift and fewer consistency bugs.

3. **Extract shared native-live/degraded calculations**  
   - Priority: `P1`  
   - Effort: `S`  
   - Risk: `low`  
   - Why: Native/foreign live counting logic appears in multiple call sites.  
   - Expected effect: Single semantics point for policy/degraded state.

4. **Reduce lock/alloc pressure in real transport tick hot path**  
   - Priority: `P2`  
   - Effort: `M`  
   - Risk: `medium`  
   - Why: Frequent lock windows and string-keyed map updates on repeated ticks.  
   - Expected effect: Better churn throughput and lower contention.

5. **Dependency hygiene for `tower-http` version duplication**  
   - Priority: `P2`  
   - Effort: `S`  
   - Risk: `low`  
   - Why: Duplicate versions increase build surface and maintenance cost.  
   - Expected effect: Cleaner dependency graph and faster CI/build.

## Quick Wins

- Introduce shared `record_dial_result` helper for transport metrics updates (`P1`, `S`, low risk).
- Introduce shared native-live counter helper and reuse in policy/transport/readback (`P1`, `S`, low risk).
- Merge retry/backoff calculators into one deterministic function (`P2`, `S`, low risk).
- Replace stringly labels in hot paths with typed constants/enums where possible (`P2`, `S`, low risk).
- Review and trim dependency/features footprint (`P2`, `S`, low risk).

## Structural Refactors

- Split `pwmd` runtime into modules: `identity`, `api_v1`, `snapshot`, `peer_policy`, `transport_stub`, `transport_real`, `runtime` (`P1`, `L`, medium).
- Introduce `TransportEngine` abstraction for stub/real implementations with shared policy plumbing (`P1`, `M`, medium).
- Isolate peer registry/state transitions into a dedicated component (`P2`, `M`, medium).
- Replace string-keyed hot counters with compact enum-indexed keys where practical (`P2`, `M`, medium).
- Move large in-file tests into `tests/` or thematic test modules (`P3`, `M`, low).

## Oversized Units

- `crates/pwmd/src/lib.rs` is oversized and should be the first decomposition target.
- The test section in `lib.rs` is also large enough to justify extraction into dedicated modules.
