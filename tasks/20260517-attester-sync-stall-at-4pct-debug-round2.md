# Debug report round 2: attester sync stalls near 3-5%

Ticket: `tasks/20260517-attester-sync-stall-at-4pct.json`  
Agent: `pwm-debug`  
Verbosity focus: `transport:sync`  
Artifact: `tasks/20260517-attester-sync-stall-at-4pct-debug-round2.md`

## Scope and repro status

I did not start a new live `pwmd` process and did not change product Rust. This round is a code-level confirmation against the operator terminal snapshot from `cy-cluster-attester.ps1`.

Observed terminal evidence from terminal 2:

- `08:32:10.431`: `snapshot loading started` and `pwmd startup phase: loading_snapshot` for `tmp/state-cy-attester/pwm-data.json` (`terminals/2.txt` lines 724-725).
- `08:32:10.433`: HTTP and peer listener are announced while the node is still in `loading_snapshot` (`terminals/2.txt` lines 726-727).
- `08:32:12.429` through `08:32:14.731`: sync starts from genesis (`mem=0 disk=0`) and applies standby checkpoints `1..32` through `257..288` before snapshot load completes (`terminals/2.txt` lines 728-737).
- `08:32:14.733`: snapshot loader then reports `snapshot startup load ok ... tip_h=544 canonical_h=544` and `ready (snapshot loaded)` (`terminals/2.txt` lines 738-742).
- `08:32:26.291`: after readiness, progress is only `3%` with `mem=544 disk=544` against proposer goal `16401`, then the run is interrupted (`terminals/2.txt` lines 743-745).

This makes the race visible without a new run: peer sync traffic mutates the genesis chain during `InitPhase::LoadingSnapshot`, then the snapshot loader overwrites `app.inner.chain` with the persisted tip `544`.

## Code confirmation

`InitState::is_ready()` only returns true for `Ready` or `ReadyDegraded` (`crates/pwmd/src/state.rs` lines 288-290). Several outbound/scheduler paths honor that gate:

- `spawn_transport_loop` skips ticks while not ready (`crates/pwmd/src/transport/spawn.rs` lines 14-18).
- `spawn_real_transport_loop` also skips while not ready (`crates/pwmd/src/transport/spawn.rs` lines 37-40).
- `run_seed_session` sleeps and continues before any TCP connect when `app.init` is not ready (`crates/pwmd/src/transport/peer_session/seed/mod.rs` lines 24-31).

The inbound listener does not have the same gate. `spawn_peer_listener_loop` binds immediately, accepts sockets immediately, and spawns `process_inbound_socket` directly (`crates/pwmd/src/transport/spawn.rs` lines 54-86). Inside `process_inbound_socket`, the inbound path:

- reads `Hello`, validates it, and writes an accepted `HelloAck` without checking `app.init.is_ready()` (`crates/pwmd/src/transport/peer_session/inbound.rs` lines 38-178);
- builds the local hello using `app.inner.read().await.chain.tip_h()`, which is still the genesis/in-flight chain if the snapshot is loading (`crates/pwmd/src/transport/peer_session/inbound.rs` lines 141-156);
- immediately opens the session, sends cross-shard/account/cluster frames, and enters the steady read loop (`crates/pwmd/src/transport/peer_session/inbound.rs` lines 179-241);
- routes inbound sync frames via `route_sync_stub`, which can apply blocks during the same session (`crates/pwmd/src/transport/peer_session/inbound.rs` lines 313-346).

Snapshot loading is concurrent with this I/O. `spawn_snapshot_loader` sets `InitState::loading` before `backend.load(...)` (`crates/pwmd/src/lifecycle.rs` lines 591-604), later takes a write lock and replaces `g.chain.blocks`, `g.chain.st`, `g.roaming_pool`, and `g.cross_shard` from the snapshot (`crates/pwmd/src/lifecycle.rs` lines 643-648), then flips init to ready (`crates/pwmd/src/lifecycle.rs` lines 651-653). The standby checkpoint log comes from block apply/persist after `apply_blk_batch` sees a standby range crossing the flush interval (`crates/pwmd/src/transport/peer_session/sync_live.rs` lines 993-1037).

## Root cause

The likely root cause for the new 3-5% stall is an init-readiness race on inbound peer sessions. The proposer can dial the attester listener as soon as the listener is announced, while the attester is still `loading_snapshot`. Because `process_inbound_socket` has no readiness gate, it accepts the session and allows sync frames to apply to the genesis chain. A few seconds later, the snapshot loader replaces `app.inner.chain` with the persisted snapshot tip `544`. That replacement can leave already-open peer session state, negotiated hello tip, sync queues/in-flight counters, and remote expectations based on the pre-snapshot chain, while the canonical chain has been reset to the snapshot tail. The terminal ordering at 08:32:10-08:32:14 matches exactly this sequence.

This does not disprove the earlier CUP epoch-clamp suspicion, but it explains why the CUP clamp did not help enough: the attester can enter the post-ready phase with transport/session state contaminated by sync work that happened before the canonical snapshot state was installed.

## Minimal fix directions for `pwm-coding`

1. Add an inbound readiness gate in `crates/pwmd/src/transport/peer_session/inbound.rs` after the remote `Hello` has been validated/rejected, but before building/sending the accepted local `HelloAck` and before any `send_*` or `route_sync_stub` path. This keeps the listener bind behavior unchanged while preventing accepted sessions from advertising a genesis tip or applying sync during `LoadingSnapshot`.
2. If the gate is placed after `HelloAck` for compatibility, it must still run before `peer session open`, `send_cross_shard_facts`, `send_account_views`, `send_cluster_prop`, and the steady read loop. Prefer the earlier placement before local hello construction because `chain_tip_height` in the ack should reflect the snapshot-loaded chain.
3. Consider adding a small helper such as `wait_until_init_ready(app).await` to avoid duplicating the `200ms` sleep loop from `run_seed_session`.
4. Add a defensive gate at the start of `run_seed_initial_exchange` for symmetry. The outer `run_seed_session` already waits before TCP connect, so this is a guard against future direct calls or reconnect refactors rather than the primary fix.
5. Add a regression test that starts inbound processing while `app.init.phase == LoadingSnapshot` and asserts no sync frame is routed/applied until the init state becomes ready. A focused unit/integration test around `process_inbound_socket` is enough; no product behavior should depend on sleeps longer than a bounded poll.

## Debug instrumentation

No temporary Rust instrumentation was added.

- Instrumentation files/hunks: none.
- Reverted: yes.

## Commands and tools run

- Read `docs/AGENT_PROMPT_debug.md` and skill `colloquium-cqds-mcp`: PASS.
- Called CQDS `cq_help` before CQDS search and confirmed `project_id=5`: PASS.
- CQDS registered-index search returned no useful first-page hits for these symbols, so I used project-approved `rg` via shell for exact symbols: PASS.
- Read code: `inbound.rs`, `seed/mod.rs`, `seed/session/mod.rs`, `seed/session/initial_exchange.rs`, `seed/handshake.rs`, `spawn.rs`, `state.rs`, `lifecycle.rs`, relevant `sync_live.rs` section: PASS.
- Read terminal 2 snapshot and prior debug artifact: PASS.
- Live `pwmd` repro: NOT_RUN.
- Product Rust changes: NOT_DONE.

## Orchestrator ticket note

```json
{
  "agent": "pwm-debug",
  "result": "PASS",
  "verbosity_focus": "transport:sync",
  "instrumentation": {
    "files": [],
    "hunks": 0,
    "reverted": "yes"
  },
  "repro": {
    "commands": [
      "./cy-cluster-attester.ps1 (operator terminal evidence)"
    ],
    "deterministic": "not rerun by agent; terminal ordering directly shows inbound/sync activity during loading_snapshot"
  },
  "artifacts": [
    "tasks/20260517-attester-sync-stall-at-4pct-debug-round2.md"
  ],
  "commands": [
    "CQDS help/list_projects/start_grep: PASS (start_grep first page unhelpful)",
    "rg exact symbol map: PASS",
    "code/terminal read: PASS",
    "live pwmd run: NOT_RUN"
  ],
  "cleanup": {
    "cleaned": "yes",
    "killed": []
  },
  "token_usage": {
    "source": "estimate",
    "input": 31000,
    "output": 5200,
    "total": 36200,
    "confidence": "medium"
  }
}
```
