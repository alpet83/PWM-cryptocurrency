# 2026-06-08 Rollback: coalesce + attest wake (pwmd 0.1.66 → 0.1.65 baseline)

## Why

Owner reported worse suppression stats after debug cycle. Last committed throughput experiment (`543c997`, marker **0.1.66**) added:

1. **Propose coalesce** — `sent_key_by_node` suppresses duplicate `cluster_propose` per `(height, round)` per remote node.
2. **Attest wake** — `seal_wake.notify_waiters()` + `select!(sleep | notified)` in seal loop pre-deadline wait.

Uncommitted debug patches (NOT kept) from `20260607` experiments:

- Resend propose while round has zero attestations.
- Clear `sent_key_by_node` on `record_peer_close()`.

## Debug variants reference (what was tried)

| ID | Ticket / doc | What changed | Outcome |
|----|----------------|--------------|---------|
| V0 | `20260606` iterative | Resumed ~34k, no code change | T100~158s, pending p50~330, struck~57% |
| V1 | `20260606` | CleanState fresh chain | T100~108s, pending~45, struck~1% |
| — | `20260604` coding | `gate_recheck` same-iteration seal after deadline | Owner: mean ~110s, bad tails ~135s |
| **543c997** | `20260607` coding | Coalesce + attest wake (0.1.66) | Suspected suppression regression |
| E1/E0 | `20260607` experiments | Temporary resend + close dedup clear | Fresh OK; resumed partial only |

## Rollback applied (working tree)

**Kept (0.1.65-era behavior):**

- Variant C deadline scheduler, `gate_recheck_needed`, `ClusterGateDedup`, interval suppression metrics.
- `seal_ahead_ms` + `cluster_prop_nudge` + `try_prop_nudge` (propose-only before grid deadline).
- `record_cluster_prop_tick` for ahead window.

**Removed:**

- Wire propose coalesce (`sent_key_by_node` gate in `send_cluster_prop`).
- Attest-driven seal loop wake (`seal_wake` still in `App` but unused).
- Debug experiment resend/close patches.

**Marker:** `pwmd` crate version set back to **0.1.65**.

## Rebuild

```powershell
taskkill /F /IM pwmd.exe /T 2>$null
cargo build -p pwmd
```

Restart attester then proposer (`cy-cluster-*.ps1`).

## Follow-up

- Re-open `20260607-v5-resumed-state-seal-throughput-coding` only with measured A/B on resumed ~34k state.
- Do not run `CleanState` on long-lived chain without `devnet_state_backup.ps1`.
