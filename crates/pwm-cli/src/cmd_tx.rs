//! Local transfer and staking tx submission (`tx-init`, `tx-send`, `tx-stake`, `tx-unstake`, `tx-burn-mark`).

use crate::cli_parse::{parse_address_input, resolve_tx_send_amount};
use crate::cmd_roaming;
use crate::rpc_helpers::{fetch_nonce, post_signed_tx, preflight_recipient_init};
use crate::signer::load_tx_signer_source;
use crate::wallet::assert_tx_recipient_allowed;
use crate::wallet::load_wallet_yaml_upgrade;
use crate::{exit_user_error, http_client_for_rpc};
use pwm_core::tx::{SignedTx, TxBody};
use pwm_core::validate_recipient_domain_policy;
use std::path::PathBuf;

pub(crate) fn run_tx_init(
    rpc_base: &str,
    wallet: Option<PathBuf>,
    master: Option<String>,
    domain: Option<String>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
    index: u32,
    flags: u32,
) {
    let source = load_tx_signer_source(wallet, master, domain, wallet_passphrase, upgrade_wallet)
        .unwrap_or_else(|e| exit_user_error(&e));
    let tx = SignedTx::sign_body(
        &source.sk,
        source.dom,
        source.idx,
        0,
        TxBody::Init { index, flags },
    );
    let c = http_client_for_rpc();
    post_signed_tx(&c, rpc_base, &tx).unwrap_or_else(|e| exit_user_error(&e));
}

pub(crate) fn run_tx_send(
    rpc_base: &str,
    wallet: Option<PathBuf>,
    master: Option<String>,
    domain: Option<String>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
    to: String,
    amount: Option<u128>,
    fee: u128,
) {
    let source = load_tx_signer_source(
        wallet.clone(),
        master.clone(),
        domain,
        wallet_passphrase,
        upgrade_wallet,
    )
    .unwrap_or_else(|e| exit_user_error(&e));
    let (to_id, uri_amount) =
        parse_address_input("--to", &to).unwrap_or_else(|e| exit_user_error(&e));
    let amount = resolve_tx_send_amount(amount, uri_amount).unwrap_or_else(|e| exit_user_error(&e));
    validate_recipient_domain_policy(&to_id, Some("--to")).unwrap_or_else(|e| exit_user_error(&e));
    if master.is_none() {
        if let Some(ref wp) = wallet {
            let doc = load_wallet_yaml_upgrade(wp, upgrade_wallet).unwrap_or_else(|e| {
                exit_user_error(&format!("failed to read wallet for address_book: {e}"))
            });
            assert_tx_recipient_allowed(&doc, &to_id).unwrap_or_else(|e| exit_user_error(&e));
        }
    }
    let c = http_client_for_rpc();
    let same_hi = pwm_core::tx::same_hi_domain(&source.from, &to_id);
    if same_hi {
        preflight_recipient_init(&c, rpc_base, to_id, "tx-send")
            .unwrap_or_else(|e| exit_user_error(&e));
    } else {
        eprintln!(
            "tx-send note: target recipient preflight is unavailable in this source-RPC flow; target tx-import will reject missing/uninitialized recipients"
        );
    }
    let nonce = fetch_nonce(&c, rpc_base, source.from).unwrap_or_else(|e| exit_user_error(&e));
    let tx = SignedTx::sign_body(
        &source.sk,
        source.dom,
        source.idx,
        nonce,
        TxBody::Transfer {
            to: to_id,
            amount,
            fee,
        },
    );
    if same_hi {
        post_signed_tx(&c, rpc_base, &tx).unwrap_or_else(|e| exit_user_error(&e));
    } else {
        cmd_roaming::run_tx_send_cross_domain(rpc_base, &source, to_id, amount, fee, nonce);
    }
}

pub(crate) fn run_tx_stake(
    rpc_base: &str,
    wallet: Option<PathBuf>,
    master: Option<String>,
    domain: Option<String>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
    amount: u128,
) {
    let source = load_tx_signer_source(wallet, master, domain, wallet_passphrase, upgrade_wallet)
        .unwrap_or_else(|e| exit_user_error(&e));
    let c = http_client_for_rpc();
    let nonce = fetch_nonce(&c, rpc_base, source.from).unwrap_or_else(|e| exit_user_error(&e));
    let tx = SignedTx::sign_body(
        &source.sk,
        source.dom,
        source.idx,
        nonce,
        TxBody::Stake { amount },
    );
    post_signed_tx(&c, rpc_base, &tx).unwrap_or_else(|e| exit_user_error(&e));
}

pub(crate) fn run_tx_unstake(
    rpc_base: &str,
    wallet: Option<PathBuf>,
    master: Option<String>,
    domain: Option<String>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
    amount: u128,
) {
    let source = load_tx_signer_source(wallet, master, domain, wallet_passphrase, upgrade_wallet)
        .unwrap_or_else(|e| exit_user_error(&e));
    let c = http_client_for_rpc();
    let nonce = fetch_nonce(&c, rpc_base, source.from).unwrap_or_else(|e| exit_user_error(&e));
    let tx = SignedTx::sign_body(
        &source.sk,
        source.dom,
        source.idx,
        nonce,
        TxBody::Unstake { amount },
    );
    post_signed_tx(&c, rpc_base, &tx).unwrap_or_else(|e| exit_user_error(&e));
}

pub(crate) fn run_tx_burn_mark(
    rpc_base: &str,
    wallet: Option<PathBuf>,
    master: Option<String>,
    domain: Option<String>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
    mark_amount: u128,
    beneficiary: Option<String>,
) {
    let source = load_tx_signer_source(wallet, master, domain, wallet_passphrase, upgrade_wallet)
        .unwrap_or_else(|e| exit_user_error(&e));
    let c = http_client_for_rpc();
    let nonce = fetch_nonce(&c, rpc_base, source.from).unwrap_or_else(|e| exit_user_error(&e));
    let beneficiary = beneficiary
        .as_deref()
        .map(|v| {
            let (parsed, uri_amount) = parse_address_input("--beneficiary", v)?;
            if uri_amount.is_some() {
                return Err(
                    "URI amount is not allowed for --beneficiary in tx-burn-mark".to_string(),
                );
            }
            Ok(parsed)
        })
        .transpose()
        .unwrap_or_else(|e| {
            exit_user_error(&format!(
                "{e}. Hint: verify --rpc/PWM_RPC points to the intended source-shard node for this signer"
            ))
        });
    beneficiary
        .as_ref()
        .map(|b| validate_recipient_domain_policy(b, Some("--beneficiary")))
        .transpose()
        .unwrap_or_else(|e| {
            exit_user_error(&format!(
                "{e}. Hint: verify beneficiary domain policy and --rpc/PWM_RPC target shard"
            ))
        });
    let tx = SignedTx::sign_body(
        &source.sk,
        source.dom,
        source.idx,
        nonce,
        TxBody::BurnMark {
            mark_amount,
            beneficiary,
        },
    );
    post_signed_tx(&c, rpc_base, &tx).unwrap_or_else(|e| exit_user_error(&e));
}
