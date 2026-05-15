//! Wallet account CRUD helpers backed by YAML v3 disk format.

use pwm_core::{account_id_to_human, parse_account_id};
use slip10_ed25519::derive_ed25519_private_key;
use std::path::Path;

use crate::wallet::crypto::wallet_secrets;
use crate::wallet::store::{default_v3_account, load_wallet_v3_raw, save_v3_merge};
use crate::wallet::types::{
    WalletAccountEntry, WalletAccountRemoveResult, WalletYaml, WalletYamlV3Account,
};

pub fn wallet_account_list(path: &Path) -> Result<Vec<WalletAccountEntry>, String> {
    let wallet_v3 = load_wallet_v3_raw(path)?;
    let default_id = default_v3_account(&wallet_v3.accounts)
        .expect("validated non-empty accounts")
        .id_hex
        .clone();
    Ok(wallet_v3
        .accounts
        .into_iter()
        .map(|account| WalletAccountEntry {
            derivation_index: account.derivation_index,
            derivation_path: account.derivation_path,
            id_hex: account.id_hex.clone(),
            id_pretty: account.id_pretty,
            is_active: account.id_hex.eq_ignore_ascii_case(&default_id),
        })
        .collect())
}

pub fn wallet_account_add(
    path: &Path,
    derivation_index: u32,
    passphrase: Option<&str>,
) -> Result<WalletAccountEntry, String> {
    let wallet_v3 = load_wallet_v3_raw(path)?;
    let seed_hex_owned = match wallet_v3.mode.as_str() {
        "plaintext_dev" => wallet_v3
            .master_seed_hex
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .ok_or_else(|| {
                "wallet schema v3 account add requires master_seed_hex in wallet".to_string()
            })?,
        "encrypted" => {
            let secrets = wallet_secrets(
                &WalletYaml {
                    schema_version: 3,
                    mode: wallet_v3.mode.clone(),
                    created_at_unix_sec: wallet_v3.created_at_unix_sec,
                    country_code_label: wallet_v3.country_code_label.clone(),
                    derivation_index: 0,
                    derivation_path: None,
                    domain_u16: 0,
                    flags_mask_u32: 0,
                    expected_flags_u32: 0,
                    flags_derived_u32: 0,
                    account_id_hex: String::new(),
                    account_id_human: String::new(),
                    master_seed_hex: None,
                    master_seed_b64: wallet_v3.master_seed_b64.clone(),
                    signing_key_hex: None,
                    signing_key_b64: wallet_v3.signing_key_b64.clone(),
                    verifying_key_hex: None,
                    verifying_key_b64: wallet_v3.verifying_key_b64.clone(),
                    encrypted_payload_b64: wallet_v3.encrypted_payload_b64.clone(),
                    kdf_salt_b64: wallet_v3.kdf_salt_b64.clone(),
                    aead_nonce_b64: wallet_v3.aead_nonce_b64.clone(),
                    kdf: wallet_v3.kdf.clone(),
                    kdf_iters: wallet_v3.kdf_iters,
                    address_book: Vec::new(),
                    ignored_legacy_pretty_entries: 0,
                },
                passphrase,
            )?;
            secrets.master_seed_hex
        }
        other => return Err(format!("unsupported wallet mode '{other}'")),
    };
    let seed_hex = seed_hex_owned.trim();
    let seed_vec =
        hex::decode(seed_hex).map_err(|e| format!("wallet master_seed_hex is invalid: {e}"))?;
    if seed_vec.len() != 32 {
        return Err("wallet master_seed_hex must be 32-byte hex".to_string());
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&seed_vec);
    wallet_account_add_seed(path, derivation_index, &seed)
}

pub fn wallet_account_add_seed(
    path: &Path,
    derivation_index: u32,
    seed: &[u8; 32],
) -> Result<WalletAccountEntry, String> {
    let mut wallet_v3 = load_wallet_v3_raw(path)?;
    let baseline = wallet_v3
        .accounts
        .first()
        .expect("validated non-empty accounts");
    let baseline_sk = derive_ed25519_private_key(seed, &[0, baseline.derivation_index]);
    let baseline_pk = ed25519_dalek::SigningKey::from_bytes(&baseline_sk)
        .verifying_key()
        .to_bytes();
    let baseline_expected =
        pwm_core::hd::account_id_from_parts(&baseline_pk, baseline.derivation_index);
    let baseline_actual = parse_account_id(baseline.id_hex.trim())
        .map_err(|e| format!("wallet schema v3 baseline account id_hex is invalid: {e}"))?;
    if baseline_expected != baseline_actual {
        return Err(
            "provided master seed does not match existing wallet accounts; refusing to append"
                .to_string(),
        );
    }
    let sk_bytes = derive_ed25519_private_key(seed, &[0, derivation_index]);
    let sk = ed25519_dalek::SigningKey::from_bytes(&sk_bytes);
    let pk = sk.verifying_key().to_bytes();
    let id = pwm_core::hd::account_id_from_parts(&pk, derivation_index);
    let id_hex = hex::encode(id);
    if wallet_v3
        .accounts
        .iter()
        .any(|account| account.id_hex.eq_ignore_ascii_case(&id_hex))
    {
        return Err(format!(
            "wallet schema v3 already contains account id_hex={id_hex} (duplicate)"
        ));
    }
    if wallet_v3
        .accounts
        .iter()
        .any(|account| account.derivation_index == derivation_index)
    {
        return Err(format!(
            "wallet schema v3 already contains derivation_index={derivation_index} (duplicate)"
        ));
    }
    let account = WalletYamlV3Account {
        derivation_index,
        derivation_path: format!("m/0/{derivation_index}"),
        domain_u16: u16::from_be_bytes([id[0], id[1]]),
        flags_mask_u32: baseline.flags_mask_u32,
        expected_flags_u32: baseline.expected_flags_u32,
        flags_derived_u32: u32::from_be_bytes([id[2], id[3], id[4], id[5]]),
        id_hex: id_hex.clone(),
        id_pretty: account_id_to_human(&id),
        added_at_unix_sec: Some(WalletYaml::now_unix_sec()),
    };
    wallet_v3.accounts.push(account.clone());
    save_v3_merge(path, &wallet_v3)?;
    Ok(WalletAccountEntry {
        derivation_index: account.derivation_index,
        derivation_path: account.derivation_path,
        id_hex: account.id_hex,
        id_pretty: account.id_pretty,
        is_active: false,
    })
}

pub fn wallet_account_remove(
    path: &Path,
    id_hex: &str,
) -> Result<WalletAccountRemoveResult, String> {
    let mut wallet_v3 = load_wallet_v3_raw(path)?;
    if wallet_v3.accounts.len() == 1 {
        return Err("wallet account remove refused: cannot remove last account".to_string());
    }
    let default_before = default_v3_account(&wallet_v3.accounts)
        .expect("validated non-empty accounts")
        .id_hex
        .clone();
    let wanted =
        parse_account_id(id_hex.trim()).map_err(|e| format!("invalid --id-hex account id: {e}"))?;
    let wanted_hex = hex::encode(wanted);
    let idx = wallet_v3
        .accounts
        .iter()
        .position(|a| a.id_hex.eq_ignore_ascii_case(&wanted_hex))
        .ok_or_else(|| format!("wallet schema v3 account id_hex={} not found", wanted_hex))?;
    let removed = wallet_v3.accounts.remove(idx);
    let new_active_id_hex = default_v3_account(&wallet_v3.accounts)
        .expect("account list is non-empty after guard")
        .id_hex
        .clone();
    let removed_was_active = removed.id_hex.eq_ignore_ascii_case(&default_before);
    save_v3_merge(path, &wallet_v3)?;
    Ok(WalletAccountRemoveResult {
        removed_id_hex: removed.id_hex,
        new_active_id_hex,
        removed_was_active,
    })
}

pub fn wallet_account_use(path: &Path, id_hex: &str) -> Result<(), String> {
    let wallet_v3 = load_wallet_v3_raw(path)?;
    let wanted =
        parse_account_id(id_hex.trim()).map_err(|e| format!("invalid --id-hex account id: {e}"))?;
    let wanted_hex = hex::encode(wanted);
    if !wallet_v3
        .accounts
        .iter()
        .any(|account| account.id_hex.eq_ignore_ascii_case(&wanted_hex))
    {
        return Err(format!(
            "wallet schema v3 account id_hex={} not found",
            wanted_hex
        ));
    }
    Ok(())
}
