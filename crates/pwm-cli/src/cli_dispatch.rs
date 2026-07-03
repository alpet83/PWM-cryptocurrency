//! CLI command dispatch (`run` orchestration after Parse).

use crate::{
    cli_config::is_rpc_offline, cmd_account, cmd_addr, cmd_book, cmd_genesis, cmd_key, cmd_node,
    cmd_offchain, cmd_roaming, cmd_status, cmd_tx, cmd_wallet, Cli, Cmd, WalletCmd,
};

fn command_allowed_offline(cmd: &Cmd) -> bool {
    matches!(
        cmd,
        Cmd::KeyGen
            | Cmd::GenesisBuild { .. }
            | Cmd::AddrDer { .. }
            | Cmd::AddrBruteforce { .. }
            | Cmd::Wallet { .. }
            | Cmd::OffDemo
    )
}

pub fn run(cli: Cli) {
    let rpc_base = cli.rpc.trim_end_matches('/').to_string();
    if is_rpc_offline(&rpc_base) && !command_allowed_offline(&cli.cmd) {
        crate::exit_user_error(
            "command requires live pwmd; use a real --rpc URL (offline is only for local commands such as addr-bruteforce)",
        );
    }
    let wallet_passphrase = cli.wallet_passphrase.clone();
    let genesis_passphrase = cli.genesis_passphrase.clone();
    let upgrade_wallet = cli.upgrade_wallet;
    match cli.cmd {
        Cmd::OffDemo => cmd_offchain::run_off_demo(),
        Cmd::KeyGen => cmd_key::run_keygen(),
        Cmd::Status => cmd_status::run_status(&rpc_base),
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
        } => {
            let wal_out_explicit = wallet_out.is_some();
            cmd_addr::run_addr_derive(
                master,
                domain,
                max_try,
                wallet_out,
                wal_out_explicit,
                wallet_passphrase.clone(),
                upgrade_wallet,
            )
        }
        Cmd::AddrBruteforce {
            master,
            domain,
            flags_mask,
            expected_flags,
            max_try,
            count,
            wallet_out,
            overwrite_wallet,
        } => {
            let wal_out_explicit = wallet_out.is_some();
            cmd_addr::run_addr_bruteforce(
                master,
                domain,
                flags_mask,
                expected_flags,
                max_try,
                count,
                wallet_out,
                wal_out_explicit,
                overwrite_wallet,
                wallet_passphrase.clone(),
                upgrade_wallet,
                &rpc_base,
            )
        }
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
            owner_kind,
            owner_name,
            owner_country,
            metadata_commitment,
            verification_ref,
            requested_domain_lo,
            rescue_address,
            initial_policy,
            save_activation_tx,
        } => cmd_tx::run_tx_init(
            &rpc_base,
            wallet.clone(),
            master,
            domain,
            wallet_passphrase.as_deref(),
            upgrade_wallet,
            index,
            flags,
            cmd_tx::InitV4Args {
                owner_kind,
                owner_name,
                owner_country,
                metadata_commitment,
                verification_ref,
                requested_domain_lo,
                rescue_address,
                initial_policies: initial_policy,
            },
            save_activation_tx,
        ),
        Cmd::AccountInfo { account, wallet } => {
            cmd_account::run_account_info(&rpc_base, account, wallet, upgrade_wallet)
        }
        Cmd::TxPolicySet {
            wallet,
            master,
            domain,
            index,
            policy,
            activation,
            activate_at_height,
            fee,
        } => cmd_tx::run_tx_policy_set(
            &rpc_base,
            wallet,
            master,
            domain,
            wallet_passphrase.as_deref(),
            upgrade_wallet,
            index,
            policy,
            activation,
            activate_at_height,
            fee,
        ),
        Cmd::TxPolicyActivate {
            wallet,
            master,
            domain,
            index,
            policy,
            policy_id,
            fee,
            activation_target,
            activation_tx,
            rescue_account_index,
            rescue_wallet,
            rescue_master,
            rescue_domain,
            rescue_passphrase,
        } => cmd_tx::run_tx_policy_activate(
            &rpc_base,
            wallet,
            master,
            domain,
            wallet_passphrase.as_deref(),
            upgrade_wallet,
            index,
            policy,
            policy_id,
            fee,
            activation_target,
            activation_tx,
            cmd_tx::RescueCosignArgs {
                rescue_account_index,
                rescue_wallet,
                rescue_master,
                rescue_domain,
                rescue_passphrase,
            },
        ),
        Cmd::TxPolicyDeactivate {
            wallet,
            master,
            domain,
            index,
            policy,
            policy_id,
            fee,
        } => cmd_tx::run_tx_policy_deactivate(
            &rpc_base,
            wallet,
            master,
            domain,
            wallet_passphrase.as_deref(),
            upgrade_wallet,
            index,
            policy,
            policy_id,
            fee,
        ),
        Cmd::TxSend {
            wallet,
            master,
            domain,
            index,
            to,
            amount,
            fee,
            nonce,
        } => cmd_tx::run_tx_send(cmd_tx::TxSendOpts {
            rpc_base: &rpc_base,
            wallet,
            master,
            domain,
            wallet_passphrase: wallet_passphrase.as_deref(),
            upgrade_wallet,
            index,
            to,
            amount,
            fee,
            nonce,
        }),
        Cmd::TxStake {
            wallet,
            master,
            domain,
            index,
            amount,
        } => cmd_tx::run_tx_stake(
            &rpc_base,
            wallet,
            master,
            domain,
            wallet_passphrase.as_deref(),
            upgrade_wallet,
            index,
            amount,
        ),
        Cmd::TxUnstake {
            wallet,
            master,
            domain,
            index,
            amount,
        } => cmd_tx::run_tx_unstake(
            &rpc_base,
            wallet,
            master,
            domain,
            wallet_passphrase.as_deref(),
            upgrade_wallet,
            index,
            amount,
        ),
        Cmd::TxBurnMark {
            wallet,
            master,
            domain,
            index,
            mark_amount,
            beneficiary,
            purpose,
        } => cmd_tx::run_tx_burn_mark(
            &rpc_base,
            wallet,
            master,
            domain,
            wallet_passphrase.as_deref(),
            upgrade_wallet,
            index,
            mark_amount,
            beneficiary,
            purpose,
        ),
        Cmd::TxClaim {
            wallet,
            master,
            domain,
            ..
        } => {
            let _ = (
                wallet,
                master,
                domain,
                wallet_passphrase,
                upgrade_wallet,
                rpc_base,
            );
            crate::exit_user_error("tx-claim is retired in V5");
        }
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
