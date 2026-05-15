# Whitepaper Coverage Matrix (v1 Testnet)

**Status:** Working document  
**Purpose:** Keep `MVP SPEC v1 testnet` aligned with strict-upgrade from `WHITE_SPEC_v0` while preserving future extensibility.

---

## 1. Legend

| Column | Meaning |
| --- | --- |
| MVP Now | Must exist in v1 testnet baseline |
| Reserve Now | Must be anticipated by interfaces/storage, but can be partial |
| Defer | Post-v1 feature, not required for current milestone |

---

## 2. Matrix

| Feature | MVP Now | Reserve Now | Defer | Notes |
| --- | :---: | :---: | :---: | --- |
| Domain-aware addresses (`bech32DX`) | Yes | ? | ? | Routing and recipient policy basis |
| Pretty + canonical forms | Yes | ? | ? | UX and interoperability |
| Account-based state core | Yes | ? | ? | Strict-upgrade from v0 |
| v0 tx set (`INIT/TRANSFER/STAKE/UNSTAKE/BURN_MARK`) | Yes | ? | ? | Must stay semantically compatible |
| Two independent shards | Yes | ? | ? | Minimum maturity target for v1 testnet |
| Explicit `EXPORT/IMPORT` cross-shard flow | Yes | ? | ? | Additive to local transfer path |
| Protocol-derived shard routing (`domain_hi` compare) | Yes | ? | ? | No forced route-mode parameter |
| Replay protection (`ImportedSet` or equivalent) | Yes | ? | ? | Critical safety requirement |
| Finality proof (minimal profile) | Yes | Yes | ? | Upgrade path to stricter models |
| Static validator set (per shard) | Yes | ? | ? | Dynamic rotation is deferred |
| Minimal recipient/domain policy | Yes | ? | ? | Reject invalid recipient classes |
| Unified `Account.marks` burn path (legacy `marks_quota` only for old snapshots) | Yes | Yes | ? | Current MVP v2 burns from the single `marks` counter; `marks_quota` is historical compatibility text, not an active public balance |
| `fee=0` allowance for mark-based baseline operations | Yes | ? | ? | Zero-fee burn flow allowed in v1 baseline profile |
| Cross-domain burn proof handled source-side only | Yes | ? | ? | Target shard does not mutate burn state for external burn events |
| Advanced policy engine (membership/cosign matrix) | ? | Yes | Yes | Hooked, not mandatory in baseline |
| Witness class full semantics | ? | Yes | Yes | Detectability now, full behavior later |
| Admission layer (compliance) | ? | Yes | Yes | Optional extension |
| IPv4 claim distribution | ? | Yes | Yes | Future economics/governance track |
| Annual redistribution | ? | ? | Yes | Post-v1 economics |
| Dynamic validator sets | ? | Yes | Yes | Planned but not baseline |
| Governance layer | ? | ? | Yes | Post-v1 |
| Bitcoin anchoring | ? | ? | Yes | Advanced extension |

---

## 3. Critical Observations

### 3.1 Core Rule

> v1 testnet must not break compatibility with v0 local account flows.

### 3.2 Red Flags

Architecture must be reviewed immediately if any condition appears:

- cross-shard support requires replacing account core with UTXO core;
- existing v0 transaction semantics are silently changed;
- replay protection is optionalized;
- policy checks are bypassed in user transaction paths.

### 3.3 Safe Evolution Pattern

Allowed evolution:

- add new transaction envelopes (`EXPORT/IMPORT`) without changing v0 local flow;
- derive shard routing by protocol comparison (`domain_hi(sender/receiver)`) instead of manual route flags;
- harden finality proof profile without RPC breakage;
- extend policy modules behind stable validation interfaces;
- extend validator/governance economics as post-v1 tracks.

---

## 4. MVP v1 Success Criteria

MVP v1 testnet is successful when:

- two shards run independently;
- cross-shard coin transfer works via explicit export/import flow;
- replay attacks are blocked deterministically;
- local v0-compatible flows remain operational.

Not required for v1 baseline:

- complete economics (IPv4 claims, annual redistribution);
- full governance and dynamic validator lifecycle;
- production-grade decentralization/security guarantees.
