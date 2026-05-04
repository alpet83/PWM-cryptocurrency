//! CLI command dispatch (`run` orchestration after Parse).

use crate::{
    cmd_addr, cmd_book, cmd_genesis, cmd_key, cmd_node, cmd_offchain, cmd_roaming, cmd_tx,
    cmd_wallet, Cli, Cmd, WalletCmd,
};

pub(crate) fn run(cli: Cli) {
    let rpc_base = cli.rpc.trim_end_matches('/').to_string();
    let wallet_passphrase = cli.wallet_passphrase.clone();
    let genesis_passphrase = cli.genesis_passphrase.clone();
    let upgrade_wallet = cli.upgrade_wallet;
    match cli.cmd {
        Cmd::OffDemo => cmd_offchain::run_off_demo(),
        Cmd::KeyGen => cmd_key::run_keygen(),
        Cmd::GenesisBuild {
            wallet,
            out,
            val_id,
            premine_bal,
            block_reward,
            marks_coeff,
        } => cmd_genesis::run_genesis_build(
            genesis_passphrase,
            wallet_passphrase.clone(),
            upgrade_wallet,
            wallet,
            out,
            val_id,
            premine_bal,
            block_reward,
            marks_coeff,
        ),
        Cmd::AddrDer {
            master,
            domain,
            max_try,
            wallet_out,
        } => cmd_addr::run_addr_derive(
            master,
            domain,
            max_try,
            wallet_out,
            wallet_passphrase.clone(),
        ),
        Cmd::AddrBruteforce {
            master,
            domain,
            flags_mask,
            expected_flags,
            max_try,
            wallet_out,
            overwrite_wallet,
        } => cmd_addr::run_addr_bruteforce(
            master,
            domain,
            flags_mask,
            expected_flags,
            max_try,
            wallet_out,
            overwrite_wallet,
            wallet_passphrase.clone(),
            upgrade_wallet,
            &rpc_base,
        ),
        Cmd::Wallet { cmd } => match cmd {
            WalletCmd::BookAdd {
                wallet,
                address,
                label,
            } => cmd_book::run_book_add(wallet, address, label, upgrade_wallet),
            WalletCmd::BookList { wallet } => cmd_book::run_book_list(wallet, upgrade_wallet),
            WalletCmd::BookRemove { wallet, address } => cmd_book::run_book_remove(wallet, address),
            other => {
                cmd_wallet::run_wallet_non_book(wallet_passphrase.clone(), upgrade_wallet, other)
            }
        },
        Cmd::TxInit {
            wallet,
            master,
            domain,
            index,
            flags,
        } => cmd_tx::run_tx_init(
            &rpc_base,
            wallet,
            master,
            domain,
            wallet_passphrase.as_deref(),
            upgrade_wallet,
            index,
            flags,
        ),
        Cmd::TxSend {
            wallet,
            master,
            domain,
            to,
            amount,
            fee,
        } => cmd_tx::run_tx_send(
            &rpc_base,
            wallet,
            master,
            domain,
            wallet_passphrase.as_deref(),
            upgrade_wallet,
            to,
            amount,
            fee,
        ),
        Cmd::TxStake {
            wallet,
            master,
            domain,
            amount,
        } => cmd_tx::run_tx_stake(
            &rpc_base,
            wallet,
            master,
            domain,
            wallet_passphrase.as_deref(),
            upgrade_wallet,
            amount,
        ),
        Cmd::TxUnstake {
            wallet,
            master,
            domain,
            amount,
        } => cmd_tx::run_tx_unstake(
            &rpc_base,
            wallet,
            master,
            domain,
            wallet_passphrase.as_deref(),
            upgrade_wallet,
            amount,
        ),
        Cmd::TxBurnMark {
            wallet,
            master,
            domain,
            mark_amount,
            beneficiary,
        } => cmd_tx::run_tx_burn_mark(
            &rpc_base,
            wallet,
            master,
            domain,
            wallet_passphrase.as_deref(),
            upgrade_wallet,
            mark_amount,
            beneficiary,
        ),
        Cmd::TxExport {
            wallet,
            master,
            domain,
            to,
            target_domain,
            amount,
            fee,
        } => cmd_roaming::run_tx_export(
            &rpc_base,
            wallet,
            master,
            domain,
            wallet_passphrase.as_deref(),
            upgrade_wallet,
            to,
            target_domain,
            amount,
            fee,
        ),
        Cmd::TxImport {
            wallet,
            master,
            domain,
            to,
            amount,
            export_id,
        } => cmd_roaming::run_tx_import(
            &rpc_base,
            wallet,
            master,
            domain,
            wallet_passphrase.as_deref(),
            upgrade_wallet,
            to,
            amount,
            export_id,
        ),
        Cmd::TxHandoffRegister { handoff_json } => {
            cmd_roaming::run_tx_handoff_register(&rpc_base, handoff_json)
        }
        Cmd::NodeShutdown => cmd_node::run_node_shutdown(&rpc_base),
    }
}
