# Sprint 15 S3.12 Coding

Implemented S3.12 reconnect/visibility fixes across `pwmd` transport and `pwm-cli` semantics.

- Stabilized stateful peer sessions:
  - Added tolerant idle-timeout handling in peer wire loops (bounded timeout streak before disconnect).
  - Reduced reconnect churn from transient heartbeat/read jitter.
- Restored live foreign-balance propagation over protocol:
  - Stateful peers now push `CrossShardFacts` and `AccountViews` not only at handshake, but also during heartbeat cadence.
  - Keeps CY-side foreign view from DO fresh without waiting for reconnect.
- Preserved strict trust boundaries:
  - Authoritative merges still happen only in trusted outbound seed session path.
  - Inbound/untrusted path remains non-authoritative.
- Aligned `pwm-cli` with TUI unknown/known semantics:
  - `tx-import` recipient preflight now distinguishes foreign `home_lookup_status` states and reports explicit protocol-unavailable/unknown instead of fake “uninitialized”.
  - `tx-import` sender preflight rejects foreign/non-local account context early (wrong-shard RPC) instead of attempting misleading auto-init.
- Added focused CLI parser tests for foreign lookup metadata fields.

Out of scope: federation table work (S3.13), handshake/genesis guardrail weakening.
