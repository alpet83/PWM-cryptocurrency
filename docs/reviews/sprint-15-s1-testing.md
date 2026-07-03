# Sprint 15 / S1 testing: cross-shard hardening (final verification after remediation2)

Date: 2026-04-29  
Repo: `P:/opt/docker/pwm-protocol`  
Mode: focused automated checks (`pwmd`, local `cargo test`)

## Commands run (final)

1. `cargo test -p pwmd v1_export_readiness_caps_oversized_ttl_sec -- --nocapture`  
   - result: PASS
2. `cargo test -p pwmd v1_export_rejects_without_readiness_and_keeps_balance -- --nocapture`  
   - result: PASS
3. `cargo test -p pwmd v1_roaming_intent_rejects_without_readiness_and_keeps_balance -- --nocapture`  
   - result: PASS
4. `cargo test -p pwmd v1_export_applies_with_valid_readiness_preflight -- --nocapture`  
   - result: PASS
5. `cargo test -p pwmd v1_status_bridge_counters_grow_after_http_export_import -- --nocapture`  
   - result: PASS
6. `cargo test -p pwmd v1_tx_http_export_import_advances_head_height_via_sync_seal -- --nocapture`  
   - result: PASS
7. `cargo test -p pwmd export_import -- --nocapture`  
   - result: PASS (focused contract area sweep)

## Validation matrix (requested)

1. Readiness TTL cap works (oversized TTL is clamped)  
   - PASS  
   - Covered by `v1_export_readiness_caps_oversized_ttl_sec`.

2. `/v1/tx` EXPORT fail-closed still happens before debit  
   - PASS  
   - Covered by `v1_export_rejects_without_readiness_and_keeps_balance`:
     - HTTP `409`,
     - reject `code=missing_preflight`,
     - sender balance/nonce unchanged.

3. `/v1/roaming-intents` EXPORT now also fail-closed before side effects  
   - PASS  
   - Covered by `v1_roaming_intent_rejects_without_readiness_and_keeps_balance`:
     - HTTP `409`,
     - reject `code=missing_preflight`,
     - no debit / no intent side effects.

4. Structured reject JSON contract has stable fields (`code`, `hint`, `message`)  
   - PASS  
   - The reject-path tests parse and assert these exact keys via `readiness_reject_fields(...)` for both `/v1/tx` and `/v1/roaming-intents`.

5. Happy path with valid readiness still passes  
   - PASS  
   - Covered by `v1_export_applies_with_valid_readiness_preflight`.

## Explicit final verdict

**PASS**

Reason: previously failing legacy tests now pass, core S15-S1 readiness tests remain green, and no new failures were observed in the focused `pwmd` export/import contract area.
