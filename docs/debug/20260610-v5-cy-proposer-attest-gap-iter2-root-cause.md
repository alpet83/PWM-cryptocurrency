# V5 CY proposer attest gap iter-2 root cause

**Ticket:** `20260610-v5-cy-proposer-attest-gap-iter2-debug`  
**Agent:** `pwm-debug`  
**Date:** 2026-05-31  
**Verdict:** PASS - root cause diagnosed. Do not mask with `max_tip_lag=2` or log dedup.

## Executive verdict

The iter-1 heartbeat fix is applied: both observed proposer and attester launch paths report `heartbeat_interval_ms=1000`. The remaining failure is two coupled protocol/runtime issues:

1. **Primary cause for persistent lag/pending ticks:** proposer cluster preflight treats the attester's `sync_live.tip_h` as a hard per-block readiness signal, but `sync_live.tip_h` is the attester's own advertised/applied chain tip and is updated on the live sync heartbeat/request/apply loop. Cluster attest itself does not require the attester to have applied the proposed parent; it signs the proposal immediately when received. That lets the proposer seal ahead while the attester's advertised tip trails by a stable two-block transport/apply pipeline delay. With `max_tip_lag=1`, this repeatedly flips `cluster_attest_ready` -> `cluster_attest_waiting_sync`, inflating `cluster_gate_pending_summary`.
2. **Primary cause for `ready` then `quorum_timeout got=0`:** the proposer can open local round-state and start `propose_opened_at_ms` from `record_cluster_prop_tick`/seal-ahead before a fresh wire proposal has actually yielded an attester ACK. On startup, this produced `elapsed_ms=6907` at the first real `cluster_attest_ready`; in repro, the same stale-open/no-ACK path produced multiple `got=0` timeouts at the nominal 2s limit. This is not fixed by raising tip lag; the timeout clock and resend semantics are wrong for a round that has no accepted attestation.

## Repro

Command used from `P:/opt/docker/PWM-cryptocurrency`:

```powershell
$env:RUST_BACKTRACE='full'
$env:RUST_LIB_BACKTRACE='1'
$env:RUST_LOG='pwmd::lifecycle=debug,pwmd::peer=debug,pwmd::sync=debug,pwm_core::state=info'
powershell -File tasks/20260610-v5-cy-proposer-attest-gap-iter2-debug-evidence/run_repro.ps1
```

Result: deterministic enough for this ticket. The 5-minute attester-first repro reproduced symptom A/C and produced symptom B-style `got=0` timeouts.

- `repro-proposer`: waiting_sync=40, lag_counts={`2`: 39, `65500`: 1}, timeouts=5, pending_max=380.
- `repro-attester`: short-tail `sync_catchup_stall rem=1` while proposer continued sealing; no product instrumentation was added.

## Evidence

Artifacts:

- `tasks/20260610-v5-cy-proposer-attest-gap-iter2-debug-evidence/summary.json`
- `tasks/20260610-v5-cy-proposer-attest-gap-iter2-debug-evidence/correlation.md`
- `tasks/20260610-v5-cy-proposer-attest-gap-iter2-debug-evidence/grep-snippets.md`
- `tasks/20260610-v5-cy-proposer-attest-gap-iter2-debug-evidence/repro-proposer-stdout.log`
- `tasks/20260610-v5-cy-proposer-attest-gap-iter2-debug-evidence/repro-attester-stdout.log`

Key source anchors:

- `crates/pwmd/src/lifecycle.rs:300-316` reads proposer local tip and counts ready attesters from handshake `sync_live`.
- `crates/pwmd/src/lifecycle.rs:374-385` requires `local_h - peer_h <= max_lag` for sync-ready.
- `crates/pwmd/src/lifecycle.rs:1234-1248` seal-ahead can call `record_cluster_prop_tick` before the normal gate path.
- `crates/pwmd/src/lifecycle.rs:1316-1336` suppresses sealing on the hard sync-ready preflight.
- `crates/pwmd/src/lifecycle.rs:1397-1403` treats cluster gate failure as a pending tick and loops.
- `crates/pwmd/src/transport/peer_session/mod.rs:505-510` records local proposal round-state without wire-send confirmation.
- `crates/pwmd/src/transport/peer_session/mod.rs:618-640` attester signs a received proposal without checking local applied parent/tip.
- `crates/pwmd/src/transport/peer_session/mod.rs:765-807` proposer records accepted attest only after a matching `ClusterAttest` arrives.
- `crates/pwmd/src/transport/peer_session/sync_live.rs:290-335` sends `SyncTipAnnounce` from the node's current chain tip.
- `crates/pwmd/src/transport/peer_session/sync_live.rs:629-656` updates `sync_live.tip_h` from peer tip announce.
- `crates/pwmd/src/transport/peer_session/sync_live.rs:800-818` live tail asks for headers after seeing lag.
- `crates/pwmd/src/transport/peer_session/sync_live.rs:1405-1431` block apply advances local chain and then pulls the next tail.

## Timestamp correlation

### Symptom B, original `082709` / `085107`

- Proposer startup: `logs/2026-05-31/pwmd-cy-proposer-082709.log:4` has `heartbeat_interval_ms=1000`.
- Attester startup: `logs/2026-05-31/pwmd-cy-attester-085107.log:4` has `heartbeat_interval_ms=1000`.
- Attester listens before snapshot is ready: `08:51:08.132` listening, then `08:51:16.303` snapshot load OK at `tip_h=65300`, then `08:51:16.304` ready.
- Proposer receives sync progress at `08:51:16.305`, logs `cluster_attest_ready` at `08:51:16.461`, and immediately times out height `65301` with `elapsed_ms=6907 limit_ms=2000 got=0`.

Interpretation: the round-state timeout was already aging before the attester was actually able to participate. The first real ready check inherited stale `propose_opened_at_ms` and had no attestation in the round map.

### Symptom A/C, original `101620` and repro

- Original `101620`: waiting_sync=139, lag_counts={`2`: 138, `65300`: 1}, pending_max=336.
- Bounded repro: waiting_sync=40, lag_counts={`2`: 39, `65500`: 1}, pending_max=380.
- Attester repro stalls at `rem=1` (`sync_catchup_stall`), while proposer repeatedly observes `attester_tip_max = proposer_tip - 2` when opening the next gate.

Interpretation: attester sync is close but phase-lagged. The quorum ACK path and the sync apply path are not the same readiness signal, so the hard `max_tip_lag=1` gate creates oscillation and high pending ticks even with equal heartbeat interval.

## Hypothesis results

| ID | Verdict | Result |
|---|---|---|
| H1 | CONFIRMED | `sync_ready` uses `hs.sync_live.peers[node].tip_h` and proposer local tip. That value is the peer's advertised/applied tip and is phase-lagged behind cluster attest readiness. |
| H2 | CONFIRMED for B, mechanism narrowed | `got=0` means no matching `ClusterAttest` reached proposer round-state. Logs show no drop/binding/signature warnings; code shows local round opens before ACK and the timeout starts from local record time. Wire-level LLDB was unavailable in this session. |
| H3 | CONFIRMED | Attester applies through live sync request/response after proposer tip announce. Repro kept `rem=1` stalls and repeated proposer-visible lag=2 while connected. |
| H4 | REJECTED | Both original and repro proposer/attester logs show `heartbeat_interval_ms=1000`; launchers do not override it back to 1500. |
| H5 | CONFIRMED | Proposer gates for `tip+1`, while attester advertised/applied tip is often `proposer_tip-2`. Attestation signing itself does not prove the attester has applied that parent. |

## Primary root cause

The cluster gate currently conflates two different signals: transport-level attestation availability and chain-sync apply freshness. The attester can sign a `ClusterPropose` immediately, but the proposer's preflight uses the attester's independently advertised sync tip as a strict freshness gate. Because sync tip follows a heartbeat + header/block request + apply loop, it trails the actively sealing proposer by about two blocks under normal operation. Separately, seal-ahead/local proposal recording starts the quorum timeout clock before a fresh wire ACK is guaranteed, causing `cluster_attest_ready` followed by `quorum_timeout got=0` when startup or transport phase delay leaves the local round without an attestation.

## Follow-on coding tickets

1. `20260610-v5-cluster-gate-attester-apply-readiness-coding` - Replace the hard `sync_live.tip_h <= max_tip_lag` preflight with a protocol-valid readiness signal: either attest only after local parent apply, or make proposer readiness use an explicit attester-applied/parent-ack marker tied to the proposed height.
2. `20260610-v5-cluster-propose-timeout-and-resend-coding` - Start `propose_opened_at_ms` on confirmed wire proposal send, not local seal-ahead record, and allow bounded resend/reopen when a round has `got=0` and no attester ACK.

## Cleanup and instrumentation

- Temporary product instrumentation: none.
- Instrumentation reverted: yes, nothing to revert.
- Processes cleaned: yes, `pwmd`/`pwm-tui` check returned no live processes after repro.
- `lldb MCP`: requested method noted, but MCP servers available to this subagent exposed only `user-git`; no LLDB MCP endpoint was available. Logs/code/repro were sufficient for the root-cause verdict.

## Orchestrator copy block

```yaml
agent: pwm-debug
result: PASS
verbosity_focus: seal:cluster-gate; secondary transport:peers, transport:cluster-attest via scoped RUST_LOG
instrumentation: product files 0 hunks; reverted: yes
repro: powershell -File tasks/20260610-v5-cy-proposer-attest-gap-iter2-debug-evidence/run_repro.ps1; deterministic: partial/high, reproduced A/C and B-style got=0 timeouts in one 5m run
artifacts:
  - docs/debug/20260610-v5-cy-proposer-attest-gap-iter2-root-cause.md
  - tasks/20260610-v5-cy-proposer-attest-gap-iter2-debug-evidence/summary.json
  - tasks/20260610-v5-cy-proposer-attest-gap-iter2-debug-evidence/correlation.md
  - tasks/20260610-v5-cy-proposer-attest-gap-iter2-debug-evidence/grep-snippets.md
  - tasks/20260610-v5-cy-proposer-attest-gap-iter2-debug-evidence/repro-proposer-stdout.log
  - tasks/20260610-v5-cy-proposer-attest-gap-iter2-debug-evidence/repro-attester-stdout.log
commands:
  - read ticket/debug prompt/skill: pass
  - CQDS MCP schema/help attempt: blocked, server unavailable in subagent; fallback rg/scripts used
  - original log correlation: pass
  - bounded 5m repro: pass
  - cleanup process check: pass
cleanup: cleaned yes; stopped pre-existing pwmd before repro and no pwmd/pwm-tui remained after repro
token_usage: { source: estimate, input: null, output: null, total: 52000, confidence: low }
```
