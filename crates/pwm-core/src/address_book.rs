//! Wallet `address_book` entries and YAML-safe append (preserves unrelated keys).

use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::path::Path;

use crate::domain_index::{self, DomainCategory};
use crate::hd::domain_of_account_id;
use crate::types::{
    account_id_to_bech32dx, format_domain_for_display, parse_acct_id_mig, AccountId,
};

/// One row in wallet `address_book`. Legacy wallets use a bare string; optional UI label uses a map.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum AddressBookEntry {
    AddressOnly(String),
    WithLabel {
        address: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
}

impl AddressBookEntry {
    pub fn address_str(&self) -> &str {
        match self {
            AddressBookEntry::AddressOnly(s) => s.as_str(),
            AddressBookEntry::WithLabel { address, .. } => address.as_str(),
        }
    }

    pub fn label(&self) -> Option<&str> {
        match self {
            AddressBookEntry::AddressOnly(_) => None,
            AddressBookEntry::WithLabel { label, .. } => label.as_deref(),
        }
    }

    pub fn account_id(&self) -> Result<AccountId, String> {
        parse_acct_id_mig(self.address_str().trim()).map_err(|e| e.to_string())
    }
}

/// Recipient domain rules for transfers / `address_book` registration (reserve, witness, index).
///
/// `cli_field`: when `Some("--to")` / `Some("--beneficiary")`, errors use CLI-style `Invalid value for …`
/// prefixes. When `None`, use short neutral messages for wallet tooling.
pub fn validate_recipient_domain_policy(
    recipient: &AccountId,
    cli_field: Option<&str>,
) -> Result<(), String> {
    let domain_raw = domain_of_account_id(recipient) as u32;
    let domain_display = format_domain_for_display(domain_raw).0;
    match domain_index::category_for_raw(domain_raw) {
        Some(DomainCategory::Reserve) => Err(if let Some(field) = cli_field {
            format!(
                "Invalid value for {field}: domain '{domain_display}' is reserve and cannot be used as transaction recipient."
            )
        } else {
            format!("recipient domain '{domain_display}' is reserve and cannot be used.")
        }),
        Some(DomainCategory::Witness) => Err(if let Some(field) = cli_field {
            format!(
                "Invalid value for {field}: domain '{domain_display}' is witness-only and cannot receive funds in regular user flow."
            )
        } else {
            format!("recipient domain '{domain_display}' is witness-only.")
        }),
        Some(DomainCategory::Regulatory) | Some(DomainCategory::Sector) => {
            if domain_index::lookup_for_display(domain_raw).is_none() {
                Err(if let Some(field) = cli_field {
                    format!(
                        "Invalid value for {field}: domain '{domain_display}' is not recognized by domain index."
                    )
                } else {
                    format!("recipient domain '{domain_display}' is not recognized.")
                })
            } else {
                Ok(())
            }
        }
        None => Err(if let Some(field) = cli_field {
            format!(
                "Invalid value for {field}: domain '{domain_display}' is not recognized by domain index."
            )
        } else {
            format!("recipient domain '{domain_display}' is not recognized.")
        }),
    }
}

/// Same as [`validate_recipient_domain_policy`] with neutral error wording (wallet / book-add).
pub fn validate_recipient_address_policy(recipient: &AccountId) -> Result<(), String> {
    validate_recipient_domain_policy(recipient, None)
}

pub fn address_book_contains(entries: &[AddressBookEntry], to: &AccountId) -> bool {
    entries
        .iter()
        .any(|e| e.account_id().ok().as_ref() == Some(to))
}

fn yaml_item_account_id(item: &Value) -> Option<AccountId> {
    match item {
        Value::String(s) => parse_acct_id_mig(s.trim()).ok(),
        Value::Mapping(map) => map
            .get(&Value::String("address".into()))
            .and_then(|v| v.as_str())
            .and_then(|s| parse_acct_id_mig(s.trim()).ok()),
        _ => None,
    }
}

/// Append one entry to `address_book` in a wallet YAML file without re-serializing the whole struct
/// (keeps encrypted/plaintext fields intact). Stores canonical bech32dx only.
/// Appends an entry to the wallet YAML address book section.
pub fn append_addr_book(
    path: &Path,
    address_input: &str,
    label: Option<&str>,
) -> Result<(), String> {
    let id = parse_acct_id_mig(address_input.trim()).map_err(|e| e.to_string())?;
    validate_recipient_address_policy(&id)?;
    let canonical = account_id_to_bech32dx(&id);

    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut root: Value = serde_yaml::from_str(&raw).map_err(|e| e.to_string())?;
    let m = root
        .as_mapping_mut()
        .ok_or_else(|| "wallet YAML root must be a mapping".to_string())?;

    let book_key = Value::String("address_book".into());
    let book_val = m
        .entry(book_key)
        .or_insert_with(|| Value::Sequence(Default::default()));
    let seq = book_val
        .as_sequence_mut()
        .ok_or_else(|| "address_book must be a YAML sequence".to_string())?;

    for item in seq.iter() {
        if yaml_item_account_id(item).as_ref() == Some(&id) {
            return Err("address is already in address_book".into());
        }
    }

    let new_item = if let Some(l) = label.map(str::trim).filter(|s| !s.is_empty()) {
        let mut mm = serde_yaml::Mapping::new();
        mm.insert(
            Value::String("address".into()),
            Value::String(canonical.clone()),
        );
        mm.insert(Value::String("label".into()), Value::String(l.to_string()));
        Value::Mapping(mm)
    } else {
        Value::String(canonical)
    };
    seq.push(new_item);

    let out = serde_yaml::to_string(&root).map_err(|e| e.to_string())?;
    std::fs::write(path, out).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `address_book_contains` parses mixed entry types (formerly `address_book_contains_uses_account_id_parse`).
    #[test]
    fn ab_contains_acct_parse() {
        let id = [2u8; 32];
        let hex_id = hex::encode(id);
        let e = AddressBookEntry::AddressOnly(hex_id.clone());
        assert!(address_book_contains(&[e], &id));
        let e2 = AddressBookEntry::WithLabel {
            address: hex_id,
            label: Some("x".into()),
        };
        assert!(address_book_contains(&[e2], &id));
    }

    /// Reserve-domain errors differ for CLI vs neutral wording (formerly `validate_recipient_domain_policy_cli_vs_neutral_wording`).
    #[test]
    fn dom_err_wording_modes() {
        let mut id = [0u8; 32];
        id[0] = 0xE0;
        id[1] = 0x03;
        let cli = validate_recipient_domain_policy(&id, Some("--to")).expect_err("cli");
        assert!(cli.contains("Invalid value for --to"));
        let neutral = validate_recipient_domain_policy(&id, None).expect_err("neutral");
        assert!(neutral.starts_with("recipient domain"));
    }

    /// YAML append rejects duplicate book entry (formerly `append_wallet_yaml_address_book_rejects_duplicate_same_account_id`).
    #[test]
    fn ab_yaml_dup_err() {
        let path = std::env::temp_dir().join(format!(
            "pwm_core_addrbook_dup_{}.yaml",
            rand::random::<u128>()
        ));
        let (_, _, _, id) = crate::hd::brute_cluster_address(&[103u8; 32], 0x2C00, 500_000)
            .expect("brute for dup test");
        let hex_id = hex::encode(id);
        std::fs::write(&path, format!("address_book:\n  - \"{hex_id}\"\n")).unwrap();
        let err = append_addr_book(&path, hex_id.as_str(), None).expect_err("duplicate");
        assert!(err.contains("already in address_book"));
        let _ = std::fs::remove_file(&path);
    }
}
