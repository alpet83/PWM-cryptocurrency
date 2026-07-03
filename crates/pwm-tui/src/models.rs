//! Account table rows, wallet YAML fragments, and loaded wallet identity.

use crate::format_amount_compact;
use pwm_core::display::PWM_RAW_SCALE;
use pwm_core::AccountId;
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Clone)]
pub struct PendingConservationRow {
    pub recipient: String,
    pub amount_pwm: u128,
    pub nonce: u64,
    pub enqueue_height: u64,
    pub execute_at_height: u64,
}

/// One row from `GET /v1/accounts`.
#[derive(Clone)]
pub struct AcctRow {
    pub id: AccountId,
    pub id_hex: String,
    pub balance_pwm: u128,
    pub initialized: bool,
    pub nonce: u64,
    pub marks: u32,
    pub marks_last_block: u64,
    pub effective_marks: Option<u32>,
    pub marks_sat_pct: Option<u8>,
    pub pending_conservation: Vec<PendingConservationRow>,
    pub staked: u128,
    pub rescue_address: Option<AccountId>,
    pub active_policies: u16,
    pub dormant_policies: u16,
    pub finalized: bool,
    pub owner_kind: String,
    pub owner_name: String,
    pub owner_country: String,
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

pub fn parse_u16(v: &Value) -> u16 {
    match v {
        Value::String(s) => s.parse().unwrap_or(0),
        Value::Number(n) => n.as_u64().and_then(|x| u16::try_from(x).ok()).unwrap_or(0),
        _ => 0,
    }
}

pub fn parse_u64(v: &Value) -> u64 {
    match v {
        Value::String(s) => s.parse().unwrap_or(0),
        Value::Number(n) => n.as_u64().unwrap_or(0),
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
    let bal = format_pwm_compact(r.balance_pwm);
    if r.staked == 0 {
        return format!("{bal}PWM");
    }
    let staked = format_pwm_compact(r.staked);
    format!("{bal}/{staked}PWM")
}

fn format_pwm_compact(raw: u128) -> String {
    // Table cells use whole PWM coins so balance and staked text obey the same K/M/B bands as marks.
    format_amount_compact(raw / PWM_RAW_SCALE)
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

pub fn format_policy_bits(mask: u16) -> String {
    if mask == 0 {
        return "-".to_string();
    }
    let mut names: Vec<&'static str> = Vec::new();
    for id in 0..16u8 {
        let bit = 1u16 << id;
        if mask & bit == 0 {
            continue;
        }
        names.push(policy_name(id).unwrap_or("unknown"));
    }
    names.join(",")
}

fn policy_name(id: u8) -> Option<&'static str> {
    match id {
        0 => Some("same_domain"),
        1 => Some("emergency_redirect"),
        2 => Some("sender_filter"),
        3 => Some("default_behavior"),
        4 => Some("cosign_required"),
        _ => None,
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
    use super::{format_balance_cell, AcctRow, UNKNOWN_BALANCE_SENTINEL};

    fn mk_acct_row(balance_pwm: u128, staked: u128) -> AcctRow {
        AcctRow {
            id: [0u8; 32],
            id_hex: String::new(),
            balance_pwm,
            initialized: false,
            nonce: 0,
            marks: 0,
            marks_last_block: 0,
            effective_marks: None,
            marks_sat_pct: None,
            pending_conservation: Vec::new(),
            staked,
            rescue_address: None,
            active_policies: 0,
            dormant_policies: 0,
            finalized: false,
            owner_kind: String::new(),
            owner_name: String::new(),
            owner_country: String::new(),
            label: None,
        }
    }

    #[test]
    fn format_balance_keeps_rpc() {
        let row = mk_acct_row(500_000_000, 500_000_000);
        assert_eq!(format_balance_cell(&row), "500/500PWM");
    }

    #[test]
    fn format_balance_small_plain() {
        let row = mk_acct_row(999_000_000, 0);
        assert_eq!(format_balance_cell(&row), "999PWM");
    }

    #[test]
    fn format_balance_compact_ranges() {
        let row = mk_acct_row(1_500_000_000, 0);
        assert_eq!(format_balance_cell(&row), "1.50KPWM");

        let row = mk_acct_row(2_500_000_000_000, 0);
        assert_eq!(format_balance_cell(&row), "2.50MPWM");

        let row = mk_acct_row(3_000_000_000_000_000, 0);
        assert_eq!(format_balance_cell(&row), "3.00BPWM");
    }

    #[test]
    fn format_balance_staked_pair() {
        let row = mk_acct_row(1_500_000_000, 2_500_000_000_000);
        assert_eq!(format_balance_cell(&row), "1.50K/2.50MPWM");
    }

    #[test]
    fn format_balance_genesis_billion() {
        let row = mk_acct_row(21_000_000_000_000_000, 0);
        assert_eq!(format_balance_cell(&row), "21.00BPWM");
    }

    #[test]
    fn format_balance_unknown_sent() {
        let row = mk_acct_row(UNKNOWN_BALANCE_SENTINEL, 0);
        assert_eq!(format_balance_cell(&row), "???");
    }
}
