//! Address-book guards enforcing outbound recipient policy.

use pwm_core::{address_book_contains, append_addr_book, parse_acct_id_ui, AccountId};
use std::path::Path;

use crate::wallet::store::{load_wallet_yaml, save_wallet_yaml};
use crate::wallet::types::WalletYaml;

pub fn wallet_address_book_contains(wallet: &WalletYaml, to: &AccountId) -> bool {
    address_book_contains(&wallet.address_book, to)
}

/// When `address_book` is non-empty, `to` must match a registered entry (after parse).
pub fn assert_tx_recipient_allowed(wallet: &WalletYaml, to: &AccountId) -> Result<(), String> {
    if wallet.address_book.is_empty() {
        return Ok(());
    }
    if wallet_address_book_contains(wallet, to) {
        return Ok(());
    }
    Err(format!(
        "recipient not in wallet address_book ({} entries). Register: pwm wallet book-add --wallet <path> --address <ADDR>",
        wallet.address_book.len()
    ))
}

pub fn wallet_address_book_add(
    path: &Path,
    address_str: &str,
    label: Option<&str>,
) -> Result<(), String> {
    append_addr_book(path, address_str, label).map_err(|e| {
        if e.contains("already in address_book") {
            e
        } else {
            format!("wallet book-add: {e}")
        }
    })
}

pub fn wallet_address_book_remove(path: &Path, address_str: &str) -> Result<(), String> {
    let mut w = load_wallet_yaml(path)?;
    let id = parse_acct_id_ui(address_str.trim()).map_err(|e| format!("invalid --address: {e}"))?;
    let before = w.address_book.len();
    w.address_book
        .retain(|e| e.account_id().ok().as_ref() != Some(&id));
    if w.address_book.len() == before {
        return Err("address not found in address_book".into());
    }
    save_wallet_yaml(path, &w)
}
