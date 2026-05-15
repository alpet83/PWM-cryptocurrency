# V2-8 RFC nits closure note

**Date:** 2026-05-08  
**Scope:** docs-only wording closure for remaining RFC nits before automated waves.

## What changed

1. In `docs/rfc/15-same-shard-sync-v1.md` (§7.1), proposer baseline wording now contains explicit **MUST NOT** for non-deterministic sources:
   - `avg_peer_count`,
   - first-seen / chat order / arrival order,
   - time modulo peers.
2. In `docs/rfc/15-same-shard-sync-v1.md` (§7.3), `finalized_height` policy is explicit for MVP waves:
   - receiver-local PoA finalized prefix is the source-of-truth baseline,
   - local baseline and per-peer advertised value are monotonic,
   - peer regressions are treated as stale and ignored without rollback side effects,
   - remote value remains bounded by validation and `<= remote_head_height`.
3. In `docs/rfc/15-same-shard-sync-v1.md` (§6), added explicit sentence that v1 does **not** require mandatory per-frame wire `net_zone`; zone policy is profile/segment-level (§13).
4. In `docs/plans/mvp_v2.md` (Sprint V2-8 key decisions), added a tiny traceability note for wave baselines: `finalized_height` source-of-truth and stale regression behavior are tied to RFC 0015 wording.

## Why

- Close residual P0 wording ambiguity from nit register and doc-slice review without expanding protocol scope.
- Improve operator/test-wave predictability for fork-choice inputs in MVP v1.
- Keep zone semantics explicit while preserving existing wire contract boundaries.

## Traceability

- Inputs:
  - `docs/reviews/20260508-v2-8-docslice-p0-finalized-zones-review.md`
  - `docs/reviews/20260508-v2-8-nits-register.md`
  - `docs/rfc/15-same-shard-sync-v1.md`
  - `docs/plans/mvp_v2.md`
- Commit intent: `docs(rfc): close remaining V2-8 RFC nits before waves`
