# Sprint 4 Test Report (Spec Validation Gate)

**Sprint:** `Sprint 4 / Shard Runtime Identity and Native-Peer Priority`  
**Date:** `2026-04-24`  
**Scope:** spec-only validation for `RFC-8` implementation readiness (no runtime code changes)

## 1) Verdict

`partial`

Rationale: acceptance criteria are mostly testable and internally consistent, but readiness is partial because runtime implementation and executable probes do not yet exist for evidence-based pass confirmation.

## 2) Validation Matrix (RFC-8 AC 1:1)

| AC | Requirement | Validation clause (pass/fail) | Evidence source |
|---|---|---|---|
| AC-1 | Runtime requires explicit cluster/network/node identity fields at launch | **Pass** if launch contract rejects missing required fields (`network_id`, `domain_hi`, `cluster_id`, `node_id`, `node_pubkey`, capability set); **Fail** if defaults/implicit inference are accepted | `docs/rfc/8-shard-runtime-identity-and-peering.md` §4.1, §4.2 |
| AC-2 | Handshake includes signed identity envelope and capabilities | **Pass** if `NodeHello` carries identity + capabilities + `nonce` + `timestamp_ms` + signature over full envelope; **Fail** if any mandatory field can be omitted without rejection | RFC-8 §5.1, §8.1 |
| AC-3 | Peer class is deterministic by `domain_hi` equality only | **Pass** if `native/foreign` derives only from `peer.domain_hi == local.domain_hi`; **Fail** if `cluster_id`, endpoint naming, IP tags, or ranges alter class outcome | RFC-8 §6, §8.2 |
| AC-4 | Priority policy covers queue, reconnect/backoff, gossip weighting, degraded failover | **Pass** if class-aware policy exists for all four subareas with native-first bias and explicit degraded-mode behavior; **Fail** if any subarea is undefined/unobservable | RFC-8 §7.2–§7.5 |
| AC-5 | Anti-spoof checks enforce signature, replay window, network/genesis compatibility | **Pass** if handshake gate rejects invalid signature, replayed nonce, skew violations, network/genesis mismatch; **Fail** if any check is advisory-only | RFC-8 §5.2, §8.1, §8.3 |
| AC-6 | Minimal metrics/logs emitted and documented | **Pass** if all mandatory metrics/log categories are defined and wired as acceptance signals; **Fail** if reasons/events lack reason-code visibility | RFC-8 §9 |
| AC-7 | `--shard A|B` migration compatibility documented and tested | **Pass** if alias mode mapping is deterministic, explicitly transitional, and test plan covers backward-compat path; **Fail** if alias acts as heuristic routing truth | RFC-8 §10 |
| AC-8 | No contradiction with `WHITE_SPEC_v0` and RFC-6 shard semantics | **Pass** if fixed-`domain_hi` shard semantics and route invariants remain aligned; **Fail** on semantic drift or policy conflict | RFC-8 §11.8 + `docs/WHITE_SPEC_v0.md` §7.2/§7.3 + `docs/rfc/6-policy-engine.md` §7.0/§7.1 |
| AC-9 | No range heuristics (`0x80 split` or analogs) in identity/routing/priority logic | **Pass** if explicit prohibition is preserved in identity + routing + policy paths; **Fail** if any range partition appears in normative logic or migration mode | RFC-8 §3, §11.9 + White spec §7.2 + RFC-6 §7.0/§7.1 |

## 3) Mandatory Negative Scenarios

1. **Spoofed identity signature**
   - Setup: valid envelope fields, forged signature or mismatched `pubkey`.
   - Expected: handshake reject; `p2p_handshake_reject_total{reason="bad_signature"}` increments; reason-coded reject log present.

2. **Replay nonce inside replay window**
   - Setup: resend previously accepted `NodeHello` (`nonce` reused, timestamp still plausible).
   - Expected: handshake reject; reason `replay_nonce`; no peer promotion to connected set.

3. **Network/genesis mismatch**
   - Setup: peer advertises different `network_id` or non-matching `genesis_hash`.
   - Expected: hard reject before class assignment; reason `network_mismatch` or `genesis_mismatch`.

4. **Forged native claim**
   - Setup: peer advertises attractive `cluster_id`/metadata but signed `domain_hi` differs from local.
   - Expected: classified `foreign`; no native budget privileges; suspicious event log if claim inconsistency is detected.

5. **Priority regression under native deficit**
   - Setup: `native_min_live` breach with available foreign peers.
   - Expected: temporary foreign borrowing allowed, but native recovery attempts continue and degraded signal is emitted; fail if scheduler stops native recovery or permanently demotes native priority.

## 4) Observability Assertions (Pass/Fail Signals)

### Metrics assertions

- `p2p_peers_connected{class}`: **Pass** if class split is non-empty/consistent with scenario; **Fail** if class labeling is absent or contradictory.
- `p2p_dial_attempts_total{class,result}`: **Pass** if native attempts remain preferred under normal mode and visible during recovery; **Fail** if no class dimension or no ordering evidence.
- `p2p_handshake_reject_total{reason}`: **Pass** if each negative scenario maps to stable reason code; **Fail** if rejects are silent/undifferentiated.
- `p2p_reconnect_backoff_seconds{class}`: **Pass** if native backoff envelope is tighter than foreign (configurable but ordered); **Fail** if equalized or inverted unintentionally.
- `p2p_gossip_msgs_sent_total{class,topic}`: **Pass** if native weighting is higher but foreign remains non-zero; **Fail** if foreign is starved in non-degraded normal operation.
- `p2p_native_degraded_state`: **Pass** if toggles to `1` on native underflow and returns to `0` after recovery; **Fail** if stuck or never asserted.

### Log assertions

- Startup summary log includes identity tuple and capability fingerprint.
- Handshake accept/reject logs include peer identity tuple + reason.
- Native budget underflow/recovery events are explicit and correlatable to `p2p_native_degraded_state`.
- Capability downgrade/incompatibility attempts are explicitly logged.

## 5) Readiness Gate Notes for Implementation Sprint

- **Gate status:** `partial` (ready to implement with explicit testing obligations).
- **Blocking for future PASS:** runtime must emit reason-coded handshake rejects and class-labeled metrics exactly as specified.
- **Hard-fail invariant:** any reintroduction of `domain_hi` range heuristics (`0x80 split` or analogs) blocks merge.
- **Cross-doc risk to watch:** ensure migration alias `--shard A|B` remains operator convenience only and never mutates protocol truth.

