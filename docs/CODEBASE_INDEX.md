# PWM Cryptocurrency — Codebase Index

> Generated: 2026-05-22
> Language: Rust 2021 Edition
> Build System: Cargo Workspace
> Version: v0.1.55 (pwmd)

---

## 1. Workspace Overview

| Crate | Package Name | Binary | Description |
|-------|-------------|--------|-------------|
| `pwm-core` | `pwm-core` | library only | Core protocol primitives: crypto, types, transactions, state, chain, mempool, wallet utilities |
| `pwmd` | `pwmd` | `pwmd` | Node daemon: REST API, P2P transport, block sealing, roaming, federation |
| `pwm-cli` | `pwm-cli` | `pwm` | CLI wallet: key management, transaction submission, address book, brute-force tools |
| `pwm-tui` | `pwm-tui` | `pwm-tui` | Terminal UI: interactive account table, send flows, wallet management |

---

## 2. Crate `pwm-core` — Protocol Primitives

**Path:** `crates/pwm-core/src/`

| Module | File | Responsibility |
|--------|------|----------------|
| **lib** | `lib.rs` | Public re-exports; defines the crate's public API surface |
| **crypto** | `crypto.rs` | BLAKE3 hashing, Ed25519 sign/verify, block header signing payload |
| **types** | `types.rs` | Core account and ledger data types, address payload primitives |
| **display** | `display.rs` | Human-readable address formatting and display helpers |
| **hd** | `hd.rs` | SLIP-0010 Ed25519 HD derivation (`m/0'/i`), account ID generation, domain extraction, brute-force cluster address finder |
| **tx** | `tx.rs` | Transaction types (`Init`, `Transfer`, `Stake`, `Unstake`, `BurnMark`, `Export`, `Import`), `SignedTx`, signature validation, fee computation, export/import ID logic |
| **block** | `block.rs` | `BlockHdr` (PoA signed header), `Block` (header + txs), Merkle root over transactions |
| **chain** | `chain.rs` | `Chain` struct: PoA block sealing, validator rotation, `prev_hash` linking, genesis boot |
| **state** | `state.rs` | `State` (accounts map, fee pool, marks quota, imported/exported registries), `apply_tx` logic, cross-shard validation |
| **mempool** | `mempool.rs` | `Mpool`: bounded FIFO transaction queue for block sealing |
| **genesis** | `genesis.rs` | `GenCfg`, `GRow`, `VRow`, reward policy, dev-net factory (`dev_net()`) |
| **domain_index** | `domain_index.rs` | Phase 1B domain registry: 195 countries + 11 sectors, domain categories (Regulatory, Sector, Reserve, Witness), lookup by raw code / hi-byte / label |
| **bridge_commitment** | `bridge_commitment.rs` | Bridge/export commitment payload types and hashing helpers |
| **address_book** | `address_book.rs` | `AddressBookEntry` (string or labeled), recipient domain/address validation, YAML-safe address book append |
| **wallet_crypto** | `wallet_crypto.rs` | ChaCha20-Poly1305 + PBKDF2-HMAC-SHA256 wallet encryption/decryption (shared between CLI & TUI) |
| **wallet_io** | `wallet_io.rs` | Shared wallet file IO helpers and persistence glue |
| **wallet_read** | `wallet_read.rs` | Minimal wallet YAML parsing for read-only operations (TUI, thin tools), header normalization |
| **offchain** | `offchain.rs` | Offchain batch Merkle root + Ed25519 provider signature (stub, no on-chain bridge yet) |
| **ser_bin** | `ser_bin.rs` | Binary serialization helpers (e.g., Ed25519 64-byte sig for serde) |
| **ser_json_u128** | `ser_json_u128.rs` | JSON serde helpers for `u128` string/number compatibility |
| **rpc** | `rpc.rs` | Shared RPC-facing payloads and helper structs used by clients/daemon |
| **reject_wire** | `reject_wire.rs` | Wire-level reject reason enums and transport-safe serialization |

### Key Types & Structures

| Type | Location | Purpose |
|------|----------|---------|
| `AccountId` | `types.rs` | 32-byte address: first 2 bytes = domain code, rest = BLAKE3(pk || LE_U32(i)) |
| `Account` | `types.rs` | Account state: balance, nonce, initialized flag, marks |
| `SignedTx` | `tx.rs` | Signed transaction: body, domain, signature, computed account ID |
| `TxBody` | `tx.rs` | Enum: Init, Transfer, Stake, Unstake, BurnMark, Export, Import |
| `BlockHdr` | `block.rs` | PoA block header: height, prev_hash, ts, prod_idx, tx_root, state_root, sig |
| `Block` | `block.rs` | Block = header + transactions vector |
| `Chain` | `chain.rs` | In-memory PoA chain: config, validator keys, blocks, state |
| `State` | `state.rs` | Live ledger: accounts BTreeMap, fee_pool, marks_quota, imported_set, exported_registry |
| `Mpool` | `mempool.rs` | Bounded FIFO queue: push, take, prepend (abort re-injection) |
| `GenCfg` | `genesis.rs` | Genesis configuration: funding, validators, reward policy, block_reward, marks_coeff |
| `DomainEntry` | `domain_index.rs` | Domain registry row: raw code, label, category |
| `AddressBookEntry` | `address_book.rs` | Address-only string or {address, label} map |
| `WalletSealedPayload` | `wallet_crypto.rs` | Encrypted wallet secret: ciphertext_b64, salt_b64, nonce_b64, kdf params |

---

## 3. Crate `pwmd` — Node Daemon

**Path:** `crates/pwmd/src/`

| Module | File | Responsibility |
|--------|------|----------------|
| **main** | `main.rs` | CLI entry point: argument parsing (clap), config assembly, node launch |
| **lib** | `lib.rs` | Public exports, shared helpers, crate-level test entry points |
| **api/** | `api/mod.rs` + handlers | Router assembly plus endpoint-specific handlers for status, tx, roaming, peers, federation, bridge, shutdown |
| **bin/** | `bin/*.rs` | Auxiliary binaries: snapshot repair, ClickHouse import, lease probe |
| **config** | `config.rs` | `PwmdConfig`: listen addr, genesis source, data file, shard, identity, transport, logging |
| **identity** | `identity.rs` | `ShardId` (A/B), `RuntimeIdentity`, `RuntimeIdentityMode` (Explicit/Neutral/Alias), domain-based storage namespaces |
| **bootstrap** | `bootstrap.rs` | App factories: dev-net, genesis JSON, with/without shard, with/without identity |
| **state** | `state.rs` | `App` (shared state container), `Inner` (chain + pool + roaming + cross-shard + federation), `InitState` lifecycle |
| **lifecycle** | `lifecycle.rs` | Node run loop: autosnapshot (every 100 blocks), seal loop, transport spawning, summary logging (every 500 blocks) |
| **transport/** | `transport/mod.rs` + submodules | P2P transport: dial/spawn/lifecycle, incoming hello, health metrics, peer policy, peer session state |
| **handshake** | `handshake.rs` | `NodeHello` protocol: network/genesis/cluster/node validation, Ed25519 signature, replay nonce cache, rejection reasons |
| **relay** | `relay.rs` | Peer relay one-window: HTTP relay to seed peers, export handoff, import relay, genesis fetch stub |
| **roaming** | `roaming.rs` | Cross-shard roaming: `RoamingPool`, `ExportReadiness` preflight, intent lifecycle (queued/exported/relayed/imported/expired/failed), active locks |
| **ledger** | `ledger.rs` | `CrossShardLedger`: export/import fact tracking, summary generation, stuck transaction detection |
| **federation** | `federation.rs` | Federation shard height dictionary: gossip via heartbeat, merge rows, sweep loop, snapshot API |
| **snapshot/** | `snapshot/mod.rs` + submodules | Snapshot IO, genesis load/store, incremental persistence, repair, telemetry, ClickHouse export helpers |
| **tx_policy** | `tx_policy.rs` | Transaction policy: shard routing by domain category, recipient validation, import provenance prefilter, duplicate import guard, recipient init gate |
| **logging** | `logging.rs` | Custom tracing subscriber: file sink with rotation, console output, ANSI colors, TX delta logging, `NodeLogger` |
| **wire_serde** | `wire_serde.rs` | Wire-format serde helpers (u128 compatible deserialization) |
| **bridge_trust** | `bridge_trust.rs` | Bridge trust-state evaluation and guardrails for handoff/import flows |
| **lease / lease_backend** | `lease.rs`, `lease_backend.rs` | Lease coordination primitives and backend implementation helpers |
| **peer_list** | `peer_list.rs` | Shared peer list normalization and storage helpers |
| **debug_dump** | `debug_dump.rs` | Runtime debug dump output for operator diagnostics |
| **slice20_e2e_tests** | `slice20_e2e_tests.rs` | Large end-to-end support and regression tests kept outside runtime modules |

### REST API Endpoints

| Endpoint | Method | Module | Description |
|----------|--------|--------|-------------|
| `/v1/status` | GET | `api.rs` | Node status: phase, shard, identity, roaming, peers, cross-shard summary, diagnostics |
| `/v1/head` | GET | `api.rs` | Chain tip: height, tip hash |
| `/v1/tx` | POST | `api.rs` | Submit signed transaction (max 256KB JSON) |
| `/v1/account/{id}` | GET | `api.rs` | Single account query: balance, nonce, local/foreign split view |
| `/v1/accounts` | GET | `api.rs` | All accounts list with split semantics |
| `/v1/dev/peers` | GET | `api.rs` | Peer telemetry: accepted/rejected counts, class breakdown, connected peers |
| `/v1/peer/hello` | POST | `api.rs` | NodeHello handshake acceptance/rejection |
| `/v1/roaming-intents` | POST | `api.rs` | Submit cross-shard roaming intent (Export + TTL) |
| `/v1/roaming-intents` | GET | `api.rs` | List roaming intents with status |
| `/v1/export-readiness` | POST | `api.rs` | Pre-flight check for EXPORT transaction |
| `/v1/federation/shards` | GET | `api.rs` | Federation shard height table |

---

## 4. Crate `pwm-cli` — Wallet CLI

**Path:** `crates/pwm-cli/src/`

| Module | File | Responsibility |
|--------|------|----------------|
| **main** | `main.rs` | Thin CLI entry point wiring parser, dispatcher, exit handling |
| **cli_parse / cli_cmd** | `cli_parse.rs`, `cli_cmd.rs` | Clap command tree and typed command definitions |
| **cli_dispatch** | `cli_dispatch.rs` | Top-level command dispatch into feature-specific handlers |
| **cli_config / cli_exit** | `cli_config.rs`, `cli_exit.rs` | CLI runtime config loading and stable exit-code mapping |
| **cmd_* modules** | `cmd_*.rs` | Command implementations: wallet, tx, node, roaming, genesis, address book, offchain, keys, address formatting |
| **wallet/** | `wallet/*` | Wallet schema, migration, persistence, unlock/signing helpers |
| **wallet_shell** | `wallet_shell.rs` | Interactive wallet shell helpers and prompts |
| **rpc_helpers** | `rpc_helpers.rs` | Shared blocking RPC helpers for CLI workflows |
| **signer** | `signer.rs` | Signing helper layer shared across wallet and tx commands |
| **bruteforce / purpose_expand** | `bruteforce.rs`, `purpose_expand.rs` | Address brute-force engine and purpose-path expansion helpers |

### CLI Subcommands

| Command | Module | Description |
|---------|--------|-------------|
| `wallet-create` | `main.rs` | Generate new wallet (random seed), optional encryption |
| `wallet-recover` | `main.rs` | Recover wallet from seed hex |
| `wallet-show` | `main.rs` | Display wallet info (account ID, derivation, domain) |
| `wallet-backup` | `main.rs` | Backup wallet file |
| `wallet-account-add` | `main.rs` | Add derived account to wallet |
| `wallet-account-list` | `main.rs` | List all accounts in wallet |
| `wallet-account-use` | `main.rs` | Set active account |
| `wallet-account-remove` | `main.rs` | Remove account from wallet |
| `address-book-add` | `main.rs` | Add recipient to address book |
| `address-book-remove` | `main.rs` | Remove recipient from address book |
| `tx-init` | `main.rs` | Initialize account on-chain |
| `tx-transfer` | `main.rs` | Send PWM transfer |
| `tx-stake` | `main.rs` | Submit stake transaction |
| `tx-unstake` | `main.rs` | Submit unstake transaction |
| `tx-burn-mark` | `main.rs` | Submit burn-mark transaction |
| `tx-export` | `main.rs` | Cross-shard EXPORT (roaming intent) |
| `tx-import` | `main.rs` | Cross-shard IMPORT on target shard |
| `tx-handoff-register` | `main.rs` | Register EXPORT handoff on source peer |
| `tx-status` | `main.rs` | Query transaction status |
| `cluster-brute` | `main.rs` | Brute-force derive address matching domain |
| `cluster-brute-flags` | `main.rs` | Brute-force with domain + flag constraints |
| `cluster-brute-index` | `main.rs` | Resume brute-force from saved index |
| `genesis-export` | `main.rs` | Export genesis configuration |
| `offchain-sign` | `main.rs` | Sign offchain batch (provider flow) |
| `address-pretty` | `main.rs` | Convert account ID to bech32DX or human-readable format |

---

## 5. Crate `pwm-tui` — Terminal UI

**Path:** `crates/pwm-tui/src/`

| Module | File | Responsibility |
|--------|------|----------------|
| **main** | `main.rs` | TUI binary bootstrap and runtime entry |
| **lib** | `lib.rs` | Shared TUI exports and module wiring |
| **status / history / account_view** | `status.rs`, `history.rs`, `account_view.rs` | Read models and panes for status, history, account inspection |
| **send / stake / burn forms** | `send_form.rs`, `stake_form.rs`, `burn_form.rs` | Interactive transaction forms and validation state |
| **wallet / signing** | `wallet.rs`, `signing.rs` | Wallet unlock state, signing material, passphrase flows |
| **rpc_account / tx_submit** | `rpc_account.rs`, `tx_submit.rs` | RPC fetch/submit glue for account and transaction actions |
| **tui_loop / modals / selection** | `tui_loop.rs`, `modals.rs`, `selection.rs` | Event loop, modal dialogs, keyboard/navigation state |
| **models / config / test_support** | `models.rs`, `config.rs`, `test_support.rs` | Shared view models, config, test scaffolding |
| **roaming** | `roaming.rs` | Cross-shard roaming status views and actions |

### TUI Features

| Feature | Description |
|---------|-------------|
| Account table | Displays all accounts from node `/v1/accounts` with balances, nonces |
| Send flow (F2) | Interactive transfer: select recipient, enter amount, confirm |
| Burn-mark (F5) | Redirect to CLI (not wired in TUI) |
| Wallet unlock | Encrypted wallet support with auto-lock timeout |
| Roaming status | Shows cross-shard intent lifecycle (queued/exported/relayed/imported) |
| Debug mode | `PWM_TUI_DEBUG=1` enables raw JSON dumps |
| RPC config | `PWM_RPC` env var for node URL, `PWM_TUI_RPC_TIMEOUT_MS` for timeout |

---

## 6. Cross-Cutting Concerns

### Cryptography Stack

| Component | Algorithm | Location |
|-----------|-----------|----------|
| Hashing | BLAKE3 (32-byte) | `pwm-core/crypto.rs` |
| Signatures | Ed25519 (dalek) | `pwm-core/crypto.rs` |
| HD Derivation | SLIP-0010 Ed25519 (`m/0'/i`) | `pwm-core/hd.rs` |
| Address | BLAKE3(pk || LE_U32(derivation_index)) | `pwm-core/hd.rs` |
| Wallet Encryption | ChaCha20-Poly1305 + PBKDF2-HMAC-SHA256 (100k iters) | `pwm-core/wallet_crypto.rs` |
| Block Merkle | Binary BLAKE3 Merkle tree | `pwm-core/block.rs` |
| Offchain Merkle | Binary BLAKE3 Merkle tree | `pwm-core/offchain.rs` |
| State Digest | BLAKE3 over bincode-serialized state | `pwm-core/state.rs` |

### Address Formats

| Format | Description | Module |
|--------|-------------|--------|
| Raw hex | 32 bytes hex-encoded | `pwm-core/types.rs` |
| Human-readable | `pwm{domain_label}.{short_hex}` | `pwm-core/types.rs` |
| bech32DX | `pwm1{domain}{checksum}` | `pwm-core/types.rs` |

### Domain Classification (Phase 1B)

| Category | Range | Shard | Description |
|----------|-------|-------|-------------|
| Regulatory (countries) | `0x0300..=0xC5FF` | A | 195 indexed countries |
| Country prelude reserve | `0x0000..=0x02FF` | — | Reserved, not in country index |
| Sector | `0xD000..=0xDFFF` | B | 11 indexed sectors |
| Reserve | `0xE000..=0xEFFF` | — | Range-only, no labels |
| Witness | `0xF000..=0xFFFF` | — | Indexed: 0xF003, 0xF006, 0xF009 |

### Sharding Model

| Concept | Description | Modules |
|---------|-------------|---------|
| Shard A | Regulatory domain accounts | `pwmd/tx_policy.rs`, `pwmd/identity.rs` |
| Shard B | Sector domain accounts | `pwmd/tx_policy.rs`, `pwmd/identity.rs` |
| Roaming | Cross-shard Export/Import via relay | `pwmd/roaming.rs`, `pwmd/relay.rs`, `pwmd/ledger.rs` |
| Identity modes | Explicit (domain-based), Neutral (relay), Alias (compat) | `pwmd/identity.rs` |
| Storage namespaces | `state/domain-hi-0xNN/` (explicit), `state/shard-{a\|b}/` (alias), `state/pwm-data.json` (neutral) | `pwmd/identity.rs`, `pwmd/snapshot.rs` |

---

## 7. External Dependencies

| Dependency | Version | Used By | Purpose |
|------------|---------|---------|---------|
| `axum` | 0.7 | `pwmd` | HTTP REST API framework |
| `tokio` | 1 (full) | `pwmd` | Async runtime |
| `clap` | 4 | `pwmd`, `pwm-cli`, `pwm-tui` | CLI argument parsing |
| `serde` / `serde_json` / `serde_yaml` | 1 / 1 / 0.9 | All crates | Serialization |
| `ed25519-dalek` | 2 | `pwm-core`, `pwm-cli`, `pwm-tui`, `pwmd` | Ed25519 cryptography |
| `blake3` | 1 | `pwm-core` | BLAKE3 hashing |
| `slip10_ed25519` | 0.1.3 | `pwm-core`, `pwm-cli`, `pwm-tui`, `pwmd` | SLIP-0010 HD derivation |
| `ratatui` | 0.26.3 | `pwm-tui` | Terminal UI framework |
| `crossterm` | 0.27 | `pwm-tui` | Terminal manipulation |
| `reqwest` | 0.12 | `pwm-cli`, `pwm-tui`, `pwmd` | HTTP client |
| `tracing` / `tracing-subscriber` | 0.1 / 0.3 | `pwmd` | Logging |
| `chacha20poly1305` | 0.10 | `pwm-core` | AEAD encryption |
| `pbkdf2` | 0.12 | `pwm-core` | Key derivation |
| `sha2` | 0.10 | `pwm-core` | SHA-256 (for PBKDF2) |
| `bech32` | 0.9 | `pwm-core` | bech32DX encoding |
| `bincode` | 1.3 | `pwm-core` | Binary serialization |
| `hex` | 0.4 | `pwm-core`, `pwm-cli`, `pwm-tui`, `pwmd` | Hex encoding |
| `rand` | 0.8 | `pwm-core` | Random number generation |
| `rpassword` | 7 | `pwm-cli`, `pwmd` | Password input |
| `tower-http` | 0.5 | `pwmd` | CORS middleware |
| `base64` | 0.22 | `pwm-core`, `pwm-cli`, `pwm-tui` | Base64 encoding |

---

## 8. Configuration & Environment

| Variable | Module | Purpose |
|----------|--------|---------|
| `PWM_RPC` | `pwm-cli`, `pwm-tui` | Node RPC URL (default: `http://127.0.0.1:3030`) |
| `PWM_CLI_RPC_TIMEOUT_MS` | `pwm-cli` | CLI HTTP timeout (default: 10000ms, max: 120000ms) |
| `PWM_TUI_RPC_TIMEOUT_MS` | `pwm-tui` | TUI HTTP timeout (default: 3000ms, max: 120000ms) |
| `PWM_TUI_TARGET_RPC` | `pwm-tui` | Counterparty shard RPC for cross-shard queries |
| `PWM_TUI_WALLET` | `pwm-tui` | Wallet file path |
| `PWM_TUI_WALLET_PASSPHRASE` | `pwm-tui` | Wallet passphrase (env) |
| `PWM_TUI_WALLET_UNLOCK_SECS` | `pwm-tui` | Auto-lock timeout (default: 300s, max: 604800s) |
| `PWM_TUI_DEBUG` | `pwm-tui` | Enable debug JSON output |
| `PWM_GENESIS_PASSPHRASE` | `pwmd` | Genesis validator key passphrase |
| `PWM_CORS_ORIGINS` | `pwmd` | CORS allowed origins (non-loopback) |
| `PWM_PEER_LISTEN` | `pwmd` | Peer listener socket address |
| `PWM_LOG_NAME` | `pwmd` | Log stream name |
| `PWM_LOG_DIR` | `pwmd` | Log root directory |
| `PWM_LOG_FILE_MODE` | `pwmd` | File logging mode (on/off/required) |
| `PWM_LOG_CONSOLE_COLOR` | `pwmd` | Console color mode (auto/always/never) |

---

## 9. Key Documents

| Document | Path | Purpose |
|----------|------|---------|
| White Spec (EN) | `docs/WHITE_SPEC_v0-en.md` | Protocol specification |
| White Spec (RU) | `docs/WHITE_SPEC_v0.md` | Протокол спецификация |
| Whitepaper (EN) | `DRAFT_WHITEPAPER.md` | Draft whitepaper |
| Whitepaper (RU) | `DRAFT_WHITEPAPER-ru.md` | Draft whitepaper |
| Domain Clusters | `docs/DOMAINS.md` | Domain registry (195 countries + 11 sectors) |
| GEO Sharding | `docs/GEO-SHARDING-EXPLANATION.md` | Geo-sharding explanation |
| MVP Checklist | `docs/MVP-checklist.md` | MVP feature checklist |
| Phase 1 Checklist | `docs/PHASE1_CHECKLIST.md` | Phase 1 release checklist |
| Phase 1 Summary | `docs/PHASE1_RELEASE_SUMMARY.md` | Phase 1 release summary |
| Whitepaper Coverage | `docs/WHITEPAPER_COVERAGE_MATRIX.md` | Spec-to-code coverage matrix |
| TUI Design Spec | `docs/TUI_DESIGN_SPEC_v1.md` | TUI design specification |
| Wallet Security | `docs/WALLET_SECURITY_MODES.md` | Wallet security modes |
| Wallet Recovery | `docs/WALLET_BACKUP_RECOVERY_PLAYBOOK.md` | Backup & recovery guide |
| Agent Prompts | `docs/AGENT_PROMPTS.md` | AI agent collaboration prompts |
| ADR | `docs/adr/` | Architecture Decision Records |
| RFC | `docs/rfc/` | Request for Comments |
| Plans | `docs/plans/` | Development plans |
| Reviews | `docs/reviews/` | Code review records |

---

## 10. Tools & Scripts

| Script | Path | Purpose |
|--------|------|---------|
| Address Bruteforce (CMD) | `scripts/addr-bruteforce-interactive.cmd` | Windows brute-force script |
| Address Bruteforce (SH) | `scripts/addr-bruteforce-interactive.sh` | Unix brute-force script |
| CQDS Index Digest | `scripts/cqds_index_digest.py` | Index digest tool |
| Demo Two-Shard (PS1) | `tools/demo-two-shard.ps1` | PowerShell two-node demo |
| Demo Two-Shard (SH) | `tools/demo-two-shard.sh` | Shell two-node demo |
| Slice Artifacts | `tools/slice-artifacts.ps1` | PowerShell slice artifact tool |
| Slice Commit | `tools/slice-commit.ps1` | PowerShell slice commit tool |
| Node Runner 1 | `node-1.ps1` | PowerShell node 1 launcher |
| Node Runner 2 | `node-2.ps1` | PowerShell node 2 launcher |

---

## 11. State & Storage

| Path | Description |
|------|-------------|
| `state/neutral/<listen-tag>/pwm-data.json` | Neutral relay-default snapshot (`:` → `+` in tag) |
| `state/domain-hi-0xNN/pwm-data.json` | Explicit domain namespace snapshot |
| `epochs/` next to snapshot | JsonFile epoch JSONL + manifest (see storage guide) |
| `logs/` | Node log files (rotated) |
| `state/` | Default `--state-root` directory |

---

## 12. Sprint Status

| Area | Status | Notes |
|------|--------|-------|
| PoA Chain | ✅ Implemented | Block sealing, validator rotation |
| Transactions | ✅ Implemented | Init, Transfer, Stake, Unstake, BurnMark, Export, Import |
| REST API | ✅ Implemented | Full `/v1/*` endpoint set |
| P2P Transport | ✅ Implemented | TCP seed peers, NodeHello, heartbeat |
| Roaming (Cross-shard) | ✅ Implemented | Export/Import via relay one-window |
| Federation | ✅ Implemented | Shard height dictionary, gossip |
| Wallet CLI | ✅ Implemented | Full wallet + tx + address book + brute-force |
| TUI | ✅ Implemented | Account table, send flow, wallet unlock |
| Domain Index | ✅ Phase 1B | 195 countries + 11 sectors |
| Geo-Sharding | ✅ Implemented | Regulatory→A, Sector→B routing |
| Genesis JSON v4 | ✅ Implemented | Encrypted validator keys |
| State Persistence | ✅ Implemented | JSON snapshots with autosave |
| Logging | ✅ Implemented | File rotation, console, structured |
| Offchain Bridge | ⏳ Stub | Merkle + sign implemented, no on-chain bridge |
| Cross-shard EXPORT/IMPORT | ⏳ Deferred | Core-level support finalized, roaming flow active |
