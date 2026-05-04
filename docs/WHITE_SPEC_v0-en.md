# PWM White-spec v0 (MVP)

Status: implementation draft, aligned with [DRAFT_WHITEPAPER-ru.md](../DRAFT_WHITEPAPER-ru.md) with explicit simplifications.

Meaning of the **matrixchain** term in README and mapping to the whitepaper axes ("identity / execution / economics"): [MATRIXCHAIN_SPEC_v0.md](MATRIXCHAIN_SPEC_v0.md).

Next-phase plan for addresses and witness model: [ADDRESS_SPEC_PHASE1_bech32dx.md](ADDRESS_SPEC_PHASE1_bech32dx.md).

## 1. v0 Goals

- Single-process chain (devnet), classic signatures (Ed25519).
- Cluster account identification via **brute-HD** over a 16-bit domain code.
- Transactions: `INIT`, `TRANSFER`, `STAKE`, `UNSTAKE`, `BURN_MARK`.
- Marks and staking in a **simplified** form (see §6-7).
- IPv4 claiming, inflation, PQC, sharding, arbiters, full "dumb contracts" set are **out of v0 scope** (reserved in protocol as extensions).

## 2. Identifiers

### 2.1 Domain code

`domain_code: u16` is a 16-bit domain code (as in whitepaper **AABB**) used in the model:

- `0x0300..=0xC5FF` - regulatory/country (195 country clusters; main user range),
- `0x0000..=0x02FF` - reserved prelude (first 3 `domain_hi`, not assigned to countries in the current index),
- `0xD000..=0xDFFF` - sector (11 indexed sector clusters inside the class),
- `0xE000..=0xEFFF` - reserve (cannot be used as recipient in regular tx),
- `0xF000..=0xFFFF` - witness (service witness-only addresses, cannot receive regular transfers).

### 2.2 Cluster address (binary)

For derivation index `i` (non-secret brute-force counter):

1. Child key: `sk_i = HD_derive(master_sk, path m/0'/i)` (SLIP-0010 Ed25519).
2. `pk_i` is the public key.
3. `addr_raw = BLAKE3(pk_i || LE_U32(i))` (32 bytes).
4. Match condition: `u16_be(addr_raw[0..2]) == domain_code`.

The first matching `i` is fixed together with `pk_i` as the active account key.

Human-readable forms in Phase 1:

- primary UX (strict pretty): `pwm1-<label_or_$hex!>-f<flags8hex>-t<tail52hex>`,
- canonical bech32DX: `pwm1...` (supported for input/output and round-trip),
- legacy `PWMv0-<HEX64_ACCOUNT_ID>` / plain hex: compat input only.

**AccountId** in state: `addr_raw` (32 bytes).

### 2.3 Account initialization (INIT)

Before `INIT`, an account is **inactive**: incoming transfers are rejected (special devnet rule is not used - always INIT first after funding from genesis is unavailable; in v0, **genesis may assign pre-initialized validator accounts**).

`INIT` fields (MVP):

| Field      | Type    | Description                       |
|------------|---------|-----------------------------------|
| `index`    | `u32`   | index/ZIP/TNK id (metadata)       |
| `flags`    | `u32`   | behavior bitmask (reserved)       |

After `INIT`, the account is active; `index` and `flags` are stored in state.

## 3. Transactions (body + signature)

Canonical serialized body (without signature) is hashed: `tx_hash = BLAKE3(canonical_body)`.

**v0 implementation (`pwm-core`):** instead of a separate serde body-only serialization, the system uses `signing_message(tx)` = prefix `PWMv0/TX` + bincode of fields; this message is hashed for signing and verification. Semantically this is a single shared canonical format for node and CLI; when format changes, both code and this paragraph must be updated.

Signature: Ed25519 over `tx_hash` with sender key (for `INIT`, owner signature is additionally required - same key in v0).

Types:

### 3.1 `INIT { index, flags }`

- Sender: inactive account with zero or genesis-issued balance is **not** required; in v0 it is enough that the keypair exists and publishes INIT once.
- Effect: `initialized = true`, store `index`, `flags`.

### 3.2 `TRANSFER { to: AccountId, amount: u128, fee: u128 }`

- Only for active accounts.
- `from.balance >= amount + fee`, `fee` is burned as network fee (`fee_pool` accrues to validators in simplified v0 model - credited to block pool).
- Recipient policy in regular user-flow: `reserve`/`witness` and unknown/non-indexable domains are rejected.

### 3.3 `STAKE { amount: u128 }` / `UNSTAKE { amount: u128 }`

- Move value between `balance` and `staked` for active account.
- In v0 there is no unlock period.

### 3.4 `BURN_MARK { mark_amount: u128, beneficiary: AccountId | NONE }`

- Deducts marks from sender balance; `mark_amount` is destroyed.
- `beneficiary` in binary format: either 32-byte account or zero identifier for "addressless annihilation" (reserved field usage).
- Same recipient policy applies to `beneficiary` (no `reserve`/`witness`/unknown in regular flow).

Evolution note:

- For v0 devnet, burn source is account `marks` field.
- For v1 testnet baseline (see §7), burn source is redefined to `marks_quota` (burn-only resource) to preserve strict upgrade at tx-shape level with simplified marks economics.

## 3.5 Wallet-first tx path (CLI/TUI)

- In Phase 1, default signing path is `--wallet` (wallet-first); `--master` stays as explicit dev override.
- Wallet v1 is encrypted by default; plaintext is allowed only as `INSECURE_DEV_ONLY` with explicit opt-in.

## 4. Account state

```text
struct Account {
  balance_pwm: u128,
  staked: u128,
  marks: u128,
  initialized: bool,
  index: u32,
  flags: u32,
}
```

## 5. Emission and marks (v0 simplification)

- **Inflation / IPv4 claiming**: not implemented; block reward is a fixed `BLOCK_REWARD` constant from genesis, credited to `producer` from block header.
- **Marks from stake**: for each applied block, for each active account:

`marks_accrued = staked * MARKS_PER_BLOCK_COEFF / 1_000_000` (integer math, coefficient from genesis).

- **Marks TTL**: optional in v0 and may be omitted; can be added later as periodic `prune` without changing tx format.

## 6. Devnet consensus (v0)

Round-robin **PoA**: fixed validator list (Ed25519 pubkeys) in genesis; current leader signs the block; height defines leader index `height % N`.

Block header: `height`, `prev_hash`, `timestamp`, `producer_idx`, `tx_root`, `state_root`, `signature`.

## 7. v1 testnet extension (strict upgrade over v0)

This section defines the evolutionary transition from current devnet to a more mature testnet without changing the base state model.

### 7.1 Base compatibility

- `v1` keeps account-based state (`balance/staked/marks/initialized/index/flags`) as source of truth.
- Existing v0 tx types (`INIT`, `TRANSFER`, `STAKE`, `UNSTAKE`, `BURN_MARK`) keep their shape and local (same-shard) path.
- For `BURN_MARK`, v1 baseline uses an explicit economic toggle: burn deducts `marks_quota` (not `marks`) while tx shape and policy contour stay unchanged.
- Wallet/signature/tx body canonicalization remain v0-compatible; new fields/types are added only as extensions.

### 7.2 Minimal v1 testnet scope

- At least two independent **spec-level geo-shards** (domain clusters) with separate state and validator sets.
- **Normative spec-level geo-shard definition:** a shard in spec terms is an address cluster with fixed `domain_hi` (high byte of `domain_code`), not a `domain_hi` range.
- Domain-cluster "islandization" is allowed operationally: separate `domain_hi` clusters may be temporarily isolated/restricted by policy and infrastructure without changing this definition.
- `Shard A`/`Shard B` names in operational dev/test scenarios are only convenient process-instance labels and do not replace protocol geo-shard semantics.
- **Critical:** range-splitting heuristics like `domain_hi < 0x80` vs `>= 0x80` (the so-called `0x80 split`) are forbidden as protocol routing source.
- Cross-shard coin transfer is implemented via explicit additive flow:
  1. `EXPORT` in source shard,
  2. finality-proof (minimal certificate profile),
  3. `IMPORT` in target shard,
  4. replay guard (`ImportedSet` or equivalent used-export identifiers structure).
- same-shard vs cross-shard is selected protocol-wise by comparing `domain_hi(sender)` and `domain_hi(receiver)`:
  - if equal -> local path (`TRANSFER`);
  - if different -> `EXPORT/IMPORT` path is mandatory.

### 7.3 Policies and finalization in v1

- MVP v1 requires minimal recipient/domain-class policy checks (reject `reserve`/`witness`/unknown in regular flow).
- Advanced rules (cosign matrix, membership-driven routing, admission governance) stay as extensions and can be plugged in without breaking base flow.
- Operational shard runtime identity model (cluster-bound launch params, node self-identification in p2p, native/foreign peer-priority) is formalized separately in `docs/rfc/8-shard-runtime-identity-and-peering.md`; this layer does not change base routing protocol semantics defined in this white-spec.
- v1 finality certificate may use a minimal testnet profile, but the format must stay extensible for stricter models.
- For v1 testnet without secondary mark balances, `marks_quota` is introduced (burn-only account resource):
  - `BURN_MARK` deducts `marks_quota`, not `balance_pwm`;
  - baseline allows `fee = 0` for mark-based operations and cross-shard burn context.
- Cross-domain `BURN_MARK` context does not require special handling in target shard:
  - burn proof is formed and verified only in source shard;
  - target shard is not required to mutate local marks state on foreign burn event.

### 7.4 Sprint 13 cross-domain roaming MVP cut (as implemented)

Implemented baseline (Sprint 13):
- Cross-domain path for value transfer is live as explicit `EXPORT -> IMPORT` operator flow.
- `EXPORT` records deterministic provenance (`export_id` + `to/amount/target_domain`) in source runtime.
- `IMPORT` is accepted only with known matching provenance and one-time replay guard (`ImportedSet`).
- RPC/runtime contract is stable for MVP: `POST /v1/tx` returns `204` on success, `409` for duplicate import, `400` for invalid/unknown provenance.
- Current operator path is manual handoff of import material between source and target node (runbook-driven).

Not covered in this MVP cut (intentional):
- No admission/compliance certificate layer.
- No automated cross-node handoff/relay service for import material.
- No advanced finality profiles beyond minimal testnet baseline.
- No async pipeline optimization for HTTP ingest (`apply_tx + seal([])` stays synchronous for `EXPORT/IMPORT`).

Known pitfalls / fragility (must watch):
- Operator handoff: manual transfer of `export_id`/provenance is operationally fragile and error-prone.
- Retry semantics: repeated operator retries can create duplicate-import attempts (`409` expected); tooling must treat it as idempotent reject, not "unknown failure".
- Sync seal hot path: `apply_tx + seal([])` on request path increases latency/concurrency pressure under bursts.
- Targeting risks: wrong `target_domain`/RPC endpoint mix-up leads to deterministic reject (`400 invalid import`), but still creates operator confusion in multi-node runs.

Decision forks (post-MVP options):
- A) Keep manual handoff + strict runbook. Tradeoff: lowest implementation cost, highest operator burden.
- B) Add lightweight relay/handoff service for export material. Tradeoff: better UX and fewer mistakes, extra component/runtime dependency.
- C) Move `EXPORT/IMPORT` sealing off sync HTTP path (queued async worker). Tradeoff: better latency envelope, more complex status/finality semantics.
- D) Introduce admission/policy certificate layer. Tradeoff: stronger governance/compliance, larger protocol and operational surface.

## 8. Out of v1 testnet scope (for later spec)

- "Dumb contracts" policies, corporate dual signatures, CLTV.
- Zone arbiter, reversals.
- Regional consensus sharding.
- PQC, separate whitepaper address format vs `PWMv0-`.
- Production off-chain burning and X-PWM - see stub module and [OFFCHAIN_STUB.md](./OFFCHAIN_STUB.md).
