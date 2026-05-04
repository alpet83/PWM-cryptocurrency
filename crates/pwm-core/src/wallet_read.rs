//! Minimal wallet YAML fields for identity + `address_book` parsing (TUI, thin tools).
//!
//! Extra keys in the file are ignored by serde; this avoids duplicating the full `pwm-cli` wallet struct.

use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::fs;
use std::path::Path;

use crate::address_book::AddressBookEntry;
use crate::hd::account_id_from_parts;
use crate::types::{
    account_id_to_bech32dx, account_id_to_human, parse_account_id, parse_account_id_for_migration,
};
use ed25519_dalek::SigningKey;
use slip10_ed25519::derive_ed25519_private_key;

fn default_wallet_schema_version() -> u32 {
    2
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalletReadOwnedAccount {
    pub id_hex: String,
    pub id_human: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalletReadHeader {
    #[serde(default = "default_wallet_schema_version")]
    pub schema_version: u32,
    pub mode: String,
    pub derivation_index: u32,
    #[serde(default)]
    pub derivation_path: Option<String>,
    pub domain_u16: u16,
    #[serde(default)]
    pub account_id_hex: Option<String>,
    pub account_id_human: String,
    #[serde(default)]
    pub owned_accounts: Vec<WalletReadOwnedAccount>,
    #[serde(default)]
    pub address_book: Vec<AddressBookEntry>,
    #[serde(default)]
    pub signing_key_hex: Option<String>,
    #[serde(default)]
    pub master_seed_hex: Option<String>,
    #[serde(default)]
    pub encrypted_payload_b64: Option<String>,
    #[serde(default)]
    pub kdf_salt_b64: Option<String>,
    #[serde(default)]
    pub aead_nonce_b64: Option<String>,
    #[serde(default)]
    pub kdf: Option<String>,
    #[serde(default)]
    pub kdf_iters: Option<u32>,
    #[serde(skip)]
    pub ignored_legacy_pretty_entries: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct WalletReadV3Account {
    derivation_path: String,
    derivation_index: u32,
    domain_u16: u16,
    flags_mask_u32: u32,
    expected_flags_u32: u32,
    flags_derived_u32: u32,
    id_hex: String,
    id_pretty: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct WalletReadV3Header {
    mode: String,
    #[serde(default)]
    schema_version: Option<u32>,
    #[serde(default)]
    active_account_id_hex: Option<String>,
    accounts: Vec<WalletReadV3Account>,
    #[serde(default)]
    address_book: Vec<AddressBookEntry>,
    #[serde(default)]
    signing_key_hex: Option<String>,
    #[serde(default)]
    master_seed_hex: Option<String>,
    #[serde(default)]
    encrypted_payload_b64: Option<String>,
    #[serde(default)]
    kdf_salt_b64: Option<String>,
    #[serde(default)]
    aead_nonce_b64: Option<String>,
    #[serde(default)]
    kdf: Option<String>,
    #[serde(default)]
    kdf_iters: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletReadLoad {
    pub header: WalletReadHeader,
    pub upgraded_on_load: bool,
    pub ignored_legacy_pretty_entries: usize,
}

pub fn normalize_wallet_header(
    mut wallet: WalletReadHeader,
) -> Result<(WalletReadHeader, bool), String> {
    let account_id = resolve_wallet_account_id(&wallet)?;
    let mut changed = false;
    let normalized = account_id_to_human(&account_id);
    if wallet.account_id_human != normalized {
        wallet.account_id_human = normalized;
        changed = true;
    }
    let (book_changed, ignored_legacy_pretty_entries) =
        normalize_address_book_entries(&mut wallet.address_book)?;
    if book_changed {
        changed = true;
    }
    wallet.ignored_legacy_pretty_entries = ignored_legacy_pretty_entries;
    Ok((wallet, changed))
}

fn resolve_wallet_account_id(wallet: &WalletReadHeader) -> Result<[u8; 32], String> {
    if let Some(id) = account_id_from_truth_source(wallet)? {
        ensure_domain_consistency(wallet.domain_u16, &id)?;
        return Ok(id);
    }

    let account_id_from_hex = wallet
        .account_id_hex
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|hex| {
            parse_account_id(hex).map_err(|e| format!("wallet account_id_hex is invalid: {e}"))
        })
        .transpose()?;
    let account_id_from_human = parse_account_id_for_migration(wallet.account_id_human.trim());

    if let Some(id) = account_id_from_hex {
        ensure_domain_consistency(wallet.domain_u16, &id)?;
        return Ok(id);
    }

    match account_id_from_human {
        Ok(id) => {
            ensure_domain_consistency(wallet.domain_u16, &id)?;
            Ok(id)
        }
        Err(err) => Err(format!(
            "wallet account_id_human migration error: {err}; cannot recover low-byte unambiguously without account_id_hex/canonical id"
        )),
    }
}

fn account_id_from_truth_source(wallet: &WalletReadHeader) -> Result<Option<[u8; 32]>, String> {
    let index = parse_derivation_index(wallet)?;
    if let Some(seed_hex) = wallet
        .master_seed_hex
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let seed_vec =
            hex::decode(seed_hex).map_err(|e| format!("wallet master_seed_hex is invalid: {e}"))?;
        if seed_vec.len() != 32 {
            return Err("wallet master_seed_hex must be 32-byte hex".into());
        }
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&seed_vec);
        let sk_bytes = derive_ed25519_private_key(&seed, &[0, index]);
        let sk = SigningKey::from_bytes(&sk_bytes);
        let pk = sk.verifying_key().to_bytes();
        return Ok(Some(account_id_from_parts(&pk, index)));
    }

    if let Some(signing_key_hex) = wallet
        .signing_key_hex
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let key_vec = hex::decode(signing_key_hex)
            .map_err(|e| format!("wallet signing_key_hex is invalid: {e}"))?;
        if key_vec.len() != 32 {
            return Err("wallet signing_key_hex must be 32-byte hex".into());
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&key_vec);
        let sk = SigningKey::from_bytes(&key);
        let pk = sk.verifying_key().to_bytes();
        return Ok(Some(account_id_from_parts(&pk, index)));
    }

    Ok(None)
}

fn parse_derivation_index(wallet: &WalletReadHeader) -> Result<u32, String> {
    if let Some(path) = wallet
        .derivation_path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let prefix = "m/0/";
        if !path.starts_with(prefix) {
            return Err(format!(
                "wallet derivation_path is invalid: expected 'm/0/<index>', got '{path}'"
            ));
        }
        let idx = path[prefix.len()..].parse::<u32>().map_err(|_| {
            format!("wallet derivation_path is invalid: expected numeric index, got '{path}'")
        })?;
        return Ok(idx);
    }
    Ok(wallet.derivation_index)
}

fn ensure_domain_consistency(domain_u16: u16, id: &[u8; 32]) -> Result<(), String> {
    let actual = u16::from_be_bytes([id[0], id[1]]);
    if actual != domain_u16 {
        return Err(format!(
            "wallet domain_u16/account id mismatch: domain_u16={domain_u16} account_id_domain={actual}"
        ));
    }
    Ok(())
}

fn normalize_address_book_entries(
    entries: &mut Vec<AddressBookEntry>,
) -> Result<(bool, usize), String> {
    let mut changed = false;
    let mut ignored = 0usize;
    let mut normalized = Vec::with_capacity(entries.len());
    for (idx, entry) in entries.iter().enumerate() {
        if is_legacy_pretty_address(entry.address_str()) {
            ignored += 1;
            changed = true;
            continue;
        }
        let id = parse_account_id_for_migration(entry.address_str().trim())
            .map_err(|e| format!("wallet address_book[{idx}] migration error: {e}"))?;
        let canonical = account_id_to_bech32dx(&id);
        match entry {
            AddressBookEntry::AddressOnly(address) => {
                if address.trim() != canonical {
                    changed = true;
                }
                normalized.push(AddressBookEntry::AddressOnly(canonical));
            }
            AddressBookEntry::WithLabel { address, label } => {
                if address.trim() != canonical {
                    changed = true;
                }
                normalized.push(AddressBookEntry::WithLabel {
                    address: canonical,
                    label: label.clone(),
                });
            }
        }
    }
    if normalized.len() != entries.len() {
        changed = true;
    }
    *entries = normalized;
    Ok((changed, ignored))
}

fn is_legacy_pretty_address(address: &str) -> bool {
    address.trim().starts_with("pwm1-")
}

pub fn load_wallet_read_header(
    path: &Path,
    auto_upgrade_on_load: bool,
) -> Result<WalletReadLoad, String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let schema_version = detect_schema_version(&raw)?;
    let parsed: WalletReadHeader = if schema_version == 3 {
        parse_wallet_read_v3_header(&raw)?
    } else {
        serde_yaml::from_str(&raw).map_err(|e| e.to_string())?
    };
    let (normalized, changed) = normalize_wallet_header(parsed)?;
    if auto_upgrade_on_load && changed {
        rewrite_wallet_header_fields(&raw, path, &normalized)?;
    }
    Ok(WalletReadLoad {
        ignored_legacy_pretty_entries: normalized.ignored_legacy_pretty_entries,
        header: normalized,
        upgraded_on_load: changed,
    })
}

fn detect_schema_version(raw: &str) -> Result<u32, String> {
    #[derive(Deserialize)]
    struct VersionOnly {
        schema_version: Option<u32>,
    }
    let parsed: VersionOnly = serde_yaml::from_str(raw).map_err(|e| e.to_string())?;
    Ok(parsed.schema_version.unwrap_or(2))
}

fn parse_der_idx_m0_path(path: &str) -> Result<u32, String> {
    let trimmed = path.trim();
    const PREFIX: &str = "m/0/";
    if !trimmed.starts_with(PREFIX) {
        return Err(format!(
            "wallet schema v3 derivation_path must start with '{PREFIX}', got '{trimmed}'"
        ));
    }
    trimmed[PREFIX.len()..].parse::<u32>().map_err(|_| {
        format!("wallet schema v3 derivation_path must end with u32 index, got '{trimmed}'")
    })
}

fn validate_v3_derivation_paths(parsed: &WalletReadV3Header) -> Result<(), String> {
    for account in &parsed.accounts {
        let path_idx = parse_der_idx_m0_path(&account.derivation_path)?;
        if path_idx != account.derivation_index {
            return Err(format!(
                "wallet schema v3 derivation_path {:?} does not match derivation_index {}",
                account.derivation_path, account.derivation_index
            ));
        }
    }
    Ok(())
}

fn default_v3_account(accounts: &[WalletReadV3Account]) -> Option<&WalletReadV3Account> {
    accounts
        .iter()
        .min_by_key(|a| (a.derivation_index, a.id_hex.to_ascii_lowercase()))
}

fn parse_wallet_read_v3_header(raw: &str) -> Result<WalletReadHeader, String> {
    let parsed: WalletReadV3Header = serde_yaml::from_str(raw).map_err(|e| e.to_string())?;
    validate_v3_derivation_paths(&parsed)?;
    if parsed.accounts.is_empty() {
        return Err("wallet schema v3 requires non-empty accounts".to_string());
    }
    let default_account =
        default_v3_account(&parsed.accounts).expect("validated non-empty accounts");
    Ok(WalletReadHeader {
        schema_version: 3,
        mode: parsed.mode,
        derivation_index: default_account.derivation_index,
        derivation_path: Some(default_account.derivation_path.clone()),
        domain_u16: default_account.domain_u16,
        account_id_hex: Some(default_account.id_hex.clone()),
        account_id_human: default_account.id_pretty.clone(),
        owned_accounts: parsed
            .accounts
            .iter()
            .map(|a| WalletReadOwnedAccount {
                id_hex: a.id_hex.clone(),
                id_human: a.id_pretty.clone(),
            })
            .collect(),
        address_book: parsed.address_book,
        signing_key_hex: parsed.signing_key_hex,
        master_seed_hex: parsed.master_seed_hex,
        encrypted_payload_b64: parsed.encrypted_payload_b64,
        kdf_salt_b64: parsed.kdf_salt_b64,
        aead_nonce_b64: parsed.aead_nonce_b64,
        kdf: parsed.kdf,
        kdf_iters: parsed.kdf_iters,
        ignored_legacy_pretty_entries: 0,
    })
}

fn rewrite_wallet_header_fields(
    raw: &str,
    path: &Path,
    normalized: &WalletReadHeader,
) -> Result<(), String> {
    let mut root: Value = serde_yaml::from_str(raw).map_err(|e| e.to_string())?;
    let map = root
        .as_mapping_mut()
        .ok_or_else(|| "wallet YAML root must be a mapping".to_string())?;
    map.insert(
        Value::String("account_id_human".to_string()),
        Value::String(normalized.account_id_human.clone()),
    );
    let normalized_book =
        serde_yaml::to_value(&normalized.address_book).map_err(|e| e.to_string())?;
    map.insert(Value::String("address_book".to_string()), normalized_book);
    let out = serde_yaml::to_string(&root).map_err(|e| e.to_string())?;
    fs::write(path, out).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::{load_wallet_read_header, normalize_wallet_header, WalletReadHeader};
    use crate::address_book::AddressBookEntry;
    use crate::hd::account_id_from_parts;
    use crate::types::{account_id_to_bech32dx, account_id_to_human};
    use ed25519_dalek::SigningKey;
    use slip10_ed25519::derive_ed25519_private_key;

    /// Migrates CY legacy pretty to CY/LO form (formerly `normalize_wallet_header_upgrades_legacy_pretty`).
    #[test]
    fn norm_hdr_legacy_cy_lo() {
        let mut id = [0u8; 32];
        id[0] = 0x2C;
        id[1] = 0x00;
        let expected = account_id_to_human(&id);
        let legacy = expected.replacen("CY/00", "CY", 1);
        let wallet = WalletReadHeader {
            schema_version: 2,
            mode: "plaintext_dev".into(),
            derivation_index: 7,
            derivation_path: Some("m/0/7".into()),
            domain_u16: 0x2C00,
            account_id_hex: Some(hex::encode(id)),
            account_id_human: legacy,
            owned_accounts: Vec::new(),
            address_book: Vec::new(),
            signing_key_hex: None,
            master_seed_hex: None,
            encrypted_payload_b64: None,
            kdf_salt_b64: None,
            aead_nonce_b64: None,
            kdf: None,
            kdf_iters: None,
            ignored_legacy_pretty_entries: 0,
        };
        let (normalized, changed) = normalize_wallet_header(wallet).expect("normalize");
        assert!(changed);
        assert_eq!(normalized.account_id_human, expected);
    }

    /// Loader rewrites human/AB lines when upgrading (formerly `load_wallet_read_header_rewrites_normalized_fields`).
    #[test]
    fn hdr_rewrite_yaml_disk() {
        let path = std::env::temp_dir().join(format!(
            "pwm_core_wallet_read_upgrade_{}.yaml",
            rand::random::<u128>()
        ));
        let raw = r#"mode: plaintext_dev
derivation_index: 1
derivation_path: m/0/1
domain_u16: 11390
account_id_hex: "2c7e000000000000000000000000000000000000000000000000000000000000"
account_id_human: pwm1-CY-f00000000-t0000000000000000000000000000000000000000000000000000
custom_field: keep-me
address_book:
  - address: 2c7e000000000000000000000000000000000000000000000000000000000000
    label: peer
"#;
        std::fs::write(&path, raw).unwrap();

        let loaded = load_wallet_read_header(&path, true).expect("load");
        assert!(loaded.upgraded_on_load);
        assert!(loaded.header.account_id_human.contains("CY/7E"));

        let upgraded_raw = std::fs::read_to_string(&path).unwrap();
        assert!(upgraded_raw.contains("account_id_human: pwm1-CY/7E-"));
        assert!(upgraded_raw.contains("address: pwm1"));
        assert!(upgraded_raw.contains("custom_field: keep-me"));
        assert!(upgraded_raw.contains("label: peer"));
        let _ = std::fs::remove_file(&path);
    }

    /// Ambiguous CY legacy without hex/seed source fails normalization (formerly `normalize_wallet_header_rejects_ambiguous_legacy_without_reliable_source`).
    #[test]
    fn norm_hdr_ambig_legacy_fail() {
        let wallet = WalletReadHeader {
            schema_version: 2,
            mode: "plaintext_dev".into(),
            derivation_index: 7,
            derivation_path: Some("m/0/7".into()),
            domain_u16: 0x2C7E,
            account_id_hex: None,
            account_id_human:
                "pwm1-CY-f00000000-t0000000000000000000000000000000000000000000000000000".into(),
            owned_accounts: Vec::new(),
            address_book: Vec::new(),
            signing_key_hex: None,
            master_seed_hex: None,
            encrypted_payload_b64: None,
            kdf_salt_b64: None,
            aead_nonce_b64: None,
            kdf: None,
            kdf_iters: None,
            ignored_legacy_pretty_entries: 0,
        };
        let err = normalize_wallet_header(wallet).expect_err("must fail migration");
        assert!(err.contains("cannot recover low-byte"));
    }

    /// Already strict pretty + encrypted payloads stay untouched (formerly `normalize_wallet_header_no_change_for_strict_pretty`).
    #[test]
    fn norm_hdr_strict_no_op() {
        let id = [9u8; 32];
        let human = account_id_to_human(&id);
        let wallet = WalletReadHeader {
            schema_version: 2,
            mode: "encrypted".into(),
            derivation_index: 3,
            derivation_path: Some("m/0/3".into()),
            domain_u16: 0x0909,
            account_id_hex: None,
            account_id_human: human.clone(),
            owned_accounts: Vec::new(),
            address_book: vec![AddressBookEntry::AddressOnly(account_id_to_bech32dx(&id))],
            signing_key_hex: None,
            master_seed_hex: None,
            encrypted_payload_b64: Some("abc".into()),
            kdf_salt_b64: Some("salt".into()),
            aead_nonce_b64: Some("nonce".into()),
            kdf: Some("pbkdf2-hmac-sha256".into()),
            kdf_iters: Some(100_000),
            ignored_legacy_pretty_entries: 0,
        };
        let (normalized, changed) = normalize_wallet_header(wallet).expect("normalize");
        assert!(!changed);
        assert!(normalized.encrypted_payload_b64.is_some());
    }

    /// Prefer `m/0/i` derived id over bogus cached ids (formerly `normalize_wallet_header_prefers_seed_path_over_cached_account_ids`).
    #[test]
    fn hdr_norm_seed_truth() {
        let seed = [7u8; 32];
        let idx = 11u32;
        let sk_bytes = derive_ed25519_private_key(&seed, &[0, idx]);
        let sk = SigningKey::from_bytes(&sk_bytes);
        let pk = sk.verifying_key().to_bytes();
        let true_id = account_id_from_parts(&pk, idx);
        let wallet = WalletReadHeader {
            schema_version: 2,
            mode: "plaintext_dev".into(),
            derivation_index: idx,
            derivation_path: Some(format!("m/0/{idx}")),
            domain_u16: u16::from_be_bytes([true_id[0], true_id[1]]),
            account_id_hex: Some("11".repeat(32)),
            account_id_human: account_id_to_human(&[2u8; 32]),
            owned_accounts: Vec::new(),
            address_book: Vec::new(),
            signing_key_hex: Some(hex::encode(sk.to_bytes())),
            master_seed_hex: Some(hex::encode(seed)),
            encrypted_payload_b64: None,
            kdf_salt_b64: None,
            aead_nonce_b64: None,
            kdf: None,
            kdf_iters: None,
            ignored_legacy_pretty_entries: 0,
        };
        let (normalized, _) = normalize_wallet_header(wallet).expect("normalize");
        assert_eq!(normalized.account_id_human, account_id_to_human(&true_id));
    }

    /// Drops duplicate legacy-pretty duplicates in AB (formerly `normalize_wallet_header_ignores_pretty_entries_in_address_book`).
    #[test]
    fn hdr_ab_dedupe_legacy() {
        let id = [
            0x2c, 0x7e, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 1,
        ];
        let canonical = account_id_to_bech32dx(&id);
        let pretty = account_id_to_human(&id);
        let wallet = WalletReadHeader {
            schema_version: 2,
            mode: "plaintext_dev".into(),
            derivation_index: 1,
            derivation_path: Some("m/0/1".into()),
            domain_u16: 0x2C7E,
            account_id_hex: Some(hex::encode(id)),
            account_id_human: account_id_to_human(&id),
            owned_accounts: Vec::new(),
            address_book: vec![
                AddressBookEntry::AddressOnly(canonical.clone()),
                AddressBookEntry::AddressOnly(pretty),
            ],
            signing_key_hex: None,
            master_seed_hex: None,
            encrypted_payload_b64: None,
            kdf_salt_b64: None,
            aead_nonce_b64: None,
            kdf: None,
            kdf_iters: None,
            ignored_legacy_pretty_entries: 0,
        };
        let (normalized, changed) = normalize_wallet_header(wallet).expect("normalize");
        assert!(changed);
        assert_eq!(normalized.address_book.len(), 1);
        assert_eq!(normalized.address_book[0].address_str(), canonical);
        assert_eq!(normalized.ignored_legacy_pretty_entries, 1);
    }

    /// Minimal schema v3 file without legacy active markers (formerly `load_wallet_read_header_supports_schema_v3_without_active_account`).
    #[test]
    fn v3_hdr_rows_ok() {
        let path = std::env::temp_dir().join(format!(
            "pwm_core_wallet_read_v3_{}.yaml",
            rand::random::<u128>()
        ));
        let raw = r#"schema_version: 3
mode: plaintext_dev
accounts:
  - derivation_path: "m/0/0"
    derivation_index: 0
    domain_u16: 11390
    flags_mask_u32: 1023
    expected_flags_u32: 1
    flags_derived_u32: 1
    id_hex: "2c7e0000000000000000000000000000000000000000000000000000000000aa"
    id_pretty: "pwm1-CY/7E-f00000000-t00000000000000000000000000000000000000000000000000aa"
  - derivation_path: "m/0/1"
    derivation_index: 1
    domain_u16: 11390
    flags_mask_u32: 1023
    expected_flags_u32: 1
    flags_derived_u32: 1
    id_hex: "2c7e000000000000000000000000000000000000000000000000000000000000"
    id_pretty: "pwm1-CY/7E-f00000000-t0000000000000000000000000000000000000000000000000000"
"#;
        std::fs::write(&path, raw).unwrap();
        let loaded = load_wallet_read_header(&path, false).expect("load");
        assert_eq!(loaded.header.schema_version, 3);
        assert_eq!(loaded.header.derivation_index, 0);
        assert_eq!(
            loaded.header.account_id_hex.as_deref(),
            Some("2c7e0000000000000000000000000000000000000000000000000000000000aa")
        );
        assert_eq!(loaded.header.owned_accounts.len(), 2);
        assert_eq!(
            loaded.header.owned_accounts[0].id_hex,
            "2c7e0000000000000000000000000000000000000000000000000000000000aa"
        );
        assert_eq!(
            loaded.header.owned_accounts[1].id_hex,
            "2c7e000000000000000000000000000000000000000000000000000000000000"
        );
        let _ = std::fs::remove_file(&path);
    }
}
