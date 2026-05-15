//! Serialize/load wallet YAML including migrations and bruteforce hooks.

use base64::Engine;
use pwm_core::{
    account_id_to_bech32dx, account_id_to_human, parse_account_id, parse_acct_id_mig, AccountId,
    AddressBookEntry,
};
use serde::Deserialize;
use slip10_ed25519::derive_ed25519_private_key;
use std::fs;
use std::path::Path;

use crate::bruteforce::{domain_matches, DomainMatchMode};
use crate::wallet::crypto::{apply_protection, wallet_secrets};
use crate::wallet::types::{
    WalletProtection, WalletSecretPayload, WalletYaml, WalletYamlV3, WalletYamlV3Account,
    LEGACY_ACTIVE_ACCOUNT_KEY,
};

#[allow(dead_code)]
pub fn to_wallet_yaml(
    master_seed: [u8; 32],
    signing_key: [u8; 32],
    verifying_key: [u8; 32],
    derivation_index: u32,
    domain_u16: u16,
    flags_mask_u32: u32,
    expected_flags_u32: u32,
    flags_derived_u32: u32,
    account_id_hex: String,
    account_id_human: String,
) -> Result<WalletYaml, String> {
    build_wallet_yaml(
        master_seed,
        signing_key,
        verifying_key,
        derivation_index,
        domain_u16,
        flags_mask_u32,
        expected_flags_u32,
        flags_derived_u32,
        account_id_hex,
        account_id_human,
        None,
        WalletProtection::Encrypted {
            passphrase: std::env::var("PWM_WALLET_PASSPHRASE").map_err(|_| {
                "encrypted wallet mode requires passphrase: set PWM_WALLET_PASSPHRASE or use --wallet-passphrase".to_string()
            })?,
        },
    )
}

/// Builds a wallet document with metadata and applies protection mode.
pub fn build_wallet_yaml(
    master_seed: [u8; 32],
    signing_key: [u8; 32],
    verifying_key: [u8; 32],
    derivation_index: u32,
    domain_u16: u16,
    flags_mask_u32: u32,
    expected_flags_u32: u32,
    flags_derived_u32: u32,
    account_id_hex: String,
    account_id_human: String,
    country_code_label: Option<String>,
    protection: WalletProtection,
) -> Result<WalletYaml, String> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let payload = WalletSecretPayload {
        master_seed_hex: hex::encode(master_seed),
        master_seed_b64: b64.encode(master_seed),
        signing_key_hex: hex::encode(signing_key),
        signing_key_b64: b64.encode(signing_key),
        verifying_key_hex: hex::encode(verifying_key),
        verifying_key_b64: b64.encode(verifying_key),
    };
    let mut wallet = WalletYaml {
        schema_version: 2,
        mode: "encrypted".to_string(),
        created_at_unix_sec: WalletYaml::now_unix_sec(),
        country_code_label,
        derivation_index,
        derivation_path: Some(format!("m/0/{}", derivation_index)),
        domain_u16,
        flags_mask_u32,
        expected_flags_u32,
        flags_derived_u32,
        account_id_hex,
        account_id_human,
        master_seed_hex: None,
        master_seed_b64: None,
        signing_key_hex: None,
        signing_key_b64: None,
        verifying_key_hex: None,
        verifying_key_b64: None,
        encrypted_payload_b64: None,
        kdf_salt_b64: None,
        aead_nonce_b64: None,
        kdf: None,
        kdf_iters: None,
        address_book: Vec::new(),
        ignored_legacy_pretty_entries: 0,
    };
    apply_protection(&mut wallet, payload, protection)?;
    Ok(wallet)
}

pub fn save_wallet_yaml(path: &Path, wallet: &WalletYaml) -> Result<(), String> {
    ensure_wallet_parent_dir(path)?;
    let serialized = serde_yaml::to_string(wallet).map_err(|e| e.to_string())?;
    fs::write(path, serialized).map_err(|e| e.to_string())
}

pub fn save_wallet_v3_new(path: &Path, wallet: &WalletYaml) -> Result<(), String> {
    ensure_wallet_parent_dir(path)?;
    let wallet_v3 = migrate_wallet_v2v3(wallet)?;
    let serialized = ser_v3_clean(&wallet_v3)?;
    fs::write(path, serialized).map_err(|e| e.to_string())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn parse_wallet_yaml(s: &str) -> Result<WalletYaml, String> {
    serde_yaml::from_str(s).map_err(|e| e.to_string())
}

pub fn load_wallet_yaml(path: &Path) -> Result<WalletYaml, String> {
    load_wallet_yaml_upgrade(path, false)
}

/// Loads wallet YAML and optionally persists v2->v3 migration.
pub fn load_wallet_yaml_upgrade(path: &Path, upgrade_wallet: bool) -> Result<WalletYaml, String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let schema_version = detect_schema_version(&raw)?;
    let mut wallet = if schema_version == 3 {
        parse_wallet_yaml_v3(&raw)?
    } else {
        parse_wallet_yaml(&raw)?
    };
    let account_id = resolve_wallet_account_id(&wallet)?;
    let account_domain = u16::from_be_bytes([account_id[0], account_id[1]]);
    if account_domain != wallet.domain_u16 {
        return Err(format!(
            "wallet domain_u16/account_id_hex mismatch: domain_u16={} account_id_domain={}",
            wallet.domain_u16, account_domain
        ));
    }
    wallet.account_id_hex = hex::encode(account_id);
    wallet.account_id_human = account_id_to_human(&account_id);
    wallet.ignored_legacy_pretty_entries =
        normalize_address_book_entries(&mut wallet.address_book)?;
    if schema_version == 2 {
        let migrated = migrate_wallet_v2v3(&wallet)?;
        if upgrade_wallet {
            save_wallet_v3(path, &migrated)?;
            let migrated_raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
            wallet = parse_wallet_yaml_v3(&migrated_raw)?;
        } else {
            let migrated_raw = serde_yaml::to_string(&migrated).map_err(|e| e.to_string())?;
            wallet = parse_wallet_yaml_v3(&migrated_raw)?;
        }
    }
    Ok(wallet)
}

/// Detects next derivation index for resume by scanning wallet accounts in target domain scope.
pub fn detect_resume_der_index(
    path: &Path,
    upgrade_wallet: bool,
    target_domain: u16,
    domain_mode: DomainMatchMode,
) -> Result<u32, String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let schema_version = detect_schema_version(&raw)?;
    if schema_version == 3 {
        let wallet_v3 = load_wallet_v3_raw(path)?;
        let scoped_max = wallet_v3
            .accounts
            .iter()
            .filter(|a| domain_matches(a.domain_u16, target_domain, domain_mode))
            .map(|a| a.derivation_index)
            .max();
        Ok(scoped_max.map_or(0, |idx| idx.saturating_add(1)))
    } else {
        let wallet = load_wallet_yaml_upgrade(path, upgrade_wallet)?;
        Ok(wallet.derivation_index.saturating_add(1))
    }
}

/// Migrates a v2 wallet YAML structure to the v3 format in-place.
fn migrate_wallet_v2v3(wallet: &WalletYaml) -> Result<WalletYamlV3, String> {
    let derivation_path = wallet
        .derivation_path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("m/0/{}", wallet.derivation_index));
    let active_account_id_hex = wallet.account_id_hex.trim().to_ascii_lowercase();
    let active_account_id =
        parse_account_id(active_account_id_hex.as_str()).map_err(|e| format!("{e}"))?;
    let account = WalletYamlV3Account {
        derivation_index: wallet.derivation_index,
        derivation_path,
        domain_u16: wallet.domain_u16,
        flags_mask_u32: wallet.flags_mask_u32,
        expected_flags_u32: wallet.expected_flags_u32,
        flags_derived_u32: wallet.flags_derived_u32,
        id_hex: active_account_id_hex.clone(),
        id_pretty: account_id_to_human(&active_account_id),
        added_at_unix_sec: Some(wallet.created_at_unix_sec),
    };
    Ok(WalletYamlV3 {
        schema_version: 3,
        mode: wallet.mode.clone(),
        created_at_unix_sec: wallet.created_at_unix_sec,
        country_code_label: wallet.country_code_label.clone(),
        active_account_id_hex: None,
        accounts: vec![account],
        master_seed_hex: wallet.master_seed_hex.clone(),
        master_seed_b64: wallet.master_seed_b64.clone(),
        signing_key_hex: wallet.signing_key_hex.clone(),
        signing_key_b64: wallet.signing_key_b64.clone(),
        verifying_key_hex: wallet.verifying_key_hex.clone(),
        verifying_key_b64: wallet.verifying_key_b64.clone(),
        encrypted_payload_b64: wallet.encrypted_payload_b64.clone(),
        kdf_salt_b64: wallet.kdf_salt_b64.clone(),
        aead_nonce_b64: wallet.aead_nonce_b64.clone(),
        kdf: wallet.kdf.clone(),
        kdf_iters: wallet.kdf_iters,
        address_book: wallet.address_book.clone(),
    })
}

/// Loads a v3 wallet YAML file without validation (raw deserialization).
pub(crate) fn load_wallet_v3_raw(path: &Path) -> Result<WalletYamlV3, String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    if detect_schema_version(&raw)? != 3 {
        return Err("wallet account commands require schema v3 wallet file".to_string());
    }
    let wallet_v3: WalletYamlV3 = serde_yaml::from_str(&raw).map_err(|e| e.to_string())?;
    validate_v3_derivation_paths(&wallet_v3)?;
    if wallet_v3.accounts.is_empty() {
        return Err("wallet schema v3 requires non-empty accounts".to_string());
    }
    Ok(wallet_v3)
}

/// Saves wallet state as v3 YAML with strict field validation.
fn save_wallet_v3(path: &Path, wallet_v3: &WalletYamlV3) -> Result<(), String> {
    ensure_wallet_parent_dir(path)?;
    let serialized = ser_v3_clean(wallet_v3)?;
    write_atomic(path, &serialized)
}

pub(crate) fn save_v3_merge(path: &Path, wallet_v3: &WalletYamlV3) -> Result<(), String> {
    ensure_wallet_parent_dir(path)?;
    let current_raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let current_val: serde_yaml::Value =
        serde_yaml::from_str(&current_raw).map_err(|e| e.to_string())?;
    let next_val = v3_clean_value(wallet_v3)?;
    let mut merged = merge_yaml_value(&current_val, &next_val);
    remove_legacy_active_account(&mut merged);
    let serialized = serde_yaml::to_string(&merged).map_err(|e| e.to_string())?;
    write_atomic(path, &serialized)
}

fn ser_v3_clean(wallet_v3: &WalletYamlV3) -> Result<String, String> {
    let val = v3_clean_value(wallet_v3)?;
    serde_yaml::to_string(&val).map_err(|e| e.to_string())
}

fn v3_clean_value(wallet_v3: &WalletYamlV3) -> Result<serde_yaml::Value, String> {
    let mut val = serde_yaml::to_value(wallet_v3).map_err(|e| e.to_string())?;
    remove_legacy_active_account(&mut val);
    Ok(val)
}

fn remove_legacy_active_account(val: &mut serde_yaml::Value) {
    if let serde_yaml::Value::Mapping(map) = val {
        map.remove(&serde_yaml::Value::String(
            LEGACY_ACTIVE_ACCOUNT_KEY.to_string(),
        ));
    }
}

fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    let mut tmp = path.to_path_buf();
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "wallet path has invalid file name".to_string())?;
    let unique = format!("{file_name}.tmp.{}", std::process::id());
    tmp.set_file_name(unique);
    fs::write(&tmp, content).map_err(|e| e.to_string())?;
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(_) => {
            if path.exists() {
                fs::remove_file(path).map_err(|e| e.to_string())?;
            }
            fs::rename(&tmp, path).map_err(|e| e.to_string())
        }
    }
}

fn ensure_wallet_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn merge_yaml_value(existing: &serde_yaml::Value, next: &serde_yaml::Value) -> serde_yaml::Value {
    match (existing, next) {
        (serde_yaml::Value::Mapping(old_map), serde_yaml::Value::Mapping(new_map)) => {
            let mut merged = old_map.clone();
            for (k, v_new) in new_map {
                let merged_v = if let Some(v_old) = merged.get(k) {
                    merge_yaml_value(v_old, v_new)
                } else {
                    v_new.clone()
                };
                merged.insert(k.clone(), merged_v);
            }
            serde_yaml::Value::Mapping(merged)
        }
        (serde_yaml::Value::Sequence(old_seq), serde_yaml::Value::Sequence(new_seq)) => {
            let mut merged = Vec::with_capacity(new_seq.len());
            for (idx, item_new) in new_seq.iter().enumerate() {
                if let Some(item_old) = old_seq.get(idx) {
                    merged.push(merge_yaml_value(item_old, item_new));
                } else {
                    merged.push(item_new.clone());
                }
            }
            serde_yaml::Value::Sequence(merged)
        }
        _ => next.clone(),
    }
}

pub(crate) fn detect_schema_version(raw: &str) -> Result<u32, String> {
    #[derive(Deserialize)]
    struct VersionOnly {
        schema_version: Option<u32>,
    }
    let parsed: VersionOnly = serde_yaml::from_str(raw).map_err(|e| e.to_string())?;
    Ok(parsed.schema_version.unwrap_or(2))
}

/// Parses a BIP32 m/0 derivation path from a DER index string.
fn parse_der_m0_path(path: &str) -> Result<u32, String> {
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

fn validate_v3_derivation_paths(wallet_v3: &WalletYamlV3) -> Result<(), String> {
    for account in &wallet_v3.accounts {
        let path_idx = parse_der_m0_path(&account.derivation_path)?;
        if path_idx != account.derivation_index {
            return Err(format!(
                "wallet schema v3 derivation_path {:?} does not match derivation_index {}",
                account.derivation_path, account.derivation_index
            ));
        }
    }
    Ok(())
}

pub(crate) fn default_v3_account(accounts: &[WalletYamlV3Account]) -> Option<&WalletYamlV3Account> {
    accounts
        .iter()
        .min_by_key(|a| (a.derivation_index, a.id_hex.to_ascii_lowercase()))
}

fn parse_wallet_yaml_v3(raw: &str) -> Result<WalletYaml, String> {
    let wallet_v3: WalletYamlV3 = serde_yaml::from_str(raw).map_err(|e| e.to_string())?;
    validate_v3_derivation_paths(&wallet_v3)?;
    if wallet_v3.accounts.is_empty() {
        return Err("wallet schema v3 requires non-empty accounts".to_string());
    }
    let default_account =
        default_v3_account(&wallet_v3.accounts).expect("validated non-empty accounts");

    if wallet_v3.mode == "plaintext_dev" {
        validate_v3_master(&wallet_v3)?;
    }

    Ok(WalletYaml {
        schema_version: 3,
        mode: wallet_v3.mode,
        created_at_unix_sec: wallet_v3.created_at_unix_sec,
        country_code_label: wallet_v3.country_code_label,
        derivation_index: default_account.derivation_index,
        derivation_path: Some(default_account.derivation_path.clone()),
        domain_u16: default_account.domain_u16,
        flags_mask_u32: default_account.flags_mask_u32,
        expected_flags_u32: default_account.expected_flags_u32,
        flags_derived_u32: default_account.flags_derived_u32,
        account_id_hex: default_account.id_hex.clone(),
        account_id_human: default_account.id_pretty.clone(),
        master_seed_hex: wallet_v3.master_seed_hex,
        master_seed_b64: wallet_v3.master_seed_b64,
        signing_key_hex: wallet_v3.signing_key_hex,
        signing_key_b64: wallet_v3.signing_key_b64,
        verifying_key_hex: wallet_v3.verifying_key_hex,
        verifying_key_b64: wallet_v3.verifying_key_b64,
        encrypted_payload_b64: wallet_v3.encrypted_payload_b64,
        kdf_salt_b64: wallet_v3.kdf_salt_b64,
        aead_nonce_b64: wallet_v3.aead_nonce_b64,
        kdf: wallet_v3.kdf,
        kdf_iters: wallet_v3.kdf_iters,
        address_book: wallet_v3.address_book,
        ignored_legacy_pretty_entries: 0,
    })
}

/// Validates all v3 wallet accounts against the master key derivation.
fn validate_v3_master(wallet_v3: &WalletYamlV3) -> Result<(), String> {
    let seed_hex = match wallet_v3
        .master_seed_hex
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(seed_hex) => seed_hex,
        None => return Ok(()),
    };
    let seed_vec =
        hex::decode(seed_hex).map_err(|e| format!("wallet master_seed_hex is invalid: {e}"))?;
    if seed_vec.len() != 32 {
        return Err("wallet master_seed_hex must be 32-byte hex".into());
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&seed_vec);
    for account in &wallet_v3.accounts {
        let sk_bytes = derive_ed25519_private_key(&seed, &[0, account.derivation_index]);
        let sk = ed25519_dalek::SigningKey::from_bytes(&sk_bytes);
        let pk = sk.verifying_key().to_bytes();
        let expected = pwm_core::hd::account_id_from_parts(&pk, account.derivation_index);
        let actual = parse_account_id(account.id_hex.trim())
            .map_err(|e| format!("wallet schema v3 account id_hex is invalid: {e}"))?;
        if expected != actual {
            return Err(format!(
                "wallet schema v3 account id_hex mismatch at derivation_index={}: expected {}",
                account.derivation_index,
                hex::encode(expected)
            ));
        }
    }
    Ok(())
}

fn validate_wallet_recovery_payload(
    wallet: &WalletYaml,
    passphrase: Option<&str>,
) -> Result<(), String> {
    match wallet.mode.as_str() {
        "encrypted" => {
            wallet_secrets(wallet, passphrase).map(|_| ()).map_err(|e| {
                format!(
                    "wallet encrypted payload validation failed: {e}. Pass correct --wallet-passphrase and ensure backup file is not corrupted"
                )
            })
        }
        "plaintext_dev" => wallet_secrets(wallet, None).map(|_| ()).map_err(|e| {
            format!("wallet plaintext payload validation failed: {e}")
        }),
        other => Err(format!("unsupported wallet mode '{other}'")),
    }
}

pub fn backup_wallet_file(
    wallet_path: &Path,
    backup_out: &Path,
    passphrase: Option<&str>,
) -> Result<(), String> {
    if wallet_path == backup_out {
        return Err("backup destination must differ from source wallet path".to_string());
    }
    let wallet = load_wallet_yaml(wallet_path)?;
    validate_wallet_recovery_payload(&wallet, passphrase)?;
    fs::copy(wallet_path, backup_out).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn recover_wallet_file(
    backup_path: &Path,
    wallet_out: &Path,
    passphrase: Option<&str>,
) -> Result<(), String> {
    if backup_path == wallet_out {
        return Err("recovery destination must differ from backup path".to_string());
    }
    let wallet = load_wallet_yaml(backup_path)?;
    validate_wallet_recovery_payload(&wallet, passphrase)?;
    fs::copy(backup_path, wallet_out).map_err(|e| e.to_string())?;
    Ok(())
}

fn resolve_wallet_account_id(wallet: &WalletYaml) -> Result<AccountId, String> {
    if let Some(id) = acct_id_from_source(wallet)? {
        return Ok(id);
    }
    if !wallet.account_id_hex.trim().is_empty() {
        return parse_account_id(wallet.account_id_hex.trim())
            .map_err(|e| format!("wallet account_id_hex is invalid: {e}"));
    }
    parse_acct_id_mig(wallet.account_id_human.trim())
        .map_err(|e| format!("wallet account_id_human migration error: {e}"))
}

/// Resolves account id from the authoritative source (wallet or RPC).
fn acct_id_from_source(wallet: &WalletYaml) -> Result<Option<AccountId>, String> {
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
        let sk = ed25519_dalek::SigningKey::from_bytes(&sk_bytes);
        let pk = sk.verifying_key().to_bytes();
        return Ok(Some(pwm_core::hd::account_id_from_parts(&pk, index)));
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
        let sk = ed25519_dalek::SigningKey::from_bytes(&key);
        let pk = sk.verifying_key().to_bytes();
        return Ok(Some(pwm_core::hd::account_id_from_parts(&pk, index)));
    }
    Ok(None)
}

fn parse_derivation_index(wallet: &WalletYaml) -> Result<u32, String> {
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
        return path[prefix.len()..].parse::<u32>().map_err(|_| {
            format!("wallet derivation_path is invalid: expected numeric index, got '{path}'")
        });
    }
    Ok(wallet.derivation_index)
}

fn normalize_address_book_entries(entries: &mut Vec<AddressBookEntry>) -> Result<usize, String> {
    let mut ignored = 0usize;
    let mut out = Vec::with_capacity(entries.len());
    for (idx, entry) in entries.iter().enumerate() {
        if is_legacy_pretty_address(entry.address_str()) {
            ignored += 1;
            continue;
        }
        let id = parse_acct_id_mig(entry.address_str().trim())
            .map_err(|e| format!("wallet address_book[{idx}] migration error: {e}"))?;
        let canonical = account_id_to_bech32dx(&id);
        match entry {
            AddressBookEntry::AddressOnly(_) => out.push(AddressBookEntry::AddressOnly(canonical)),
            AddressBookEntry::WithLabel { label, .. } => out.push(AddressBookEntry::WithLabel {
                address: canonical,
                label: label.clone(),
            }),
        }
    }
    *entries = out;
    Ok(ignored)
}

fn is_legacy_pretty_address(address: &str) -> bool {
    address.trim().starts_with("pwm1-")
}
