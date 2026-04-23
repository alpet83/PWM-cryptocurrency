//! PWM core: txs, state, PoA chain, pool, offchain stub.

pub mod address_book;
pub mod block;
pub mod chain;
pub mod crypto;
pub mod domain_index;
pub mod genesis;
pub mod hd;
pub mod mempool;
pub mod offchain;
pub mod ser_bin;
pub mod state;
pub mod tx;
pub mod types;
pub mod wallet_crypto;
pub mod wallet_read;

pub use chain::{Chain, SealAbort};
pub use genesis::{dev_net, GRow, GenCfg};
pub use mempool::Mpool;
pub use offchain::{merkle_root, sign_batch};
pub use state::{digest, State};
pub use tx::{validate_tx_shape, SignedTx};
pub use types::{
    account_id_to_bech32dx, account_id_to_human, format_domain_for_display, parse_account_id,
    parse_account_id_for_migration, parse_account_id_for_user_input, AccountId, BECH32DX_HRP,
    LEGACY_HUMAN_ACCOUNT_PREFIX,
};

pub use address_book::{
    address_book_contains, append_wallet_yaml_address_book, validate_recipient_address_policy,
    validate_recipient_domain_policy, AddressBookEntry,
};
pub use wallet_crypto::{
    open_wallet_secret_ciphertext, seal_wallet_secret_plaintext, WalletSealedPayload, WALLET_KDF,
    WALLET_KDF_ITERS,
};
pub use wallet_read::{
    load_wallet_read_header, normalize_wallet_header, WalletReadHeader, WalletReadLoad,
};
