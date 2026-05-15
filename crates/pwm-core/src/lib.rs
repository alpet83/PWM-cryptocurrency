//! PWM core: txs, state, PoA chain, pool, offchain stub.

pub mod address_book;
pub mod block;
pub mod bridge_commitment;
pub mod chain;
pub mod crypto;
pub mod display;
pub mod domain_index;
pub mod genesis;
pub mod hd;
pub mod mempool;
pub mod offchain;
pub mod reject_wire;
pub mod rpc;
pub mod ser_bin;
pub mod ser_json_u128;
pub mod state;
pub mod tx;
pub mod types;
pub mod wallet_crypto;
pub mod wallet_io;
pub mod wallet_read;

pub use bridge_commitment::BridgeFederationCommitment;
pub use chain::{absorb_blocks_tail, Chain, SealAbort, SealTimeMode, TAIL_BLOCK_CAP};
pub use display::{format_pwm, parse_decimal_pwm_units, PWM_RAW_SCALE};
pub use genesis::{dev_net, FundingCfg, GRow, GenCfg, RewPol, VRow, ValCfg};
pub use mempool::Mpool;
pub use offchain::{merkle_root, sign_batch};
pub use reject_wire::summarize_tx_reject_json;
pub use rpc::{blocking_http_client_rpc, parse_rpc_timeout_ms, RPC_TIMEOUT_MS_CAP};
pub use state::{digest, State};
pub use tx::{validate_tx_shape, SignedTx};
pub use types::{
    account_id_to_bech32dx, account_id_to_human, format_domain_for_display, parse_account_id,
    parse_acct_id_mig, parse_acct_id_ui, AccountId, BECH32DX_HRP, LEGACY_HUMAN_ACCOUNT_PREFIX,
};

pub use address_book::{
    address_book_contains, append_addr_book, validate_recipient_address_policy,
    validate_recipient_domain_policy, AddressBookEntry,
};
pub use wallet_crypto::{
    open_wallet_secret_ciphertext, seal_wallet_secret_plaintext, WalletSealedPayload, WALLET_KDF,
    WALLET_KDF_ITERS,
};
pub use wallet_io::{expand_tilde_path, resolve_home_dir, resolve_wallet_out_path};
pub use wallet_read::{
    load_wallet_read_header, normalize_wallet_header, WalletReadHeader, WalletReadLoad,
};
