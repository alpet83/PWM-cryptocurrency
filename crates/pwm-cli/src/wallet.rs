use base64::Engine;
use pwm_core::{
    account_id_to_bech32dx, account_id_to_human, address_book_contains,
    append_wallet_yaml_address_book, open_wallet_secret_ciphertext, parse_account_id,
    parse_account_id_for_migration, parse_account_id_for_user_input, seal_wallet_secret_plaintext,
    AccountId, AddressBookEntry,
};
use serde::{Deserialize, Serialize};
use slip10_ed25519::derive_ed25519_private_key;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalletYaml {
    pub schema_version: u32,
    pub mode: String,
    pub created_at_unix_sec: u64,
    #[serde(default)]
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

impl WalletYaml {
    pub fn now_unix_sec() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before epoch")
            .as_secs()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct WalletSecretPayload {
    master_seed_hex: String,
    master_seed_b64: String,
    signing_key_hex: String,
    signing_key_b64: String,
    verifying_key_hex: String,
    verifying_key_b64: String,
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
    to_wallet_yaml_with_metadata(
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

pub fn to_wallet_yaml_with_metadata(
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
    let serialized = serde_yaml::to_string(wallet).map_err(|e| e.to_string())?;
    fs::write(path, serialized).map_err(|e| e.to_string())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn parse_wallet_yaml(s: &str) -> Result<WalletYaml, String> {
    serde_yaml::from_str(s).map_err(|e| e.to_string())
}

pub fn load_wallet_yaml(path: &Path) -> Result<WalletYaml, String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut wallet = parse_wallet_yaml(&raw)?;
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
    Ok(wallet)
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
    if let Some(id) = account_id_from_truth_source(wallet)? {
        return Ok(id);
    }
    if !wallet.account_id_hex.trim().is_empty() {
        return parse_account_id(wallet.account_id_hex.trim())
            .map_err(|e| format!("wallet account_id_hex is invalid: {e}"));
    }
    parse_account_id_for_migration(wallet.account_id_human.trim())
        .map_err(|e| format!("wallet account_id_human migration error: {e}"))
}

fn account_id_from_truth_source(wallet: &WalletYaml) -> Result<Option<AccountId>, String> {
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
        let id = parse_account_id_for_migration(entry.address_str().trim())
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

pub fn wallet_address_book_contains(wallet: &WalletYaml, to: &AccountId) -> bool {
    address_book_contains(&wallet.address_book, to)
}

/// When `address_book` is non-empty, `to` must match a registered entry (after parse).
pub fn assert_tx_recipient_in_wallet_address_book(
    wallet: &WalletYaml,
    to: &AccountId,
) -> Result<(), String> {
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
    append_wallet_yaml_address_book(path, address_str, label).map_err(|e| {
        if e.contains("already in address_book") {
            e
        } else {
            format!("wallet book-add: {e}")
        }
    })
}

pub fn wallet_address_book_remove(path: &Path, address_str: &str) -> Result<(), String> {
    let mut w = load_wallet_yaml(path)?;
    let id = parse_account_id_for_user_input(address_str.trim())
        .map_err(|e| format!("invalid --address: {e}"))?;
    let before = w.address_book.len();
    w.address_book
        .retain(|e| e.account_id().ok().as_ref() != Some(&id));
    if w.address_book.len() == before {
        return Err("address not found in address_book".into());
    }
    save_wallet_yaml(path, &w)
}

pub fn wallet_secrets(
    wallet: &WalletYaml,
    passphrase: Option<&str>,
) -> Result<WalletSecrets, String> {
    match wallet.mode.as_str() {
        "plaintext_dev" => {
            let master_seed_hex = wallet
                .master_seed_hex
                .clone()
                .ok_or_else(|| "wallet plaintext payload is missing master_seed_hex".to_string())?;
            let signing_key_hex = wallet
                .signing_key_hex
                .clone()
                .ok_or_else(|| "wallet plaintext payload is missing signing_key_hex".to_string())?;
            let verifying_key_hex = wallet.verifying_key_hex.clone().ok_or_else(|| {
                "wallet plaintext payload is missing verifying_key_hex".to_string()
            })?;
            Ok(WalletSecrets {
                master_seed_hex,
                signing_key_hex,
                verifying_key_hex,
            })
        }
        "encrypted" => decrypt_wallet(wallet, passphrase),
        other => Err(format!("unsupported wallet mode '{other}'")),
    }
}

fn apply_protection(
    wallet: &mut WalletYaml,
    payload: WalletSecretPayload,
    protection: WalletProtection,
) -> Result<(), String> {
    match protection {
        WalletProtection::PlaintextDev => {
            wallet.schema_version = 1;
            wallet.mode = "plaintext_dev".to_string();
            wallet.master_seed_hex = Some(payload.master_seed_hex);
            wallet.master_seed_b64 = Some(payload.master_seed_b64);
            wallet.signing_key_hex = Some(payload.signing_key_hex);
            wallet.signing_key_b64 = Some(payload.signing_key_b64);
            wallet.verifying_key_hex = Some(payload.verifying_key_hex);
            wallet.verifying_key_b64 = Some(payload.verifying_key_b64);
            Ok(())
        }
        WalletProtection::Encrypted { passphrase } => {
            let plaintext = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
            let sealed = seal_wallet_secret_plaintext(&plaintext, passphrase.as_str())?;
            wallet.encrypted_payload_b64 = Some(sealed.encrypted_payload_b64);
            wallet.kdf_salt_b64 = Some(sealed.kdf_salt_b64);
            wallet.aead_nonce_b64 = Some(sealed.aead_nonce_b64);
            wallet.kdf = Some(sealed.kdf);
            wallet.kdf_iters = Some(sealed.kdf_iters);
            Ok(())
        }
    }
}

fn decrypt_wallet(wallet: &WalletYaml, passphrase: Option<&str>) -> Result<WalletSecrets, String> {
    let passphrase = passphrase.ok_or_else(|| {
        "encrypted wallet requires passphrase: set PWM_WALLET_PASSPHRASE or pass --wallet-passphrase".to_string()
    })?;
    let kdf = wallet
        .kdf
        .as_deref()
        .ok_or_else(|| "encrypted wallet is missing kdf".to_string())?;
    let iters = wallet
        .kdf_iters
        .ok_or_else(|| "encrypted wallet is missing kdf_iters".to_string())?;
    let enc = wallet
        .encrypted_payload_b64
        .as_deref()
        .ok_or_else(|| "encrypted wallet is missing encrypted_payload_b64".to_string())?;
    let salt_b64 = wallet
        .kdf_salt_b64
        .as_deref()
        .ok_or_else(|| "encrypted wallet is missing kdf_salt_b64".to_string())?;
    let nonce_b64 = wallet
        .aead_nonce_b64
        .as_deref()
        .ok_or_else(|| "encrypted wallet is missing aead_nonce_b64".to_string())?;
    let plaintext =
        open_wallet_secret_ciphertext(enc, salt_b64, nonce_b64, kdf, iters, passphrase)?;
    let payload: WalletSecretPayload =
        serde_json::from_slice(&plaintext).map_err(|e| e.to_string())?;
    Ok(WalletSecrets {
        master_seed_hex: payload.master_seed_hex,
        signing_key_hex: payload.signing_key_hex,
        verifying_key_hex: payload.verifying_key_hex,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pwm_core::{
        account_id_to_human, append_wallet_yaml_address_book, parse_account_id, AddressBookEntry,
    };
    use std::path::Path;

    /// Two distinct accounts on a recognized regulatory domain (`brute_cluster_address` like `wallet init`).
    fn two_policy_valid_wallets() -> ((String, String), (String, String)) {
        use pwm_core::hd::brute_cluster_address;
        const MAX: u32 = 500_000;
        let (_, _, _, owner) = brute_cluster_address(&[101u8; 32], 0x2C00, MAX)
            .expect("brute owner for CY high-byte domain");
        let (_, _, _, peer) = brute_cluster_address(&[102u8; 32], 0x2C00, MAX)
            .expect("brute peer for CY high-byte domain");
        assert_ne!(owner, peer);
        (
            (hex::encode(owner), account_id_to_human(&owner)),
            (hex::encode(peer), account_id_to_human(&peer)),
        )
    }

    fn encrypted_wallet_fixture(seed: [u8; 32], passphrase: &str) -> WalletYaml {
        let (sk, pk, idx, id) =
            pwm_core::hd::brute_cluster_address(&seed, 0x2C00, 500_000).expect("fixture hit");
        to_wallet_yaml_with_metadata(
            seed,
            sk.to_bytes(),
            pk,
            idx,
            0x2C00,
            0x03FF,
            0,
            0,
            hex::encode(id),
            account_id_to_human(&id),
            Some("CY".to_string()),
            WalletProtection::Encrypted {
                passphrase: passphrase.to_string(),
            },
        )
        .expect("fixture wallet")
    }

    /// Load wallet from disk and apply the same recipient check as `tx-send --wallet` (no `--master`).
    fn check_tx_send_recipient_book(wallet_path: &Path, to_str: &str) -> Result<(), String> {
        let doc = load_wallet_yaml(wallet_path)?;
        let to = parse_account_id(to_str.trim()).map_err(|e| e.to_string())?;
        assert_tx_recipient_in_wallet_address_book(&doc, &to)
    }

    /// Mirrors `crate::main` `Cmd::TxSend`: `address_book` is enforced only when `master` is `None`
    /// and a `--wallet` path is present.
    fn tx_send_address_book_gate(
        wallet_path: Option<&Path>,
        master: Option<&str>,
        to_str: &str,
    ) -> Result<(), String> {
        let to = parse_account_id(to_str.trim()).map_err(|e| e.to_string())?;
        if master.is_none() {
            if let Some(wp) = wallet_path {
                let doc = load_wallet_yaml(wp)?;
                assert_tx_recipient_in_wallet_address_book(&doc, &to)?;
            }
        }
        Ok(())
    }

    #[test]
    fn wallet_yaml_roundtrip() {
        let y = to_wallet_yaml_with_metadata(
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
            7,
            0x007E,
            0x00FF_00FF,
            0x0000_00FF,
            0xAAFF_00FF,
            "11".repeat(32),
            "pwm1test".to_string(),
            Some("CY".to_string()),
            WalletProtection::PlaintextDev,
        )
        .unwrap();
        let text = serde_yaml::to_string(&y).unwrap();
        let parsed = parse_wallet_yaml(&text).unwrap();
        assert_eq!(parsed.schema_version, 1);
        assert_eq!(parsed.mode, "plaintext_dev");
        assert_eq!(parsed.country_code_label.as_deref(), Some("CY"));
        assert_eq!(parsed.derivation_index, 7);
        assert_eq!(parsed.derivation_path.as_deref(), Some("m/0/7"));
        assert_eq!(parsed.domain_u16, 0x007E);
        assert_eq!(parsed.flags_mask_u32, 0x00FF_00FF);
        assert_eq!(parsed.expected_flags_u32, 0x0000_00FF);
        assert_eq!(parsed.flags_derived_u32, 0xAAFF_00FF);
        assert_eq!(parsed.master_seed_hex, Some(hex::encode([1u8; 32])));
        assert_eq!(
            parsed.master_seed_b64,
            Some(base64::engine::general_purpose::STANDARD.encode([1u8; 32]))
        );
    }

    #[test]
    fn load_wallet_yaml_normalizes_legacy_pretty() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_loader_norm_{}.yaml",
            rand::random::<u128>()
        ));
        let raw = r#"schema_version: 1
mode: plaintext_dev
created_at_unix_sec: 1
derivation_index: 1
domain_u16: 11264
flags_mask_u32: 0
expected_flags_u32: 0
flags_derived_u32: 0
account_id_hex: "2c00000000000000000000000000000000000000000000000000000000000000"
account_id_human: pwm1-CY-f00000000-t0000000000000000000000000000000000000000000000000000
"#;
        std::fs::write(&path, raw).unwrap();
        let loaded = load_wallet_yaml(&path).expect("load");
        assert!(loaded.account_id_human.contains("CY/00"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_wallet_yaml_uses_truth_source_when_cached_ids_mismatch() {
        let seed = [9u8; 32];
        let idx = 5u32;
        let sk_bytes = derive_ed25519_private_key(&seed, &[0, idx]);
        let sk = ed25519_dalek::SigningKey::from_bytes(&sk_bytes);
        let pk = sk.verifying_key().to_bytes();
        let true_id = pwm_core::hd::account_id_from_parts(&pk, idx);
        let mut wallet = to_wallet_yaml_with_metadata(
            seed,
            sk.to_bytes(),
            pk,
            idx,
            u16::from_be_bytes([true_id[0], true_id[1]]),
            0x03FF,
            0,
            0,
            "aa".repeat(32),
            account_id_to_human(&[1u8; 32]),
            Some("CY".to_string()),
            WalletProtection::PlaintextDev,
        )
        .unwrap();
        wallet.account_id_hex = "ff".repeat(32);
        wallet.account_id_human = account_id_to_human(&[2u8; 32]);
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_truth_source_{}.yaml",
            rand::random::<u128>()
        ));
        save_wallet_yaml(&path, &wallet).unwrap();
        let loaded = load_wallet_yaml(&path).expect("must load from truth source");
        assert_eq!(loaded.account_id_hex, hex::encode(true_id));
        assert_eq!(loaded.account_id_human, account_id_to_human(&true_id));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_wallet_yaml_ignores_legacy_pretty_address_book_entry() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_loader_book_{}.yaml",
            rand::random::<u128>()
        ));
        let raw = r#"schema_version: 1
mode: plaintext_dev
created_at_unix_sec: 1
derivation_index: 1
domain_u16: 11264
flags_mask_u32: 0
expected_flags_u32: 0
flags_derived_u32: 0
account_id_hex: "2c00000000000000000000000000000000000000000000000000000000000000"
account_id_human: pwm1-CY/00-f00000000-t0000000000000000000000000000000000000000000000000000
address_book:
  - pwm1-CY-f00000000-t0000000000000000000000000000000000000000000000000000
"#;
        std::fs::write(&path, raw).unwrap();
        let loaded = load_wallet_yaml(&path).expect("must load");
        assert_eq!(loaded.address_book.len(), 0);
        assert_eq!(loaded.ignored_legacy_pretty_entries, 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn encrypted_wallet_roundtrip_decrypts() {
        let wallet = to_wallet_yaml_with_metadata(
            [7u8; 32],
            [8u8; 32],
            [9u8; 32],
            12,
            0x4359,
            0x03FF,
            0,
            0,
            "aa".repeat(32),
            "pwm1demo".to_string(),
            Some("CY".to_string()),
            WalletProtection::Encrypted {
                passphrase: "secret".to_string(),
            },
        )
        .unwrap();
        assert_eq!(wallet.mode, "encrypted");
        assert_eq!(wallet.schema_version, 2);
        assert!(wallet.master_seed_hex.is_none());
        let secrets = wallet_secrets(&wallet, Some("secret")).unwrap();
        assert_eq!(secrets.master_seed_hex, hex::encode([7u8; 32]));
    }

    #[test]
    fn encrypted_wallet_rejects_wrong_passphrase() {
        let wallet = to_wallet_yaml_with_metadata(
            [4u8; 32],
            [5u8; 32],
            [6u8; 32],
            1,
            0x4359,
            0x03FF,
            0,
            0,
            "bb".repeat(32),
            "pwm1demo".to_string(),
            Some("CY".to_string()),
            WalletProtection::Encrypted {
                passphrase: "secret".to_string(),
            },
        )
        .unwrap();
        let err = wallet_secrets(&wallet, Some("wrong")).expect_err("must fail");
        assert!(err.contains("failed to decrypt wallet payload"));
    }

    #[test]
    fn address_book_allow_list_skips_when_empty() {
        let mut w = to_wallet_yaml_with_metadata(
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
            0,
            0x4359,
            0x03FF,
            0,
            0,
            "aa".repeat(32),
            "pwm1demo".to_string(),
            Some("CY".to_string()),
            WalletProtection::PlaintextDev,
        )
        .unwrap();
        w.address_book.clear();
        let to = parse_account_id(w.account_id_hex.as_str()).unwrap();
        assert_tx_recipient_in_wallet_address_book(&w, &to).unwrap();
    }

    #[test]
    fn address_book_allow_list_enforces_when_non_empty() {
        let mut w = to_wallet_yaml_with_metadata(
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
            0,
            0x4359,
            0x03FF,
            0,
            0,
            "aa".repeat(32),
            "pwm1demo".to_string(),
            Some("CY".to_string()),
            WalletProtection::PlaintextDev,
        )
        .unwrap();
        let owner = parse_account_id(w.account_id_hex.as_str()).unwrap();
        w.address_book = vec![AddressBookEntry::AddressOnly(account_id_to_human(&owner))];
        let other = [9u8; 32];
        assert!(assert_tx_recipient_in_wallet_address_book(&w, &other).is_err());
    }

    #[test]
    fn tx_send_recipient_book_tempfile_rejects_unknown_then_allows_after_append() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_addrbook_{}.yaml",
            rand::random::<u128>()
        ));
        let (_owner_pair, (_peer_hex, peer_human)) = two_policy_valid_wallets();
        let seed = [1u8; 32];
        let (owner_sk, owner_pk, owner_idx, owner_id) =
            pwm_core::hd::brute_cluster_address(&seed, 0x2C00, 500_000).expect("owner hit");
        let owner_hex = hex::encode(owner_id);
        let owner_human = account_id_to_human(&owner_id);
        let w = to_wallet_yaml_with_metadata(
            seed,
            owner_sk.to_bytes(),
            owner_pk,
            owner_idx,
            0x2C00,
            0x03FF,
            0,
            0,
            owner_hex,
            owner_human.clone(),
            Some("CY".to_string()),
            WalletProtection::PlaintextDev,
        )
        .unwrap();
        save_wallet_yaml(&path, &w).unwrap();

        let owner = parse_account_id(w.account_id_hex.as_str()).unwrap();
        let owner_human = account_id_to_human(&owner);
        append_wallet_yaml_address_book(&path, &owner_human, None).unwrap();

        assert!(check_tx_send_recipient_book(&path, &peer_human).is_err());

        append_wallet_yaml_address_book(&path, &peer_human, None).unwrap();
        check_tx_send_recipient_book(&path, &peer_human).unwrap();

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn tx_send_address_book_skipped_when_master_some_matches_cli_tx_send_gate() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_master_bypass_{}.yaml",
            rand::random::<u128>()
        ));
        let ((owner_hex, owner_human), (_outsider_hex, outsider_human)) =
            two_policy_valid_wallets();
        let w = to_wallet_yaml_with_metadata(
            [3u8; 32],
            [4u8; 32],
            [5u8; 32],
            0,
            0x2C00,
            0x03FF,
            0,
            0,
            owner_hex,
            owner_human.clone(),
            Some("CY".to_string()),
            WalletProtection::PlaintextDev,
        )
        .unwrap();
        save_wallet_yaml(&path, &w).unwrap();
        let owner = parse_account_id(w.account_id_hex.as_str()).unwrap();
        append_wallet_yaml_address_book(&path, &account_id_to_human(&owner), None).unwrap();

        let err = tx_send_address_book_gate(Some(path.as_path()), None, &outsider_human)
            .expect_err("book");
        assert!(!err.is_empty());

        // `Cmd::TxSend` uses `if master.is_none() { ... }` — any `Some(_)` skips the allow-list read.
        tx_send_address_book_gate(Some(path.as_path()), Some("deadbeef"), &outsider_human).unwrap();

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn wallet_address_book_add_duplicate_returns_error_on_file() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_dup_{}.yaml",
            rand::random::<u128>()
        ));
        let ((dup_hex, dup_human), _) = two_policy_valid_wallets();
        let w = to_wallet_yaml_with_metadata(
            [6u8; 32],
            [7u8; 32],
            [8u8; 32],
            0,
            0x2C00,
            0x03FF,
            0,
            0,
            dup_hex,
            dup_human.clone(),
            Some("CY".to_string()),
            WalletProtection::PlaintextDev,
        )
        .unwrap();
        save_wallet_yaml(&path, &w).unwrap();
        let id = parse_account_id(w.account_id_hex.as_str()).unwrap();
        let human = account_id_to_human(&id);
        assert_eq!(human, dup_human);
        wallet_address_book_add(&path, &human, None).unwrap();
        let err = wallet_address_book_add(&path, &human, None).expect_err("duplicate");
        assert!(err.contains("already in address_book"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn wallet_address_book_remove_rejects_ambiguous_legacy_pretty_input() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_remove_ambiguous_{}.yaml",
            rand::random::<u128>()
        ));
        let seed = [9u8; 32];
        let (owner_sk, owner_pk, owner_idx, owner_id) =
            pwm_core::hd::brute_cluster_address(&seed, 0x2C00, 500_000).expect("owner hit");
        let owner_hex = hex::encode(owner_id);
        let owner_human = account_id_to_human(&owner_id);
        let w = to_wallet_yaml_with_metadata(
            seed,
            owner_sk.to_bytes(),
            owner_pk,
            owner_idx,
            0x2C00,
            0x03FF,
            0,
            0,
            owner_hex,
            owner_human.clone(),
            Some("CY".to_string()),
            WalletProtection::PlaintextDev,
        )
        .unwrap();
        save_wallet_yaml(&path, &w).unwrap();
        let ambiguous = "pwm1-CY-f00000000-t0000000000000000000000000000000000000000000000000000";
        let err = wallet_address_book_remove(&path, ambiguous).expect_err("must reject");
        assert!(err.contains("missing '/LO'"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn backup_wallet_file_rejects_wrong_passphrase_for_encrypted() {
        let source = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_backup_source_{}.yaml",
            rand::random::<u128>()
        ));
        let out = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_backup_out_{}.yaml",
            rand::random::<u128>()
        ));
        let wallet = encrypted_wallet_fixture([1u8; 32], "secret-good");
        save_wallet_yaml(&source, &wallet).unwrap();
        let err = backup_wallet_file(&source, &out, Some("secret-bad")).expect_err("must fail");
        assert!(err.contains("wallet encrypted payload validation failed"));
        assert!(err.contains("correct --wallet-passphrase"));
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn backup_wallet_file_rejects_corrupted_encrypted_payload() {
        let source = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_backup_corrupt_{}.yaml",
            rand::random::<u128>()
        ));
        let out = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_backup_corrupt_out_{}.yaml",
            rand::random::<u128>()
        ));
        let mut wallet = encrypted_wallet_fixture([4u8; 32], "secret-good");
        wallet.encrypted_payload_b64 = Some("%%%not-base64%%%".to_string());
        save_wallet_yaml(&source, &wallet).unwrap();
        let err = backup_wallet_file(&source, &out, Some("secret-good")).expect_err("must fail");
        assert!(err.contains("wallet encrypted payload validation failed"));
        assert!(err.contains("corrupted"));
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    fn recover_wallet_file_creates_verified_copy() {
        let backup = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_recover_backup_{}.yaml",
            rand::random::<u128>()
        ));
        let restored = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_recover_out_{}.yaml",
            rand::random::<u128>()
        ));
        let wallet = encrypted_wallet_fixture([7u8; 32], "secret-good");
        save_wallet_yaml(&backup, &wallet).unwrap();
        recover_wallet_file(&backup, &restored, Some("secret-good")).expect("recover");
        let restored_wallet = load_wallet_yaml(&restored).expect("load restored");
        let secrets = wallet_secrets(&restored_wallet, Some("secret-good")).expect("unlock");
        assert_eq!(secrets.master_seed_hex, hex::encode([7u8; 32]));
        let _ = std::fs::remove_file(&backup);
        let _ = std::fs::remove_file(&restored);
    }
}
