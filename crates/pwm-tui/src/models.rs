//! Account table rows, wallet YAML fragments, and loaded wallet identity.

use pwm_core::display::format_pwm;
use pwm_core::AccountId;
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;
use std::time::Instant;

/// One row from `GET /v1/accounts`.
#[derive(Clone)]
pub struct AcctRow {
    pub id: AccountId,
    pub id_hex: String,
    pub balance_pwm: u128,
    pub initialized: bool,
    pub nonce: u64,
    pub marks: u32,
    pub staked: u128,
    /// From wallet `address_book` entry (optional).
    pub label: Option<String>,
}

pub const UNKNOWN_BALANCE_SENTINEL: u128 = u128::MAX;
pub const UNKNOWN_INIT_NONCE_SENTINEL: u64 = u64::MAX;

pub fn parse_u128(v: &Value) -> u128 {
    match v {
        Value::String(s) => s.parse().unwrap_or(0),
        Value::Number(n) => n.as_u64().map(|x| x as u128).unwrap_or(0),
        _ => 0,
    }
}

pub fn parse_u32(v: &Value) -> u32 {
    match v {
        Value::String(s) => s.parse().unwrap_or(0),
        Value::Number(n) => n.as_u64().and_then(|x| u32::try_from(x).ok()).unwrap_or(0),
        _ => 0,
    }
}

pub fn parse_hex_account_id(hex: &str) -> Option<AccountId> {
    let bytes = hex::decode(hex).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut id = [0u8; 32];
    id.copy_from_slice(&bytes);
    Some(id)
}

pub fn format_balance_cell(r: &AcctRow) -> String {
    if r.balance_pwm == UNKNOWN_BALANCE_SENTINEL {
        return "???".to_string();
    }
    if r.staked == 0 {
        return format_pwm(r.balance_pwm);
    }
    let bal = format_pwm(r.balance_pwm)
        .trim_end_matches(" PWM")
        .to_string();
    let staked = format_pwm(r.staked).trim_end_matches(" PWM").to_string();
    format!("{bal}/{staked}PWM")
}

pub fn format_init_cell(r: &AcctRow) -> &'static str {
    if r.nonce == UNKNOWN_INIT_NONCE_SENTINEL {
        "???"
    } else if r.initialized {
        "yes"
    } else {
        "no"
    }
}

#[derive(Clone)]
pub struct BookRecipient {
    pub id: AccountId,
    pub label: Option<String>,
}

#[derive(Clone)]
pub struct OwnedWalletAccount {
    pub id: AccountId,
    pub domain: u16,
    pub derivation_index: u32,
    pub is_active: bool,
}

#[derive(Deserialize)]
pub struct WalletV3Meta {
    pub accounts: Vec<WalletV3Acct>,
}

#[derive(Deserialize)]
pub struct WalletV3Acct {
    pub derivation_index: u32,
    pub domain_u16: u16,
    pub id_hex: String,
}

#[derive(Clone)]
pub struct WalletIdentity {
    pub account_id: AccountId,
    pub account_id_human: String,
    pub domain: u16,
    pub derivation_index: u32,
    /// Present when signing is allowed (`plaintext_dev` always; `encrypted` after unlock).
    pub signing_key: Option<ed25519_dalek::SigningKey>,
    /// For `encrypted` wallets with an active unlock session.
    pub unlock_expires_at: Option<Instant>,
    /// True when YAML mode is `encrypted` (unlock/timer UX applies).
    pub wallet_is_encrypted: bool,
    pub wallet_path: PathBuf,
    pub upgrade_wallet: bool,
    pub owned_accounts: Vec<OwnedWalletAccount>,
    /// When non-empty, right panel lists these (enriched from RPC).
    pub address_book: Vec<BookRecipient>,
    /// Placeholder for future "encrypt upgraded plaintext wallet" UX hook.
    pub encryption_prompt_hint: Option<String>,
    /// Legacy pretty entries skipped by wallet loader.
    pub ignored_legacy_pretty_entries: usize,
    /// Master seed, when available in plaintext wallet metadata.
    pub master_seed_hex: Option<String>,
    /// Decrypted wallet secret JSON (same bytes as inside the AEAD blob). Cleared on auto-lock.
    /// Never logged. Used for F4 re-key without re-entering the old passphrase.
    pub secret_payload_plaintext: Option<Vec<u8>>,
}

impl WalletIdentity {
    pub fn has_recipient(&self, id: &AccountId) -> bool {
        self.address_book.iter().any(|b| b.id == *id)
    }
}

#[cfg(test)]
mod tests {
    use super::{format_balance_cell, AcctRow};

    fn mk_acct_row(balance_pwm: u128, staked: u128) -> AcctRow {
        AcctRow {
            id: [0u8; 32],
            id_hex: String::new(),
            balance_pwm,
            initialized: false,
            nonce: 0,
            marks: 0,
            staked,
            label: None,
        }
    }

    #[test]
    fn bal_cell_keeps_rpc_balance() {
        let row = mk_acct_row(500_000_000, 500_000_000);
        assert_eq!(format_balance_cell(&row), "500/500PWM");
    }
}
