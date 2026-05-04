# PWM Cryptocurrency — Codebase Refactoring Report

> Generated: 2026-05-01  
> Scope: Dead code, copy-paste, module decomposition, shared code extraction

**Sprint 15 — исполнение по слайсу O:** план и безопасный чеклист — [reviews/sprint-15-slice-O-plan.md](reviews/sprint-15-slice-O-plan.md), [reviews/sprint-15-slice-O-checklist.md](reviews/sprint-15-slice-O-checklist.md); тикет `tasks/20260502-s15-slice-O-codebase-cleanup.json`.

**Статус (2026-05-02):** группа **B** чеклиста закрыта в коде: общий `TextInput` (TUI), модули `pwm-core::display`, `pwm-core::rpc`, `pwm-core::wallet_io` и подключение в `pwm-tui` / `pwm-cli` (см. §7 Phase 1 п.4 и Phase 2 п.1–3).

---

## 1. Module Size Overview

| Module | Crate | Lines | Tests % | Status |
|--------|-------|-------|---------|--------|
| `main.rs` | pwm-tui | **6431** | ~30% | 🔴 CRITICAL — single file UI + logic + tests |
| `lib.rs` | pwmd | **6248** | ~80% | 🟡 Tests-heavy, acceptable after split |
| `main.rs` | pwm-cli | **5132** | ~25% | 🔴 CRITICAL — CLI subcommands + helpers inline |
| `transport.rs` | pwmd | **3407** | ~30% | 🔴 CRITICAL — types + metrics + tick logic + dial + relay |
| `wallet.rs` | pwm-cli | **2384** | ~15% | 🟡 Large but focused |
| `api.rs` | pwmd | **1957** | ~50% | 🟡 Tests-heavy |
| `snapshot/*` | pwmd | **~1650** (split) | ~35% | 🟢 §2.6 wave19 — `types` / `io` / `genesis` |
| `domain_index.rs` | pwm-core | **1232** | ~0% | 🟢 Data-only (country/sector tables), acceptable |
| `state.rs` | pwm-core | **1193** | ~65% | 🟢 Tests-heavy |
| `logging.rs` | pwmd | **1019** | ~55% | 🟡 Tests-heavy |
| `relay.rs` | pwmd | **641** | ~10% | 🟢 OK |
| `wallet_read.rs` | pwm-core | **633** | ~50% | 🟢 Tests-heavy |
| `slice20_e2e_tests.rs` | pwmd | **695** | 100% | ✅ Dedicated test file |

---

## 2. Critical Decomposition Targets (>1500 lines, mixed concerns)

### 2.1 `pwm-tui/src/main.rs` — 6431 lines → Split into ~15 modules

**Current state:** Everything in one file — types, RPC, wallet crypto, modals, event loop, rendering, tests.

**Proposed decomposition:**

| # | Suggested Module | Lines (est.) | Extract From | Key Types/Functions |
|---|------------------|--------------|--------------|---------------------|
| 1 | `config.rs` | ~50 | 1-183 | `Args`, timeout parsing, `http_client()`, RPC helpers |
| 2 | `status.rs` | ~40 | 187-310 | `RpcHealth`, `JsonFetchFailure`, footer rendering |
| 3 | `models.rs` | ~50 | 335-437 | `AcctRow`, `BookRecipient`, `WalletIdentity`, `format_pwm()` |
| 4 | `modals.rs` | ~120 | 445-713 | `BookPromptModal`, `UnlockModal`, `EncryptModal` + shared `TextInput` |
| 5 | `wallet.rs` | ~150 | 724-1243 | Wallet encryption, YAML I/O, identity loading, auto-lock |
| 6 | `rpc_account.rs` | ~60 | 1258-1406 | `fetch_nonce()`, `preflight_recipient_rpc()` |
| 7 | `signing.rs` | ~60 | 1408-1534 | Key derivation, seed parsing, `signing_material_for_sender()` |
| 8 | `tx_submit.rs` | ~60 | 1535-1647 | `submit_init()`, `submit_transfer()` |
| 9 | `roaming.rs` | ~150 | 1648-2116 | Cross-shard intent lifecycle, `submit_roaming_intent()`, import flow |
| 10 | `error_format.rs` | ~100 | 2117-2426 | `format_submit_transfer_error()` (~310 lines → split by status code) |
| 11 | `send_form.rs` | ~120 | 2427-2640 | `SendForm`, `SendField`, validation, decimal parsing |
| 12 | `history.rs` | ~30 | 2641-2715 | `OperationHistoryEntry`, `push_op_history()` |
| 13 | `account_view.rs` | ~120 | 2716-2961 | RPC worker, `DebugCache`, `PollSnapshot` |
| 14 | `selection.rs` | ~60 | 2962-3084 | Selection/navigation helpers |
| 15 | `tui_loop.rs` | ~300 | 3086-4499 | `run()` event loop + rendering (split render functions further) |
| 16 | `tests/` (multiple) | ~1900 | 4499-6431 | Split into `test_wallet.rs`, `test_send_form.rs`, `test_roaming.rs`, etc. |

**Priority actions (TUI):**

1. **Extract shared `TextInput` struct** — 4 modals (`BookPromptModal`, `UnlockModal`, `EncryptModal`, `SendForm`) each duplicate ~40 lines of cursor/edit logic (`clamp_cursor`, `move_left`, `move_right`, `insert_char`, `backspace`, `delete`). Shared struct eliminates ~160 lines of copy-paste.
2. **Split `run()` function (1413 lines)** — The `term.draw()` closure alone is ~500 lines. Split into: `event_loop()`, `handle_key_event()`, `draw_ui()`, `render_panels()`, `render_modals()`, `render_footer()`.
3. **Inline trivial passthroughs** — `xflow_report()` (3 lines → inline to `xflow_terminal_report`), `xflow_handoff_help()` (returns static string, used once).

---

### 2.2 `pwmd/src/transport.rs` — 3407 lines → Split into 5 modules

**Current state:** One file contains types, metrics, tick logic, dial logic, HTTP relay connect, stub transport, health tracking.

**Proposed decomposition:**

| # | Suggested Module | Lines (est.) | Extract From | Key Types/Functions |
|---|------------------|--------------|--------------|---------------------|
| 1 | `peer_types.rs` | ~200 | 1-330 | `PeerClass`, `PeerStatus`, `PeerRecord`, `TrustedPeer`, `BackoffEnvelope`, `PeerPolicyConfig`, `ClassLabel`, `PeerCloseReason`, `PeerReconnectReason` |
| 2 | `metrics.rs` | ~250 | 380-700 | `HandshakeMetrics`, `TransportCounters`, `TransportSnapshot`, `ChurnSnapshot`, `SoakConfidenceSnapshot`, counter helpers, `bounded_add_u64()`, bucket incrementers |
| 3 | `transport_tick.rs` | ~300 | 700-1070 | Tick state management, `run_transport_tick()`, `run_transport_tick_with()`, seed rotation, backoff, reconnect streaks, runaway guard |
| 4 | `dial.rs` | ~300 | 1165-1500 | `attempt_seed_connect()`, `build_local_node_hello()`, `local_hello_signing_key()`, `SeedStatus`, `PeerHelloAck`, HTTP seed connection flow |
| 5 | `peer_session.rs` | ~400 | 1500-2100 | TCP peer session management, handshake wire protocol, heartbeat, wire read/write, session close handling |
| 6 | `health.rs` | ~200 | 2100-2600 | Health aggregation, `SoakConfidence`, `ChurnSnapshot`, policy enforcement, native degraded state |
| 7 | `public_api.rs` | ~200 | 2600-3407 | Public functions: `spawn_*_loop`, `process_incoming_peer_hello`, `count_native_live_peers`, `classify_peer`, snapshot builders |

**Priority actions (transport):**

- `dial_stub_attempt()` (lines 1030-1035) — test stub function mixed with production code. Move to `#[cfg(test)]` module.
- `hello_stub()` in `federation.rs` (line 330) — same pattern, test stub in production file.
- Multiple small helper functions with 2-3 lines that could be inlined or grouped (`wire_close_reason`, `detail_with_err`, `reconnect_from_close`).

---

### 2.3 `pwm-cli/src/main.rs` — 5132 lines → Split by subcommand groups

**Current state:** CLI подкоманды разнесены по `cmd_*`; общие RPC-хелперы в `rpc_helpers.rs`; парсинг ввода/подписант/wallet-shell — **`cli_parse.rs`**, **`signer.rs`**, **`wallet_shell.rs`** (wave 12). **`exit_user_error`** — **`cli_exit.rs`** (wave 14). Дерево **clap** — **`cli_cmd.rs`** (wave 15). Диспетчер после парсинга — **`cli_dispatch::run`** (wave 16). Subprocess smoke — **`tests/cli_smoke.rs`** (wave 13). **`main.rs`** — реэкспорты, **`#[cfg(test)] mod tests`**, **`main()`**.

**Proposed decomposition:**

| # | Suggested Module | Lines (est.) | Extract From | Key Commands |
|---|------------------|--------------|--------------|--------------|
| 1 | `cli_config.rs` | ~120 | 1-260 | `Cli` struct, `http_client_for_rpc()`, timeout parsing, home dir resolution |
| 2 | `cmd_key.rs` | ~60 | key-gen commands | `KeyGen` |
| 3 | `cmd_genesis.rs` | ~150 | genesis commands | `GenesisBuild`, `genesis-export` |
| 4 | `cmd_addr.rs` | ~120 | addr-derive/bruteforce (**реализовано** — `run_addr_derive`, `run_addr_bruteforce`, persist/resume/format helpers) | `addr-derive`, `addr-bruteforce` |
| 5 | `cmd_tx.rs` | ~400 | tx-* commands | `tx-init`, `tx-transfer`, `tx-stake`, `tx-unstake`, `tx-burn-mark` |
| 6 | `cmd_roaming.rs` | ~300 | inter-shard commands | `tx-export`, `tx-import`, `tx-handoff-register` |
| 7 | `cmd_wallet.rs` | ~300 | wallet-* commands | wallet-create, recover, show, backup, account CRUD |
| 8 | `cmd_book.rs` | ~100 | address-book commands | `address-book-add`, `address-book-remove` |
| 9 | `cmd_offchain.rs` | ~80 | offchain commands | `off-demo` (локально); при появлении — `offchain-sign` и др. |
| 10 | `rpc_helpers.rs` | ~150+ | перенесено из `main.rs` | JSON/account meta, nonce fetch/init, recipient preflight, post signed tx, handoff JSON read |
| 11 | `tests/` (multiple) | ~1000 | 4000-5132 | Split into `test_wallet.rs`, `test_tx.rs`, `test_roaming.rs` |

**Priority actions (CLI):**

- `resolve_home_dir()`, `expand_tilde_path()`, `resolve_wallet_out_path()` — wallet path utilities that should live in `pwm-core::wallet_io` or shared crate, also used by TUI via different code path.
- `nonce_404_account_hint()` — exact duplicate of same function in `pwm-tui` (line 1271).

---

### 2.4 `pwm-cli/src/wallet.rs` — 2384 lines → Split into 3-4 modules

**Current state (wave17):** монолит заменён каталогом **`crates/pwm-cli/src/wallet/`** — **`types.rs`**, **`store.rs`** (файловый слой), **`crypto.rs`**, **`account.rs`**, **`address_book.rs`**, фасад **`mod.rs`** с **`pub use`** для прежних путей **`crate::wallet::…`**.

**Proposed decomposition:**

| # | Suggested Module | Lines (est.) | Extract From | Key Types/Functions |
|---|------------------|--------------|--------------|---------------------|
| 1 | `wallet_types.rs` | ~200 | types only | `WalletYaml`, `WalletYamlV3`, `WalletAccountEntry`, `WalletSecretPayload`, `WalletProtection` |
| 2 | `wallet_io.rs` | ~400 | file operations | `load_wallet_yaml_upgrade()`, `save_wallet_v3_new()`, `backup_wallet_file()`, `recover_wallet_file()`, `wallet_secrets()` |
| 3 | `wallet_crypto.rs` | ~300 | encryption | `seal_wallet_secret_plaintext()`, `open_wallet_secret_ciphertext()`, passphrase handling |
| 4 | `wallet_account.rs` | ~400 | account CRUD | `wallet_account_add()`, `wallet_account_list()`, `wallet_account_remove()`, `wallet_account_use()` |
| 5 | `wallet_address_book.rs` | ~200 | address book | `wallet_address_book_add()`, `wallet_address_book_remove()` |

---

### 2.5 `pwmd/src/api.rs` — 1957 lines → Split by endpoint groups

**Current state (wave18):** монолит заменён каталогом **`crates/pwmd/src/api/`** — **`types.rs`**, **`router.rs`**, **`common.rs`**, **`handlers_status`**, **`handlers_account`**, **`handlers_tx`**, **`handlers_roaming`**, **`handlers_peer`**, **`handlers_federation`**, фасад **`mod.rs`**.

**Proposed decomposition:**

| # | Suggested Module | Lines (est.) | Extract From | Key Handlers |
|---|------------------|--------------|--------------|--------------|
| 1 | `handlers_status.rs` | ~200 | status endpoints | `/v1/status`, `/v1/head` |
| 2 | `handlers_tx.rs` | ~200 | tx endpoints | `/v1/tx` submit, validation |
| 3 | `handlers_account.rs` | ~200 | account endpoints | `/v1/account/{id}`, `/v1/accounts` |
| 4 | `handlers_peer.rs` | ~150 | peer endpoints | `/v1/peer/hello`, `/v1/dev/peers` |
| 5 | `handlers_roaming.rs` | ~200 | roaming endpoints | `/v1/roaming-intents`, `/v1/export-readiness` |
| 6 | `handlers_federation.rs` | ~100 | federation endpoints | `/v1/federation/shards` |
| 7 | `router.rs` | ~100 | router builder | `router()` function, CORS setup |
| 8 | `types.rs` | ~150 | output types | `StatusOut`, `HeadOut`, `AcctOut`, etc. |

---

### 2.6 `pwmd/src/snapshot.rs` — 1649 lines → Split into 3 modules

**Current state (wave19):** монолит заменён каталогом **`crates/pwmd/src/snapshot/`** — **`types.rs`** (wire + версии + преобразования формата), **`io.rs`** (`load_snapshot`, `save_snapshot`, `validate_snapshot`), **`genesis.rs`** (`GenesisFileV4`, `load_genesis_bundle`, `snapshot_genesis_accounts`), фасад **`mod.rs`** с **`pub use genesis::load_genesis_bundle`** для **`lib.rs`** и **`pub(crate) use`** для путей **`crate::snapshot::…`** из **`bootstrap`**, **`lifecycle`**, **`relay`**, **`api/common`**, **`tests/prelude`**.

**Proposed decomposition:**

| # | Suggested Module | Lines (est.) | Extract From | Key Functions |
|---|------------------|--------------|--------------|---------------|
| 1 | `snapshot_types.rs` | ~200 | wire types | `SnapshotData`, `SnapshotDataV2`, `SnapshotDataV1`, `SnapshotRoamingWire`, version constants |
| 2 | `snapshot_io.rs` | ~300 | load/save | `load_snapshot()`, `save_snapshot()`, `validate_snapshot()`, `load_genesis_bundle()` |
| 3 | `genesis_loader.rs` | ~200 | genesis parsing | `GenesisFileV4`, `load_genesis_bundle()`, validator key decryption |

---

## 3. Copy-Paste & Redundancy Findings

### 3.1 Cross-Crate Duplicates (pwm-cli ↔ pwm-tui)

| Duplicated Code | pwm-cli Location | pwm-tui Location | Recommendation |
|-----------------|------------------|------------------|----------------|
| `parse_rpc_timeout_from_env()` | `main.rs:50` | `main.rs:155` | **Extract to `pwm-core::rpc` or new `pwm-rpc-client` shared crate** |
| `http_client_for_rpc()` / `http_client()` | `main.rs:124` | `main.rs:169` | **Extract shared HTTP client builder** |
| `nonce_404_account_hint()` | `main.rs:233` | `main.rs:1271` | **Extract to shared crate** |
| `parse_u64_json_field()` / `parse_u64_value()` | `main.rs:188` | `main.rs:1298` | **Extract to shared crate** |
| Wallet passphrase prompt logic | `main.rs:167-186` | Similar in TUI | **Extract to `pwm-core::wallet_io`** |
| Decimal PWM formatting (`format_pwm`) | — | `main.rs:130-142` | **Extract to `pwm-core::display`** (also needed in CLI for consistent output) |
| `parse_decimal_pwm_units()` | CLI tx parsing | `main.rs:2490-2640` | **Extract to `pwm-core::display`** |

### 3.2 Internal Copy-Paste (within single files)

| Pattern | File | Occurrences | Lines | Recommendation |
|---------|------|-------------|-------|----------------|
| **Text editor methods** (`clamp_cursor`, `move_left`, `move_right`, `move_home`, `move_end`, `insert_char`, `backspace`, `delete`) | `pwm-tui/main.rs` | 4 structs: `BookPromptModal`, `UnlockModal`, `EncryptModal`, `SendForm` | ~160 lines | **Extract `struct TextInput` with shared impl** |
| **Passphrase field handling** | `pwm-tui/main.rs` | `UnlockModal` vs `EncryptModal` | ~80 lines | Compose `EncryptModal` from two `TextInput` fields |
| **`parse_seed_hex` + `derive_sender_for_from`** | `pwm-tui/main.rs` | Lines 1408-1431 vs 1432-1441 | ~30 lines | `derive_sender_for_from` should call `parse_seed_hex` |
| **`fetch_nonce` + `preflight_recipient_rpc`** | `pwm-tui/main.rs` | Lines 1319-1341 vs 1360-1406 | ~50 lines | Extract `fetch_account_info()` shared helper |
| **Genesis mismatch guard** | `pwmd/transport.rs` | Lines 1338-1365 (seed connect) vs `api.rs` / `relay.rs` | ~30 lines | Extract `record_genesis_mismatch()` helper |

### 3.3 Logic Duplicated Across Crates

| Logic | In `pwm-cli` | In `pwm-tui` | In `pwmd` | Recommendation |
|-------|-------------|--------------|-----------|----------------|
| Key derivation (SLIP-0010) | ✅ | ✅ | ✅ (`hd.rs`) | CLI/TUI should use `pwm-core::hd` |
| Wallet encryption (ChaCha20+PBKDF2) | ✅ | ✅ | ✅ (`wallet_crypto.rs`) | CLI/TUI should use `pwm-core::wallet_crypto` |
| Account ID formatting (bech32DX) | ✅ | ✅ | ✅ (`types.rs`) | Already shared ✅ |
| Recipient domain validation | ✅ | ✅ (inline) | ✅ (`tx_policy.rs`) | TUI should use `pwm-core::address_book` |
| `resolve_home_dir()` + tilde expansion | ✅ | (different impl) | — | **Extract to `pwm-core::wallet_io`** |
| Wallet YAML write/backup | ✅ (`wallet.rs`) | ✅ (inline in `main.rs`) | — | **Extract to `pwm-core::wallet_io`** |

---

## 4. Dead Code Findings

### 4.1 `#[allow(dead_code)]` Markers

| Location | Code | Assessment |
|----------|------|------------|
| `pwm-cli/src/wallet.rs:161` | `#[allow(dead_code)]` on struct fields | Some wallet YAML fields may not be used in all code paths. Review if `schema_version`, `country_code_label`, etc. are actively read. |
| `pwm-cli/src/bruteforce.rs:37` | `matches_flags_mask()` | Marked dead but potentially useful for future flag-based address derivation. Consider removing or adding tests. |
| `pwm-cli/src/bruteforce.rs:89` | `brute_force_domain_flags()` | Marked dead but is the core bruteforce engine. The `#[allow(dead_code)]` is for non-test builds. Consider `#[cfg(test)]` or make public. |
| `pwmd/src/roaming.rs:278` | `#[allow(dead_code)]` on struct fields | `ExportReadiness` fields may be partially unused. Review. |

### 4.2 Potentially Unused Functions

| Function | File | Lines | Assessment |
|----------|------|-------|------------|
| `xflow_report()` | `pwm-tui/main.rs` | 1674-1676 | 3-line passthrough to `xflow_terminal_report`. Inline it. |
| `xflow_handoff_help()` | `pwm-tui/main.rs` | 1678-1680 | Returns static string, used once. Inline. |
| `fetch_account_balance_raw()` | `pwm-tui/main.rs` | 1975-2005 | Called only from `format_balance_verify_step5`. Candidate for merge. |
| `dial_stub_attempt()` | `pwmd/transport.rs` | — | **Done (S15-O):** logic inlined into `run_transport_tick` closure (minimal loop dial simulation stays in prod). |
| `run_transport_tick()` | `pwmd/transport.rs` | — | Retained for `spawn_transport_loop`; stub dial inlined (see above). |
| `hello_stub()` | `pwmd/federation.rs` | — | **Done:** lives under `#[cfg(test)] mod tests` only. |
| `default_wallet_candidate()` | `pwm-tui/main.rs` | 1253-1256 | Only used in one test. Consider if test adds value beyond `default_wallet_if_present`. |
| `_unused_` placeholder | `pwm-tui/main.rs` | 5187, 5220 | Test code uses `PathBuf::from("_unused_")`. Cosmetic but worth cleanup. |

### 4.3 TODO markers vs tracked debt

Inline `TODO` / `FIXME` comments are **sparse** by policy; larger items live in **[MVP-checklist.md](../MVP-checklist.md)**, **[tasks/](../tasks/)**, sprint reviews under **[reviews/](reviews/)**, and refactoring sections of this document. That keeps grep noise low but shifts discovery to planning artifacts.

**Optional:** use `// TODO(scope): brief note` for small local reminders (`scope` = `transport`, `tui`, `cli`, `protocol`, …). Prefer a ticket/checklist row for anything that affects release or operator behavior.

---

## 5. Shared Code Extraction Opportunities

### 5.1 Proposed New Crate: `pwm-rpc-client`

Currently, both `pwm-cli` and `pwm-tui` independently implement HTTP client construction, timeout parsing, nonce fetching, and account lookup. A shared crate would eliminate this.

```
pwm-rpc-client/
├── Cargo.toml
└── src/
    ├── lib.rs          # Re-exports
    ├── client.rs       # HTTP client builder, timeout config
    ├── account.rs      # fetch_nonce, fetch_account_info, parse helpers
    ├── tx_submit.rs    # submit_init, submit_transfer
    └── roaming.rs      # roaming intent submission, export-readiness
```

**Dependencies:** `reqwest` (blocking), `serde_json`, `pwm-core`

### 5.2 Expand `pwm-core` with New Modules

| New Module | Purpose | Current Location (to move) |
|------------|---------|---------------------------|
| `pwm-core::wallet_io` | Wallet file I/O, tilde expansion, backup, recovery | `pwm-cli/wallet.rs` (~300 lines), `pwm-tui/main.rs` (~100 lines) |
| `pwm-core::display` | PWM amount formatting, decimal parsing | `pwm-tui/main.rs:130-142`, `pwm-tui/main.rs:2490-2640` |
| `pwm-core::rpc` | Timeout parsing, HTTP client config | `pwm-cli/main.rs:44-55`, `pwm-tui/main.rs:147-160` |

### 5.3 Code Already Properly Shared ✅

| Module | Purpose | Used By |
|--------|---------|---------|
| `pwm-core::wallet_crypto` | Wallet encryption/decryption | `pwm-cli`, `pwm-tui`, `pwmd` |
| `pwm-core::wallet_read` | Read-only wallet parsing | `pwm-tui`, `pwmd` tests |
| `pwm-core::types` | Account ID, formatting | All crates |
| `pwm-core::hd` | HD derivation | All crates |
| `pwm-core::tx` | Transaction types | All crates |
| `pwm-core::address_book` | Address validation | `pwm-cli`, `pwmd` |
| `pwm-core::domain_index` | Domain registry | All crates |

---

## 6. Additional Optimization Recommendations

### 6.1 Dependency Alignment

| Issue | Current State | Recommendation |
|-------|---------------|----------------|
| `serde_yaml` versions | Both `0.9` in core, CLI, TUI | ✅ Aligned |
| `ed25519-dalek` version | `2` everywhere | ✅ Aligned |
| `reqwest` features | `pwm-tui` and `pwm-cli` both use `blocking` + `json` | Consider extracting to shared crate to avoid feature duplication |
| `slip10_ed25519` | Duplicated in all 4 crates | ✅ Workspace-level would be cleaner |

### 6.2 Test Organization

| Crate | Current State | Recommendation |
|-------|---------------|----------------|
| `pwmd/lib.rs` | ~5000 lines of inline `#[cfg(test)]` module | Split into `tests/api_tests.rs`, `tests/transport_tests.rs`, `tests/roaming_tests.rs`, `tests/snapshot_tests.rs` |
| `pwm-tui/main.rs` | ~1900 lines of inline tests at file end | Split into `tests/wallet_tests.rs`, `tests/send_form_tests.rs`, `tests/roaming_tests.rs` |
| `pwm-cli` | Inline/unit tests largely in `src/tests/mod.rs`; subprocess smoke in `tests/cli_smoke.rs` (wave13) | Optional split of remaining groups into named files under `tests/` / `src/tests/` |
| `pwmd/slice20_e2e_tests.rs` | Dedicated file, 695 lines | ✅ Good pattern — follow for other test groups |

### 6.3 Module Complexity Hotspots

| Function | File | Lines | Complexity | Recommendation |
|----------|------|-------|------------|----------------|
| `run()` (TUI event loop) | `pwm-tui/main.rs` | ~1413 | Very High | Split into `event_loop()`, `handle_key()`, `render()` |
| `format_submit_transfer_error()` | `pwm-tui/main.rs` | ~310 | High | Split by HTTP status code branches |
| `submit_roaming_intent()` | `pwm-tui/main.rs` | ~158 | High | Split into preflight/export/poll/import steps |
| `run_transport_tick_with()` | `pwmd/transport.rs` | ~300+ (with helpers) | High | Already planned for decomposition |
| `load_snapshot()` | `pwmd/snapshot.rs` | ~200 | Medium-High | Split version-specific parsing logic |
| `validate_snapshot()` | `pwmd/snapshot.rs` | ~150 | Medium | Split chain replay from state validation |

### 6.4 Naming Consistency

| Issue | Examples | Recommendation |
|-------|----------|----------------|
| Mixed naming for "shard" vs "domain" | `ShardId` (A/B) vs `domain_hi` (0x10/0x20) vs `cluster_domain_hi` | Document the mapping clearly; consider deprecating `ShardId` in favor of domain-based identity |
| `transport_real` vs `transport_peer_listen` vs `transport_peer_seed` | CLI flags in `pwmd/main.rs` | Group transport flags under `--transport.*` namespace in help text |
| `--shard` deprecated but kept | `pwmd/main.rs:19-20` | Add `#[deprecated]` attribute to compiler-enforce migration |

---

## 7. Priority Action Plan

### Phase 1: Quick Wins (1-2 days)

| # | Action | Impact | Effort |
|---|--------|--------|--------|
| 1 | Inline `xflow_report()` and `xflow_handoff_help()` in TUI | Eliminates trivial functions | 15 min |
| 2 | Move `dial_stub_attempt()` and `run_transport_tick()` to `#[cfg(test)]` in transport.rs | Clean separation | 30 min |
| 3 | Move `hello_stub()` in federation.rs to test module | Clean separation | 15 min |
| 4 | Extract shared `TextInput` struct in TUI | Eliminates ~160 lines copy-paste | 2 hours |
| 5 | Add `#[deprecated]` attribute to `--shard` flag | Compiler-enforced migration | 15 min |

### Phase 2: Code Sharing (1 week)

| # | Action | Impact | Effort |
|---|--------|--------|--------|
| 1 | Create `pwm-core::wallet_io` module | Eliminates ~400 lines of duplicated wallet I/O | 1 day |
| 2 | Create `pwm-core::display` module | Unified PWM formatting across CLI/TUI | 2 hours |
| 3 | Create `pwm-core::rpc` module for timeout/client config | Eliminates duplicated HTTP setup | 2 hours |
| 4 | Refactor TUI to use shared `pwm-core` modules | Reduces TUI by ~200 lines | 1 day |
| 5 | Refactor CLI to use shared `pwm-core::wallet_io` | Reduces CLI by ~100 lines | 1 day |

### Phase 3: Module Decomposition (2-3 weeks)

| # | Action | Impact | Effort |
|---|--------|--------|--------|
| 1 | Split `pwm-tui/main.rs` into 15 modules (per section 2.1) | Reduces 6431 lines → ~400 lines main | 1 week |
| 2 | Split `pwmd/src/transport.rs` into 5-7 modules (per section 2.2) | Reduces 3407 lines → manageable chunks | 3 days |
| 3 | Split `pwm-cli/src/main.rs` by subcommand groups (per section 2.3) | Reduces 5132 lines → focused modules | 3 days |
| 4 | Split `pwm-cli/src/wallet.rs` into 3-4 modules (per section 2.4) | Reduces 2384 lines → focused modules | 2 days |
| 5 | Split `pwmd/src/api.rs` by endpoint groups (per section 2.5) | Reduces 1957 lines → focused handlers | 2 days |
| 6 | Split `pwmd/src/snapshot.rs` into 3 modules (per section 2.6) | Reduces 1649 lines → focused modules | 1 day |

### Phase 4: Test Reorganization (1 week)

| # | Action | Impact | Effort |
|---|--------|--------|--------|
| 1 | Move `pwmd/lib.rs` inline tests to `tests/` directory | Reduces 6248 → ~1200 lines of production code | 2 days |
| 2 | Move `pwm-tui/main.rs` tests to `tests/` directory | Reduces 6431 → ~4500 lines | 1 day |
| 3 | Move `pwm-cli/main.rs` tests to `tests/` directory | Reduces 5132 → ~4100 lines | 1 day |
| 4 | Split `pwm-core/state.rs` tests (1193 lines, ~65% tests) | Already acceptable but could be cleaner | 1 day |

---

## 8. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Breaking changes during module extraction | Medium | High | Maintain backward-compatible public API in `pwm-core` |
| Test regressions from reorganization | Low | Medium | Run full test suite after each extraction step |
| Merge conflicts during parallel decomposition | Medium | Low | Decompose crates independently (TUI first, then CLI, then pwmd) |
| Shared crate introduces unwanted coupling | Low | Medium | Keep `pwm-core` focused on protocol primitives, not application logic |

---

## 9. Summary Metrics

| Metric | Value | Target |
|--------|-------|--------|
| Total source lines (crates/) | ~25,000 | — |
| Lines in modules >1500 | ~28,000 (counting tests) | <10,000 after decomposition |
| Copy-paste code blocks identified | 12 | 0 after extraction |
| Dead code markers | 4 | Review and remove |
| Functions >100 lines | 8 | <5 after refactoring |
| Proposed new modules | ~30 | — |
| Proposed new crate | 1 (`pwm-rpc-client`, optional) | Consider merging into `pwm-core` instead |
