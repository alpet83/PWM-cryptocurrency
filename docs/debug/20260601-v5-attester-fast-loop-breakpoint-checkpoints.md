# v5 CY: attester fast-loop breakpoint checkpoints

Purpose: inspect attester-side block-production path after the seed-session micro-scheduler migration.

## 1) Start cluster and identify attester PID

Use the existing cluster scripts, then resolve attester PID via RPC listen socket (`127.0.0.2:3030`).

## 2) Primary breakpoints (attester path)

Set these first:

- `route_cluster_stub` entry: `crates/pwmd/src/transport/peer_session/mod.rs:727`
- `cluster propose accepted` checkpoint zone: `crates/pwmd/src/transport/peer_session/mod.rs:852`
- `mk_cluster_attest` builder: `crates/pwmd/src/transport/peer_session/mod.rs:696`
- attest object return callsite: `crates/pwmd/src/transport/peer_session/mod.rs:860`
- `ClusterAttest` wire write in seed steady session: `crates/pwmd/src/transport/peer_session/seed/session/steady_session.rs:346`

## 3) Scheduler checkpoints (send cadence)

Validate scheduler gates fire in expected order:

- scheduler struct + due calculation: `crates/pwmd/src/transport/peer_session/seed/session/steady_session.rs:42`
- next due math: `crates/pwmd/src/transport/peer_session/seed/session/steady_session.rs:63`
- heartbeat gated send: `crates/pwmd/src/transport/peer_session/seed/session/steady_session.rs:126`
- cluster propose gated send: `crates/pwmd/src/transport/peer_session/seed/session/steady_session.rs:141`
- sync tip gated send: `crates/pwmd/src/transport/peer_session/seed/session/steady_session.rs:197`
- read timeout tied to nearest due task: `crates/pwmd/src/transport/peer_session/seed/session/steady_session.rs:214`

## 4) Seal-gate checkpoints (proposer side reference)

When attester emits attest but seal still stalls, validate proposer gate sequence:

- `should_attempt_seal`: `crates/pwmd/src/lifecycle.rs:197`
- pre-seal attempt gate: `crates/pwmd/src/lifecycle.rs:1292`
- quorum readiness log zone: `crates/pwmd/src/lifecycle.rs:1395`

## 5) LLDB command snippet

```lldb
process attach --pid <ATTESTER_PID>
breakpoint set --file mod.rs --line 727
breakpoint set --file mod.rs --line 852
breakpoint set --file mod.rs --line 696
breakpoint set --file mod.rs --line 860
breakpoint set --file steady_session.rs --line 346
breakpoint set --file steady_session.rs --line 126
breakpoint set --file steady_session.rs --line 141
breakpoint set --file steady_session.rs --line 214
continue
```

## 6) What to capture when stopped

- `(height, round)` from propose/attest frames.
- whether `route_cluster_stub` returns `Some(ClusterAttestWire)`.
- whether write path at steady session `ClusterAttest` send succeeds.
- timing deltas between propose accept -> attest build -> attest wire send.
