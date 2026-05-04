//! serde definitions for wallet YAML v3 and legacy-compatible envelopes.

use pwm_core::AddressBookEntry;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const LEGACY_ACTIVE_ACCOUNT_KEY: &str = "active_account_id_hex";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct WalletYamlV3Account {
    pub derivation_index: u32,
    pub derivation_path: String,
    pub domain_u16: u16,
    pub flags_mask_u32: u32,
    pub expected_flags_u32: u32,
    pub flags_derived_u32: u32,
    pub id_hex: String,
    pub id_pretty: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added_at_unix_sec: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct WalletYamlV3 {
    pub schema_version: u32,
    pub mode: String,
    pub created_at_unix_sec: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country_code_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_account_id_hex: Option<String>,
    pub accounts: Vec<WalletYamlV3Account>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master_seed_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master_seed_b64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signing_key_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signing_key_b64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verifying_key_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verifying_key_b64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_payload_b64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kdf_salt_b64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aead_nonce_b64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kdf: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kdf_iters: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub address_book: Vec<AddressBookEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalletYaml {
    pub schema_version: u32,
    pub mode: String,
    pub created_at_unix_sec: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country_code_label: Option<String>,
    pub derivation_index: u32,
    #[serde(default)]
    pub derivation_path: Option<String>,
    pub domain_u16: u16,
    pub flags_mask_u32: u32,
    pub expected_flags_u32: u32,
    pub flags_derived_u32: u32,
    pub account_id_hex: String,
    pub account_id_human: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master_seed_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master_seed_b64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signing_key_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signing_key_b64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verifying_key_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verifying_key_b64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_payload_b64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kdf_salt_b64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aead_nonce_b64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kdf: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kdf_iters: Option<u32>,
    /// Registered recipients (strict pretty / canonical / legacy). When non-empty,
    /// `tx-send --wallet` accepts only `to` accounts present in this list (allow-list).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub address_book: Vec<AddressBookEntry>,
    #[serde(skip)]
    pub ignored_legacy_pretty_entries: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletAccountEntry {
    pub derivation_index: u32,
    pub derivation_path: String,
    pub id_hex: String,
    pub id_pretty: String,
    pub is_active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletAccountRemoveResult {
    pub removed_id_hex: String,
    pub new_active_id_hex: String,
    pub removed_was_active: bool,
}

impl WalletYaml {
    pub fn now_unix_sec() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before epoch")
            .as_secs()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct WalletSecretPayload {
    pub(crate) master_seed_hex: String,
    pub(crate) master_seed_b64: String,
    pub(crate) signing_key_hex: String,
    pub(crate) signing_key_b64: String,
    pub(crate) verifying_key_hex: String,
    pub(crate) verifying_key_b64: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletSecrets {
    pub master_seed_hex: String,
    pub signing_key_hex: String,
    pub verifying_key_hex: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WalletProtection {
    Encrypted { passphrase: String },
    PlaintextDev,
}
