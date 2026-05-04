//! `wallet book-*` address book helpers.

use crate::exit_user_error;
use crate::wallet::{
    load_wallet_yaml_upgrade, wallet_address_book_add, wallet_address_book_remove,
};
use pwm_core::{account_id_to_human, parse_account_id};
use std::path::PathBuf;

pub(crate) fn run_book_add(
    wallet: PathBuf,
    address: String,
    label: Option<String>,
    _upgrade_wallet: bool,
) {
    wallet_address_book_add(&wallet, &address, label.as_deref())
        .unwrap_or_else(|e| exit_user_error(&e));
    println!("ok");
}

pub(crate) fn run_book_list(wallet: PathBuf, upgrade_wallet: bool) {
    let doc = load_wallet_yaml_upgrade(&wallet, upgrade_wallet)
        .unwrap_or_else(|e| exit_user_error(&format!("failed to read wallet file: {e}")));
    if doc.address_book.is_empty() {
        println!("(address_book empty — tx-send allows any policy-valid recipient)");
    } else {
        for e in &doc.address_book {
            let id = parse_account_id(e.address_str()).unwrap_or_else(|err| {
                exit_user_error(&format!(
                    "wallet address_book contains invalid canonical address: {err}"
                ))
            });
            let mut s = account_id_to_human(&id);
            if let Some(l) = e.label() {
                s.push_str("  label=");
                s.push_str(l);
            }
            println!("{s}");
        }
    }
    if doc.ignored_legacy_pretty_entries > 0 {
        println!(
            "warning: ignored {} legacy pretty address_book entries from wallet file",
            doc.ignored_legacy_pretty_entries
        );
    }
}

pub(crate) fn run_book_remove(wallet: PathBuf, address: String) {
    wallet_address_book_remove(&wallet, &address).unwrap_or_else(|e| exit_user_error(&e));
    println!("ok");
}
