//! Wallet CLI: keys, cluster derive, submit txs to `pwmd`.

mod bruteforce;
mod cli_cmd;
mod cli_config;
mod cli_dispatch;
mod cli_exit;
mod cli_parse;
mod cmd_addr;
mod cmd_book;
mod cmd_genesis;
mod cmd_key;
mod cmd_node;
mod cmd_offchain;
mod cmd_roaming;
mod cmd_tx;
mod cmd_wallet;
mod rpc_helpers;
mod signer;
mod wallet;
mod wallet_shell;

use clap::Parser;
pub(crate) use cli_cmd::{Cli, Cmd, WalletAccountCmd, WalletCmd};
pub(crate) use cli_config::http_client_for_rpc;
pub(crate) use cli_exit::exit_user_error;

#[cfg(test)]
pub(crate) use crate::rpc_helpers::tx_import_contract_note;
#[cfg(test)]
pub(crate) use crate::wallet::{wallet_account_add, wallet_account_list};
#[cfg(test)]
pub(crate) use cmd_roaming::{
    ensure_import_sender, get_roaming_intent_status, is_terminal_intent_status,
    parse_export_id_hex_arg, post_export_handoff, post_import_retry_inner as post_import_retry,
    post_roaming_intent, user_msg_roaming_intent_error,
};

fn main() {
    cli_dispatch::run(Cli::parse());
}

#[cfg(test)]
mod tests;
