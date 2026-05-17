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
        } => cmd_tx::run_tx_init(
            &rpc_base,
            wallet,
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
        ),
        Cmd::TxPolicySet {
            wallet,
            master,
            domain,
            policy,
            activation,
            fee,
        } => cmd_tx::run_tx_policy_set(
            &rpc_base,
            wallet,
            master,
            domain,
            wallet_passphrase.as_deref(),
            upgrade_wallet,
            policy,
            activation,
            fee,
        ),
        Cmd::TxPolicyActivate {
            wallet,
            master,
            domain,
            policy,
            policy_id,
            fee,
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
            policy,
            policy_id,
            fee,
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
            policy,
            policy_id,
            fee,
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
            purpose,
        } => cmd_tx::run_tx_burn_mark(
            &rpc_base,
            wallet,
            master,
            domain,
            wallet_passphrase.as_deref(),
            upgrade_wallet,
            mark_amount,
            beneficiary,
            purpose,
        ),
        Cmd::TxClaim {
            wallet,
            master,
            domain,
            claim_mode,
            claim_units,
            all,
            anchor_ref,
            fee,
        } => {
            let mode = match cmd_tx::parse_claim_mode_cli(&claim_mode) {
                Ok(m) => m,
                Err(e) => crate::exit_user_error(&e),
            };
            let units = if all || claim_units == 0 {
                pwm_core::tx::CLAIM_ALL
            } else {
                claim_units
            };
            cmd_tx::run_tx_claim(
                &rpc_base,
                wallet,
                master,
                domain,
                wallet_passphrase.as_deref(),
                upgrade_wallet,
                mode,
                units,
                anchor_ref,
                fee,
            );
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
