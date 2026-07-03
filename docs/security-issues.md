# V7-8 Security Issues

Security findings fixed during the V7-8 MVP hardening sprint. Each item is recorded with the affected endpoint or component, the issue, the fixing commit, and final status.

| ID | Severity | Endpoint/component | Description | Fix | Status |
|---|---|---|---|---|---|
| SEC-001 | Critical | `POST /v1/shutdown` | Endpoint had no operator-auth gate, so a remote caller on a non-loopback bind could stop the node. | Added loopback-or-bearer operator auth gate in `f91c477`. | FIXED |
| SEC-002 | Critical | `POST /v1/bridge-federation/reset` | Endpoint had no operator-auth gate, allowing a remote caller to reset local bridge federation trust state. | Added loopback-or-bearer operator auth gate in `f91c477`. | FIXED |
| SEC-003 | High | `OffchainStore`, `POST /v1/offchain/batch` | Offchain batch ingestion accepted unbounded batch entries and stored batches in an unbounded in-memory map, creating OOM DoS risk. | Added per-batch and total-store caps plus readiness gate in `04d69fc`. | FIXED |
| SEC-004 | Medium | `GET /v1/account/:id` | Single-account endpoint held `inner.read()` across an async foreign-home lookup, which could starve the seal loop's write lock under load. | Snapshotted account data before the await in `823ea36`. | FIXED |
| SEC-005 | Medium | `POST /v1/cross-shard/backfill` | Backfill endpoint lacked operator auth and accepted unchecked caller-supplied `peer_base`, allowing SSRF and unauthenticated validator-key signing triggers. | Added operator auth and peer-base allowlist validation in `409ada9`. | FIXED |
| SEC-006 | Medium | `ser_json_u128::U128Visitor` | Visitor omitted explicit signed integer handling, producing generic type drift errors for negative JSON values instead of precise rejection. | Added explicit `visit_i64`/`visit_i128` rejection and `visit_u128` acceptance in `fc16364`. | FIXED |
| SEC-007 | Medium | `relay_import` roaming state | Source shard marked a relay as imported after remote HTTP 204, trusting remote self-report without cross-shard fact confirmation. | Switched HTTP 204 handling to relayed/in-flight state and reserved imported transition for confirmed cross-shard fact ingestion in `b1772aa`. | FIXED |
| SEC-008 | Low | Transaction envelope shape validation | `import_fee` and `import_provenance` were accepted on non-`Import` tx bodies, creating malleable envelope surface. | Added `validate_tx_shape` guards for non-Import envelope fields in `fc16364`. | FIXED |

Additional hardening note: commit `59e0cc0` added `rpc_allowed_ips` and `rpc_allowed_auto` to gate all `/v1/*` routes by source IP when configured.
