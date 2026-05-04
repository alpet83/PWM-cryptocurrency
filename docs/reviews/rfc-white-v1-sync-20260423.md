# RFC/White Sync Review for MVP SPEC v1 Testnet

Status: draft for architecture discussion  
Date: 2026-04-23  
Primary baseline: `docs/WHITE_SPEC_v0.md`  
Target: `MVP SPEC v1 testnet` (>=2 independent shards + coin transfer between shards)

## Agreed Constraints

- State model: **account-based extension** (no protocol pivot to UTXO core).
- Evolution strategy: **strict upgrade** from v0 (minimize breaking changes in tx/wallet/RPC).
- New capability for v1 testnet: **multi-shard operation** and **cross-shard coin transfer**.

## Contradiction Matrix and Branch Decisions

| Topic | Current conflict | Branch A | Branch B | Decision (for v1) | Rationale |
|---|---|---|---|---|---|
| Ledger model | `WHITE_SPEC_v0` account-state vs RFC7 UTXO core | Keep account ledger and extend for shards | Pivot to UTXO core | **A** | Preserves strict upgrade and existing v0 behavior |
| Cross-shard transfer | RFC3/7 assumes UTXO Export/Import types | Account-ledger Export/Import envelopes | Reuse UTXO outputs (`XFER_OUT`) | **A** | Adds roaming semantics without rewriting state kernel |
| Transfer semantics | Cross-domain implied in generic transfer paths | `TRANSFER` stays same-shard; cross-shard uses explicit `EXPORT/IMPORT` | Single tx auto-routing | **A** | Deterministic validation and compatibility with existing tx parser |
| Finality proof depth | RFC4 uses >=2/3 cert as hard MVP | Minimal cert profile for v1 testnet, upgradable later | Full cert stack now | **A** | Delivers shard trust bridge with lower implementation risk |
| Policy depth | RFC6 requires cosign/membership as default MVP | Minimal recipient/domain rules in v1 MVP; advanced policy as extension hooks | Full policy engine now | **A** | Avoids blocking testnet milestones while preserving future path |

## Compatibility Rules for v1

1. Existing v0 transaction types remain valid and semantically stable on single-shard paths.
2. New cross-shard flow is explicit and additive (`EXPORT`, `IMPORT`), not a hidden behavior change for `TRANSFER`.
3. Wallet and CLI/TUI UX keep wallet-first signing and recipient-policy baseline; new flags/commands may be additive.
4. RPC may add endpoints/fields for shard and roaming, but existing v0 clients should keep working on local-shard operations.

## Tiered Sync Scope

- Tier A (normative): `WHITE_SPEC_v0`, `RFC0003`, `RFC0007`.
- Tier B (control maps): `DEPEDENCY_GRAPH`, `WHITEPAPER_COVERAGE_MATRIX`.
- Tier C (supporting RFCs): `RFC0002`, `RFC0004`, `RFC0006`.

## Acceptance Criteria for Discussion Sync

- No document claims UTXO as mandatory core for v1.
- No document implies breaking replacement of v0 account model.
- Cross-shard transfer path is clearly specified as additive flow.
- Finality/policy sections distinguish `v1 MVP minimal` vs `post-v1 advanced`.
