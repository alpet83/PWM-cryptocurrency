# PWM Dependency Graph (v1 Testnet Direction)

**Status:** Working document  
**Purpose:** Define evolution order from v0 devnet to v1 testnet without breaking the account-based core.

---

## 1. Layer Model

PWM evolves in layers. Higher layers MUST NOT force a rewrite of lower committed layers.

### Layer 0 — Identity and Addressing

```text
bech32DX forms
domain encoding (u16 split)
subject class detectability
```

Dependencies: none

### Layer 1 — Account Ledger Core

```text
account state (balance/staked/marks/init metadata)
v0 tx set (INIT/TRANSFER/STAKE/UNSTAKE/BURN_MARK)
signature verification + deterministic state transitions
```

Depends on: Layer 0

### Layer 2 — Shard Runtime and Finality Profile

```text
per-shard validator set
block production
minimal finality proof profile for testnet
```

Depends on: Layer 1

### Layer 3 — Cross-Shard Transfer (Additive)

```text
EXPORT/IMPORT flow
export commitment proof
ImportedSet (or equivalent replay guard)
```

Depends on:

* Layer 1
* Layer 2

### Layer 4 — Policy Extensions

```text
minimal recipient/domain policy (MVP)
optional advanced policy hooks (cosign/membership/admission)
```

Depends on:

* Layer 1
* Layer 3

### Layer 5 — Advanced Economics and Governance (post-v1)

```text
IPv4 claim economics
annual redistribution
dynamic validator governance
```

Depends on:

* Layer 2
* Layer 4

---

## 2. Critical Dependency Paths

### 2.1 Strict-Upgrade Path

```text
Addressing -> AccountCore -> ShardRuntime -> RoamingAdditions
```

Core rule: roaming MUST extend account model, not replace it.

### 2.2 Cross-Shard Safety Path

```text
Export -> FinalityProof -> Import -> ReplayGuard
```

This chain MUST stay deterministic and replay-safe.

### 2.3 Policy Path

```text
Tx -> PolicyCheck -> StateMutation
```

Policy MUST run before state mutation, while MVP policy remains minimal.

---

## 3. Forbidden Dependencies

- Policy layer redefining tx canonical core.
- Roaming layer requiring full synchronous cross-shard consensus.
- Extension features forcing migration off account-based state in v1.
- UI/CLI behavior defining protocol validity.

---

## 4. Minimal Valid Graph for MVP SPEC v1 Testnet

```text
Layer 0 -> Layer 1 -> Layer 2 -> Layer 3
```

Layer 4 minimal recipient policy is required for user safety, advanced policy subsets are optional.

---

## 5. Incremental Build Strategy

### Phase A (stability)

* Layer 0-1 locked (no breaking changes vs v0)
* single-shard compatibility maintained

### Phase B (testnet core)

* Layer 2 + Layer 3
* at least two independent shards with coin transfer between them

### Phase C (safety hardening)

* Layer 4 minimal policy enforcement
* replay/finality edge-case hardening

### Phase D (post-v1 track)

* Layer 5 economics/governance extensions

---

## 6. Architecture Integrity Rule

> Any proposal that breaks strict-upgrade constraints must be treated as a separate track, not as MVP v1 baseline.

---

## 7. Practical Interpretation

Before accepting a feature into v1 baseline:

1. Verify it does not rewrite Layer 1 account core.
2. Verify compatibility with existing v0 tx/wallet/RPC behavior.
3. Verify shard transfer safety path (export/finality/import/replay).
4. Classify advanced behavior as optional extension if not required for two-shard testnet operation.
