//! pwm-cli as a library.
//!
//! Exposes internal modules needed by harness binaries (e.g. claim-ipv4-batch)
//! and for integration/testing. This was added to fix the V5-8 slice3 compile gate.

mod bruteforce;
mod cli_cmd;
mod cli_config;
pub mod cli_dispatch;
mod cli_exit;
pub mod cli_parse;
mod cmd_account;
mod cmd_addr;
mod cmd_book;
mod cmd_genesis;
mod cmd_key;
mod cmd_node;
mod cmd_offchain;
mod cmd_roaming;
mod cmd_tx;
mod cmd_wallet;
mod purpose_expand;
mod rpc_helpers;
pub mod signer;
mod wallet;
mod wallet_shell;

// Convenience re-exports used by harness binaries and the main CLI.
pub use cli_parse::{hex32, master_seed, parse_domain};
pub use signer::load_wallet_account_signer;
pub use wallet::{load_wallet_yaml_upgrade, wallet_account_list, wallet_secrets};

// Re-export the main CLI type for the binary wrapper.
pub use cli_cmd::Cli;
pub(crate) use cli_cmd::{Cmd, WalletAccountCmd, WalletCmd};
pub(crate) use cli_config::http_client_for_rpc;
pub(crate) use cli_exit::exit_user_error;

#[cfg(test)]
pub(crate) use crate::rpc_helpers::tx_import_contract_note;
#[cfg(test)]
pub(crate) use crate::wallet::wallet_account_add;
#[cfg(test)]
pub(crate) use cmd_roaming::{
    ensure_import_sender, get_roaming_intent_status, is_terminal_intent_status, parse_export_hex,
    post_export_handoff, post_import_retry_inner as post_import_retry, post_roaming_intent,
    roaming_intent_err,
};

#[cfg(test)]
mod tests;
