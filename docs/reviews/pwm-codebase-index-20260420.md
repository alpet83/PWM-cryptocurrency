# PWM Codebase Index (2026-04-20)

## Scope / source

- **Primary source:** CQDS cached index via MCP `cq_files_ctl` → `get_index` with `project_id=5`.
- Snapshot metadata from index:
  - `project_name`: `pwm-protocol`
  - `context_date`: `2026-04-20`
  - `last_build_kind`: `full`
  - indexed files: `3598`
  - indexed entities: `2055`
- Practical caveats:
  - cache includes build artifacts (`target/...`), so raw entity counts are noisy;
  - part of source paths appears in both forms (`crates/...` and `pwm-protocol/crates/...`), so duplicates exist;
  - below is a **deduplicated, practical map** focused on `crates/*/src`.

## Crate map

- `pwm-core` — domain/core logic: blocks, chain, tx, state, crypto, genesis, mempool, serialization, offchain helpers.
- `pwmd` — daemon/API layer: app state wrapper, bootstrap from genesis/snapshot, router and runtime endpoints.
- `pwm-cli` — command-line client for tx creation/sign/send and nonce retrieval.
- `pwm-tui` — terminal UI client polling daemon data and rendering interactive state.

## Key entities by crate

### `pwm-core`

- Main modules from `lib.rs`: `block`, `chain`, `crypto`, `genesis`, `hd`, `mempool`, `offchain`, `ser_bin`, `state`, `tx`, `types`.
- Key structs/types:
  - `BlockHdr`, `Block` (`block.rs`)
  - `Chain` (`chain.rs`)
  - `State` (`state.rs`)
  - `SignedTx` (`tx.rs`)
- Key functions:
  - `txs_root`, `hdr_hash` (`block.rs`)
  - `hash_header_signing_payload`, `sign`, `verify` (`crypto.rs`)
  - `validate_tx_shape` (`tx.rs`)
  - genesis/HD helpers: `dev_net`, `domain_of_account_id`, `brute_cluster_address`.

### `pwmd`

- Main runtime container/types:
  - `App`, `Inner`, `HeadOut`
  - snapshot/genesis DTOs: `GenesisFile`, `SnapshotData`, `SnapshotGenesisRow`, `SnapshotStateRow`, `SnapshotStateWire`
- Core runtime functions:
  - bootstrap path: `app_from_dev_net`, `app_from_chain_boot`, `app_from_genesis`, `app_from_genesis_with_data`
  - persistence path: `load_genesis_bundle`, `snapshot_genesis_rows`, `validate_snapshot`, `load_snapshot`, `save_snapshot`
  - API assembly: `router`.

### `pwm-cli`

- Main CLI model: `Cli`
- Key flow functions:
  - input/seed/domain: `hex32`, `parse_domain`, `master_seed`, `derive_sender`
  - daemon interaction: `fetch_nonce`, `post_tx`
  - entrypoint: `main`.

### `pwm-tui`

- Core UI models:
  - `AcctRow`, `Ui`
- Main runtime functions:
  - transport/helpers: `base_url`, `debug_json`, `fetch_json`, `parse_u128`, `short_id`
  - polling/render loop: `poll_data`, `clamp_sel`, `run`
  - layout/boot: `centered_rect`, `main`.

## Runtime entrypoints and data flow summary

- Binary entrypoints:
  - `crates/pwmd/src/main.rs` → daemon process start
  - `crates/pwm-cli/src/main.rs` → one-shot CLI operations
  - `crates/pwm-tui/src/main.rs` → interactive TUI loop
- Data flow (high-level):
  1. **Core logic** lives in `pwm-core` (`tx`/`state`/`chain`/`block`).
  2. **Daemon (`pwmd`)** wraps core state in app container, initializes from genesis/snapshot, exposes router endpoints.
  3. **CLI (`pwm-cli`)** derives sender identity, gets nonce from daemon, submits signed tx.
  4. **TUI (`pwm-tui`)** polls daemon JSON endpoints (`poll_data`) and renders account/state view for operator workflow.

## Fast navigation cheatsheet

- **Transactions (`tx`)**
  - `crates/pwm-core/src/tx.rs`
  - `crates/pwm-cli/src/main.rs` (`post_tx`, `fetch_nonce`, sender derivation)
- **State transition / balances / nonce**
  - `crates/pwm-core/src/state.rs`
- **Chain/block sealing**
  - `crates/pwm-core/src/chain.rs`
  - `crates/pwm-core/src/block.rs`
- **Daemon API / RPC surface**
  - `crates/pwmd/src/lib.rs` (`router`, app bootstrap, snapshot I/O)
  - `crates/pwmd/src/main.rs`
- **TUI behavior**
  - `crates/pwm-tui/src/main.rs` (`poll_data`, `run`, UI state/layout)
- **Genesis/bootstrap**
  - `crates/pwm-core/src/genesis.rs`
  - `crates/pwmd/src/lib.rs` (`load_genesis_bundle`, `app_from_*`)
