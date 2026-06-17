# RFC 0009: Cross-Domain Roaming MVP Status (Sprint 13)

**Status:** Active (MVP baseline implemented in Sprint 13)  
**Version:** 0.1  
**Scope:** Runtime/ops contract for current roaming MVP (`roaming-intent` + `EXPORT/IMPORT` fallback), not full roaming target model.

## 1. Purpose

This RFC documents the **current as-implemented roaming MVP** after Sprint 13, so operators and reviewers can distinguish:
- what is implemented and stable now;
- what is intentionally out-of-scope for this cut.

It complements:
- `docs/WHITE_SPEC_v0.md` (protocol baseline and boundaries; §7.4–§7.5: stabilization, future lock direction Appendix A.5, **bridge trust refusal** / one-window closure Appendix A.6),
- `docs/rfc/3-cross-domain-roaming.md` (broader target model draft; §5.3 Mode B vs as-implemented MVP),
- `docs/ROAMING-SAMPLE.md` (operator runbook with happy/negative checks),
- `docs/GEO-SHARDING-EXPLANATION.md` (plain-language model explanation).

## 2. Implemented MVP baseline

- Cross-domain transfer path is explicit: `EXPORT` on source node, then `IMPORT` on target node.
- Source runtime records export provenance in `exported_registry` keyed by deterministic `export_id`.
- Target/runtime accepts `IMPORT` only when:
  - `export_id` is known for the current flow;
  - `to`, `amount`, `target_domain` match recorded provenance;
  - `export_id` has not been consumed before (`imported_set` replay guard).
- Duplicate `IMPORT` is rejected deterministically and does not mutate state.
- Replay/provenance state survives restart via snapshot persistence (`exported_registry` + `imported_set`).

## 3. Intentionally out-of-scope (this MVP)

- Admission/compliance certificate layer.
- **Note (Sprint 15 update):** operator-transparent **relay of handoff** and **client-driven Import submit** after `relayed` are **in scope** for one-window UX; the historical bullet «automated operator handoff» referred to a fully manual social handoff without `pwmd` relay — still out-of-scope as the *only* path, but no longer the whole story.
- Advanced finality/quorum profiles beyond minimal baseline.
- Async decoupled ingestion pipeline for `EXPORT/IMPORT` (currently sync request path).

## 4. API / runtime contract (compressed)

Ingress:
- `POST /v1/roaming-intents` is the home-shard intent create API for one-window client flow.
- `GET /v1/roaming-intents/:id` is lifecycle status API.
- `POST /v1/roaming-intents/:id/finalize` is explicit operator handoff (`queued|exported -> relayed`) and is idempotent for retries/terminal statuses (`changed=false`).
- `GET /v1/flow/recent` exposes bounded runtime trace for diagnostics (`accepted:*`, `applied:*`, `exported:*`, `imported:*`, `sealed:*`, `roaming_status:*`, `finalized:*`).
- `POST /v1/tx` keeps direct `EXPORT/IMPORT` fallback/debug path.

Status mapping:
- Success -> `204 NO_CONTENT`.
- Duplicate import (`DuplicateImport`) -> `409 CONFLICT`.
- Invalid/unknown import provenance (`InvalidImport`/unknown `export_id`) -> `400 BAD_REQUEST`.

Execution model:
- `EXPORT/IMPORT` are applied on request path (`apply_tx` then `seal([])`), not queued in mempool-only flow.
- **Import delivery:** for foreign Import submitted to **source** RPC, `pwmd` may forward the signed Import to **target** via HTTP relay (`relay_import`); completion on source may mirror roaming state after successful delivery. Polling intent status alone does not apply Import — a **`POST /v1/tx`** with Import body is required (CLI/TUI automation after `relayed`).
- `finalize` writes `finalized:roaming_intent` and, on actual transition, `roaming_status:relayed` into `flow/recent`.
- Runtime publishes bridge counters in `GET /v1/status`:
  - `bridge_exported_registry_size`,
  - `bridge_imported_set_size`.

## 5. Operator contract (MVP)

- Roaming has two operator-visible modes:
  1. **one-window client mode** (CLI/TUI): create/poll roaming intent from home shard;
  2. **manual fallback mode**: `EXPORT` on source -> handoff -> `IMPORT` on target.
- Retry policy:
  - repeated `IMPORT` for same `export_id` is expected to fail with `409` (idempotent reject);
  - wrong target/provenance should fail with `400`, then operator must re-check source/target pairing.
- Client UX contract for one-window path (`tx-send` / `F6 send`):
  - `duplicate` / already-created intent must map to deterministic "already started/reused" message;
  - invalid request/provenance must map to deterministic "invalid request" message with optional details;
  - expired lifecycle must map to deterministic "expired, retry from home shard" message.

### 5.1 One-window path — scalability caveat (MVP only)

The **one-window** operator/client pattern (home RPC + roaming intent lifecycle + optional **trusted peer** path for foreign account observability) is **intentionally simple** for MVP/devnet.

It is **not designed to scale** under mass usage: frequent polling and pushing observability traffic through nodes and peer sessions can **overload the network** and node RPC layers compared with a dedicated read tier.

**Post-MVP direction:** prefer optimized **centralized or semi-centralized read services** (e.g. a **global explorer**) with **client subscriptions** to address-level updates, rather than scaling “every wallet polls every shard via peer plumbing” as the primary model.

## 6. Security considerations

- Replay protection depends on durable `imported_set`; snapshot integrity is critical.
- Provenance matching (`export_id` + payload fields) reduces forged-import risk in MVP profile.
- Manual handoff is a social/operational trust boundary; leakage or misrouting of material can cause failed imports and operational confusion.
- Sync request-path sealing increases DoS sensitivity vs fully async ingestion; acceptable for current MVP/devnet envelope.

## 7. Open issues

- Define canonical machine-readable handoff envelope to reduce operator error.
- Decide post-MVP path for sync vs async sealing of roaming operations.
- Specify admission integration boundary with minimal upgrade pain.
- Tighten observability for roaming retries/failures beyond current counters.

## 8. Compatibility note

This RFC does not replace RFC 0003 target architecture.  
It freezes the **current MVP status contract** for Sprint 13 closeout and immediate follow-up operations.

## Appendix A. MVP stabilization delta (2026-05)

### A.1 Deterministic target provenance (implemented MVP profile)

- MVP deterministic path uses `Import` with embedded provenance as the accepted target-side contract.
- `POST /v1/export-provenance` (`handoff_register`) is transport/pending material only and MUST NOT mutate replay-critical `State.exported_registry`.
- `State.exported_registry` on target is mutated only during sealed block application of validated `Import`.
- Replay-critical import validation is derived from genesis + blocks without snapshot-only provenance seeding hacks.

### A.2 Automatic reimport/backfill after cleanup/rollback (implemented MVP profile)

- Recovery backfill accepts facts only from trusted peers passing network trust gate (`network_id` and `genesis_hash` compatibility).
- Backfill contract is batch-oriented and explicit: tx-path outcome with counters (`discovered`, `imported`, `skipped`, `rejected`, `untrusted`).
- Inclusion is idempotent: already consumed imports do not mutate balances and are counted as non-mutating outcomes.

### A.3 Offline repair and crash-fast operator contract (implemented MVP profile)

- Offline repair is provided by `pwmd-snap-repair` and follows backup-first workflow.
- Supported operator modes:
  - explicit target height (`--to-height`),
  - auto-select last reproducible height (`--auto-last-good`).
- Repair rewrites epochs/manifest/summary consistently and requires validate-after-write before restart.
- Runtime default on fatal snapshot mismatch is crash-fast/degraded-safe behavior; repair is an explicit offline operator action.

### A.4 Future settlement/import-export chain (non-MVP stage)

- Dedicated settlement/import-export chain remains a next-stage architecture option for stronger global ordering/finality.
- It is explicitly non-blocking for current MVP stabilization acceptance and not required for Slice A-D closeout.

### A.5 Proposed protocol upgrade: source-side lock / conditional finalization

> **V6 normative:** Mode B escrow is specified in [addenda/v6-rfc9-mode-b-escrow.md](addenda/v6-rfc9-mode-b-escrow.md). The «not implemented» posture below applies to pre-V6 MVP only.

**Intent (future):** treat `EXPORT` on the source shard as **locking** the corresponding economic capacity (UTXO value committed into a conditional / escrow-like output) until either:

- the target shard produces an agreed **finalization signal** importable into source consensus state (proof, attestation, or settlement-chain fact — TBD), or
- a **timeout** expires and a **refund** path releases funds back to a defined refund policy.

This model aligns with cross-chain atomicity patterns (conditional outputs, two-phase commit semantics, HTLC-like timeouts) and would tighten the invariant «no spend on source until destination outcome is known or timeboxed».

**Current MVP fact (as implemented):**

- There is **no** source-side lock/escrow policy layer and **no** protocol-level gate tying source finality to target finality beyond deterministic provenance + replay checks on `IMPORT`.
- Target nodes apply/sign validation for `IMPORT` **unconditionally** within the implemented rules (known provenance, replay guards, trust gates for relay/backfill). There is **no** staged «pending lock → release on proof» state machine at the protocol boundary.

**RFC posture:**

- Record this appendix as a **design direction** for a future protocol revision (possible companion to Appendix A.4 settlement chain).
- **Do not implement** lock/escrow semantics in core until: (1) finalization proof interface is specified, (2) timeout/refund and griefing boundaries are specified, (3) compatibility/fork rules are agreed — premature implementation would fork economics and operator behavior without a stable policy layer.

### A.6 Bridge federation trust refusal (normative product/spec; implementation evolves)

**Two-level accounting (must not be conflated):**

- **Level 1 — intra-shard ledger:** balances, emission, burns, same-shard transfers. Totals **will differ** across shards by design; comparing raw balances across shards is **not** a federation invariant.
- **Level 2 — cross-shard movement ledger:** export/import facts, consumed-import set, provenance-linked `export_id` consistency. This is the **federation-facing** accounting slice.

**Trust refusal:** If the node detects **divergence in level-2 views** relative to an agreed trust anchor (e.g. **bridge commitment** mismatch vs a peer that already passed the network trust gate `network_id` / `genesis_hash`; **for same `domain_hi` / replica paths**, compare the peer’s advertised digest to the local digest; **cross-shard** peers do **not** compare local vs remote digest — level-2 maps differ by shard by design), or replica mismatch on the same tip height, the node enters an explicit **bridge trust refusal** state. Causes include partition, stale mirrors, replication skew, or hostile/stale peer data — the runtime cannot honestly present a unified cross-shard story.

**One-window closure:** While bridge trust is refused, the **one-window client service** (home RPC + roaming intent flow + **foreign-shard balance/observability** as if the federation were healthy) **must not** expose actionable foreign-shard balance state to wallet clients. Responses should be refusal/diagnostic for operators, not a plausible «green» cross-shard UX.

**Bridge commitment (as implemented in `pwm-core`):** Type `BridgeFederationCommitment` — only `imported_set` and `exported_registry` (no `accounts`, `fee_pool`, or other level-1 fields). Digest: `hex(blake3(bincode(BridgeFederationCommitment)))`. **Replica / same-`domain_hi` hello:** local and remote digests **must** match after `network_id` / `genesis_hash` gates. **Cross-shard hello:** do **not** require equality with the local digest (different shards hold different level-2 rows); refusal is driven by same-shard mismatch, optional relay/HTTP checks, and operator reset (`POST /v1/bridge-federation/reset` on `pwmd`).

**Consensus wording:** In-shard chain consensus remains local; this appendix adds a **federation readiness** dimension — chain may advance while cross-shard **client trust** is withheld until level-2 agreement is restored.

**Implementation posture:** Specify readiness flags / HTTP codes in runtime docs as features land; until then, `docs/WHITE_SPEC_v0.md` §7.5 and this appendix are the **normative intent** for product behavior.
