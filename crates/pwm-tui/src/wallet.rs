//! Wallet load/save, YAML helpers, encryption hooks, identity selection.

use base64::Engine;
use pwm_core::{
    load_wallet_read_header, normalize_wallet_header, open_wallet_secret_ciphertext,
    parse_account_id, seal_wallet_secret_plaintext, AccountId, WalletReadHeader,
};
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value as YamlValue};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::config::Args;
use crate::models::{BookRecipient, OwnedWalletAccount, WalletIdentity, WalletV3Meta};

#[derive(Clone)]
pub enum IdentitySource {
    Wallet(WalletIdentity),
    SeedFallback,
}

/// Shown on a dedicated layout row when no `--wallet` / `PWM_TUI_WALLET`.
pub const FALLBACK_MODE_WARNING: &str =
    "FALLBACK MODE: wallet not provided, owner derived from seed/default path";

/// Fixed vertical slot for the bordered WARNING strip (fallback). Use **`Length`**, not `Min`, so
/// ratatui does not assign most free height to this chunk (which made the banner look half-screen).
pub const FALLBACK_WARN_CHUNK_ROWS: u16 = 5;
/// Bordered detail line (`selected: …`): top + one inner row + bottom.
pub const DETAIL_CHUNK_ROWS: u16 = 3;

pub fn parse_signing_key_hex(hex_key: &str) -> Result<ed25519_dalek::SigningKey, String> {
    let bytes = hex::decode(hex_key.trim()).map_err(|e| format!("bad signing key hex: {e}"))?;
    if bytes.len() != 32 {
        return Err("wallet signing key must be 32-byte hex".into());
    }
    let mut k = [0u8; 32];
    k.copy_from_slice(&bytes);
    Ok(ed25519_dalek::SigningKey::from_bytes(&k))
}

/// Decrypt encrypted wallet blob; returns signing key + raw JSON bytes for F4 re-key cache.
pub fn try_decrypt_wallet_secret_payload(
    wallet: &WalletReadHeader,
    passphrase: Option<&str>,
) -> Result<(ed25519_dalek::SigningKey, Vec<u8>), String> {
    let passphrase = passphrase.ok_or_else(|| {
        "encrypted wallet requires passphrase: use --wallet-passphrase or PWM_TUI_WALLET_PASSPHRASE".to_string()
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
    let payload: serde_json::Value =
        serde_json::from_slice(&plaintext).map_err(|e| e.to_string())?;
    let signing_key_hex = payload
        .get("signing_key_hex")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "wallet payload is missing signing_key_hex".to_string())?;
    let sk = parse_signing_key_hex(signing_key_hex)?;
    Ok((sk, plaintext))
}

pub fn load_owned_accounts(
    path: &Path,
    wallet: &WalletReadHeader,
    active_id: &AccountId,
) -> Result<Vec<OwnedWalletAccount>, String> {
    if wallet.schema_version == 3 {
        let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let meta: WalletV3Meta = serde_yaml::from_str(&raw).map_err(|e| e.to_string())?;
        let mut accounts = Vec::with_capacity(meta.accounts.len());
        for (i, account) in meta.accounts.iter().enumerate() {
            let id = parse_account_id(account.id_hex.trim()).map_err(|e| {
                format!(
                    "wallet accounts[{i}] invalid id_hex '{}': {e}",
                    account.id_hex
                )
            })?;
            accounts.push(OwnedWalletAccount {
                id,
                domain: account.domain_u16,
                derivation_index: account.derivation_index,
                is_active: id == *active_id,
            });
        }
        if !accounts.is_empty() {
            return Ok(accounts);
        }
    }

    let mut accounts = Vec::new();
    for (i, acc) in wallet.owned_accounts.iter().enumerate() {
        let id = parse_account_id(acc.id_hex.trim()).map_err(|e| {
            format!(
                "wallet owned_accounts[{i}] invalid id_hex '{}': {e}",
                acc.id_hex
            )
        })?;
        accounts.push(OwnedWalletAccount {
            id,
            domain: wallet.domain_u16,
            derivation_index: wallet.derivation_index,
            is_active: id == *active_id,
        });
    }
    if accounts.is_empty() || !accounts.iter().any(|a| a.is_active) {
        accounts.push(OwnedWalletAccount {
            id: *active_id,
            domain: wallet.domain_u16,
            derivation_index: wallet.derivation_index,
            is_active: true,
        });
    }
    Ok(accounts)
}

pub fn load_wallet_identity(
    path: &PathBuf,
    passphrase: Option<&str>,
    unlock_secs: u64,
    upgrade_wallet: bool,
) -> Result<WalletIdentity, String> {
    let loaded = load_wallet_read_header(path, upgrade_wallet)
        .map_err(|e| format!("failed to load wallet '{}': {e}", path.display()))?;
    let wallet: WalletReadHeader = loaded.header;
    let account_id = parse_account_id(&wallet.account_id_human)
        .map_err(|e| format!("wallet account_id_human is invalid: {e}"))?;
    let pass = passphrase.filter(|s| !s.is_empty());
    let (signing_key, unlock_expires_at, wallet_is_encrypted, secret_payload_plaintext) =
        match wallet.mode.as_str() {
            "plaintext_dev" => {
                let sk =
                    parse_signing_key_hex(wallet.signing_key_hex.as_deref().ok_or_else(|| {
                        "wallet plaintext payload is missing signing_key_hex".to_string()
                    })?)?;
                (Some(sk), None, false, None)
            }
            "encrypted" => {
                if let Some(pw) = pass {
                    let (sk, pt) = try_decrypt_wallet_secret_payload(&wallet, Some(pw))?;
                    let until = Instant::now() + Duration::from_secs(unlock_secs);
                    (Some(sk), Some(until), true, Some(pt))
                } else {
                    (None, None, true, None)
                }
            }
            other => return Err(format!("unsupported wallet mode '{other}'")),
        };
    let owned_accounts = load_owned_accounts(path, &wallet, &account_id)?;
    let mut address_book = Vec::new();
    for (i, entry) in wallet.address_book.iter().enumerate() {
        let bid = entry.account_id().map_err(|e| {
            format!(
                "wallet address_book[{i}] invalid address '{}': {e}",
                entry.address_str()
            )
        })?;
        let label = entry.label().map(|s| s.to_string());
        address_book.push(BookRecipient { id: bid, label });
    }
    let encryption_prompt_hint = wallet_upgrade_encryption_hook(&wallet, loaded.upgraded_on_load);
    Ok(WalletIdentity {
        account_id,
        account_id_human: wallet.account_id_human,
        domain: wallet.domain_u16,
        derivation_index: wallet.derivation_index,
        signing_key,
        unlock_expires_at,
        wallet_is_encrypted,
        wallet_path: path.clone(),
        upgrade_wallet,
        owned_accounts,
        address_book,
        encryption_prompt_hint,
        ignored_legacy_pretty_entries: loaded.ignored_legacy_pretty_entries,
        master_seed_hex: wallet.master_seed_hex,
        secret_payload_plaintext,
    })
}

pub fn wallet_try_unlock_with_passphrase(
    w: &mut WalletIdentity,
    passphrase: &str,
    unlock_secs: u64,
) -> Result<(), String> {
    let loaded = load_wallet_read_header(&w.wallet_path, w.upgrade_wallet)
        .map_err(|e| format!("failed to load wallet: {e}"))?;
    let wallet = loaded.header;
    if wallet.mode != "encrypted" {
        return Err("wallet is not encrypted".into());
    }
    let (sk, pt) = try_decrypt_wallet_secret_payload(&wallet, Some(passphrase))?;
    w.signing_key = Some(sk);
    w.secret_payload_plaintext = Some(pt);
    w.unlock_expires_at = Some(Instant::now() + Duration::from_secs(unlock_secs));
    Ok(())
}

pub fn wallet_apply_auto_lock(identity: &mut IdentitySource) {
    let IdentitySource::Wallet(w) = identity else {
        return;
    };
    if !w.wallet_is_encrypted {
        return;
    }
    let Some(exp) = w.unlock_expires_at else {
        return;
    };
    if w.signing_key.is_none() {
        return;
    }
    if Instant::now() >= exp {
        wallet_lock_now(w);
    }
}

pub fn wallet_lock_now(w: &mut WalletIdentity) {
    if !w.wallet_is_encrypted {
        return;
    }
    w.signing_key = None;
    w.secret_payload_plaintext = None;
    w.unlock_expires_at = None;
}

pub fn validate_encrypt_passphrase_inputs(
    passphrase: &str,
    confirm: &str,
) -> Result<(), &'static str> {
    if passphrase.is_empty() {
        Err("passphrase must not be empty")
    } else if passphrase != confirm {
        Err("passphrases do not match")
    } else {
        Ok(())
    }
}

pub fn identity_f3_action_label(identity: &IdentitySource) -> &'static str {
    match identity {
        IdentitySource::Wallet(w) if w.wallet_is_encrypted && w.signing_key.is_some() => "lock",
        _ => "unlock",
    }
}

pub fn identity_lock_status_suffix(identity: &IdentitySource) -> String {
    let IdentitySource::Wallet(w) = identity else {
        return String::new();
    };
    if !w.wallet_is_encrypted {
        return String::new();
    }
    match (&w.signing_key, w.unlock_expires_at) {
        (None, _) => " | wallet: LOCKED".into(),
        (Some(_), Some(exp)) => {
            let left = exp.saturating_duration_since(Instant::now()).as_secs();
            format!(" | wallet: unlocked (~{left}s)")
        }
        (Some(_), None) => " | wallet: unlocked".into(),
    }
}

pub fn wallet_upgrade_encryption_hook(
    wallet: &WalletReadHeader,
    upgraded_on_load: bool,
) -> Option<String> {
    if upgraded_on_load && wallet.mode == "plaintext_dev" {
        return Some(
            "wallet format auto-upgraded (plaintext): press F4 to encrypt this wallet file".into(),
        );
    }
    None
}

pub fn yaml_root_map(root: &mut YamlValue) -> Result<&mut Mapping, String> {
    root.as_mapping_mut()
        .ok_or_else(|| "wallet YAML root must be a mapping".to_string())
}

pub fn yaml_map_get_string(map: &Mapping, key: &str) -> Option<String> {
    map.get(&YamlValue::String(key.to_string()))
        .and_then(|v| v.as_str().map(|s| s.to_string()))
}

pub fn merge_normalized_wallet_header(
    root: &mut YamlValue,
    header: &WalletReadHeader,
) -> Result<(), String> {
    let map = yaml_root_map(root)?;
    map.insert(
        YamlValue::String("account_id_human".into()),
        YamlValue::String(header.account_id_human.clone()),
    );
    let hex_line = if let Some(h) = header
        .account_id_hex
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        h.to_string()
    } else {
        let id = parse_account_id(header.account_id_human.trim())
            .map_err(|e| format!("wallet account_id_human invalid: {e}"))?;
        hex::encode(id)
    };
    map.insert(
        YamlValue::String("account_id_hex".into()),
        YamlValue::String(hex_line),
    );
    let book = serde_yaml::to_value(&header.address_book).map_err(|e| e.to_string())?;
    map.insert(YamlValue::String("address_book".into()), book);
    Ok(())
}

#[derive(Deserialize, Serialize)]
pub struct WalletSecretJson {
    pub master_seed_hex: String,
    pub master_seed_b64: String,
    pub signing_key_hex: String,
    pub signing_key_b64: String,
    pub verifying_key_hex: String,
    pub verifying_key_b64: String,
}

pub fn build_plaintext_secret_json(map: &Mapping) -> Result<Vec<u8>, String> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let master_seed_hex = yaml_map_get_string(map, "master_seed_hex").ok_or_else(|| {
        "wallet plaintext is missing master_seed_hex (cannot encrypt)".to_string()
    })?;
    let signing_key_hex = yaml_map_get_string(map, "signing_key_hex").ok_or_else(|| {
        "wallet plaintext is missing signing_key_hex (cannot encrypt)".to_string()
    })?;
    let sk = parse_signing_key_hex(&signing_key_hex)?;
    let verifying_key_hex = if let Some(v) = yaml_map_get_string(map, "verifying_key_hex") {
        v
    } else {
        hex::encode(sk.verifying_key().to_bytes())
    };
    let seed_vec = hex::decode(master_seed_hex.trim())
        .map_err(|e| format!("wallet master_seed_hex invalid: {e}"))?;
    if seed_vec.len() != 32 {
        return Err("wallet master_seed_hex must be 32 bytes".into());
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&seed_vec);
    let master_seed_b64 =
        yaml_map_get_string(map, "master_seed_b64").unwrap_or_else(|| b64.encode(seed));
    let signing_key_b64 =
        yaml_map_get_string(map, "signing_key_b64").unwrap_or_else(|| b64.encode(sk.to_bytes()));
    let vk_bytes = hex::decode(verifying_key_hex.trim())
        .map_err(|e| format!("wallet verifying_key_hex invalid: {e}"))?;
    if vk_bytes.len() != 32 {
        return Err("wallet verifying_key_hex must be 32 bytes".into());
    }
    let verifying_key_b64 =
        yaml_map_get_string(map, "verifying_key_b64").unwrap_or_else(|| b64.encode(vk_bytes));
    let payload = WalletSecretJson {
        master_seed_hex,
        master_seed_b64,
        signing_key_hex,
        signing_key_b64,
        verifying_key_hex,
        verifying_key_b64,
    };
    serde_json::to_vec(&payload).map_err(|e| e.to_string())
}

pub fn replace_wallet_file(tmp: &Path, dest: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        std::fs::rename(tmp, dest).map_err(|e| e.to_string())?;
    }
    #[cfg(windows)]
    {
        if dest.exists() {
            std::fs::remove_file(dest).map_err(|e| e.to_string())?;
        }
        std::fs::rename(tmp, dest).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Atomic replace: write temp in the same directory, `fsync`, then rename (Windows: delete dest first).
pub fn write_wallet_yaml_atomic(path: &Path, contents: &str) -> Result<(), String> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let _ = path
        .file_name()
        .ok_or_else(|| "wallet path has no file name".to_string())?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let tmp_path = dir.join(format!(
        "pwm_wallet_tmp_{}_{}.yml",
        std::process::id(),
        nanos
    ));
    {
        let mut f = std::fs::File::create(&tmp_path).map_err(|e| e.to_string())?;
        f.write_all(contents.as_bytes())
            .map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
    }
    match replace_wallet_file(&tmp_path, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp_path);
            Err(e)
        }
    }
}

/// Encrypt `plaintext_dev` or re-seal an `encrypted` wallet with `rekey_payload` (decrypted JSON bytes).
pub fn wallet_encrypt_or_rekey_disk(
    path: &Path,
    new_passphrase: &str,
    rekey_payload: Option<&[u8]>,
) -> Result<(), String> {
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut root: YamlValue = serde_yaml::from_str(&raw).map_err(|e| e.to_string())?;
    let header: WalletReadHeader = serde_yaml::from_str(&raw).map_err(|e| e.to_string())?;
    let (normalized, _) = normalize_wallet_header(header)?;
    merge_normalized_wallet_header(&mut root, &normalized)?;
    let map = yaml_root_map(&mut root)?;
    let mode = yaml_map_get_string(map, "mode").unwrap_or_default();
    let secret_json: Vec<u8> = match mode.as_str() {
        "plaintext_dev" => {
            if rekey_payload.is_some() {
                return Err("internal: rekey payload set for plaintext wallet".into());
            }
            build_plaintext_secret_json(map)?
        }
        "encrypted" => rekey_payload.map(|p| p.to_vec()).ok_or_else(|| {
            "encrypted wallet: unlock (F3) or start with PWM_TUI_WALLET_PASSPHRASE before re-key"
                .to_string()
        })?,
        other => return Err(format!("unsupported wallet mode '{other}'")),
    };
    let sealed = seal_wallet_secret_plaintext(&secret_json, new_passphrase)?;
    let map = yaml_root_map(&mut root)?;
    if mode == "plaintext_dev" {
        for k in [
            "master_seed_hex",
            "master_seed_b64",
            "signing_key_hex",
            "signing_key_b64",
            "verifying_key_hex",
            "verifying_key_b64",
        ] {
            map.remove(&YamlValue::String(k.into()));
        }
    }
    map.insert(
        YamlValue::String("schema_version".into()),
        serde_yaml::to_value(2u32).map_err(|e| e.to_string())?,
    );
    map.insert(
        YamlValue::String("mode".into()),
        YamlValue::String("encrypted".into()),
    );
    map.insert(
        YamlValue::String("encrypted_payload_b64".into()),
        YamlValue::String(sealed.encrypted_payload_b64),
    );
    map.insert(
        YamlValue::String("kdf_salt_b64".into()),
        YamlValue::String(sealed.kdf_salt_b64),
    );
    map.insert(
        YamlValue::String("aead_nonce_b64".into()),
        YamlValue::String(sealed.aead_nonce_b64),
    );
    map.insert(
        YamlValue::String("kdf".into()),
        YamlValue::String(sealed.kdf),
    );
    map.insert(
        YamlValue::String("kdf_iters".into()),
        serde_yaml::to_value(sealed.kdf_iters).map_err(|e| e.to_string())?,
    );
    let out = serde_yaml::to_string(&root).map_err(|e| e.to_string())?;
    write_wallet_yaml_atomic(path, &out)
}

pub fn choose_identity(args: &Args, unlock_secs: u64) -> Result<(IdentitySource, String), String> {
    if let Some(wallet) = &args.wallet {
        let identity = load_wallet_identity(
            wallet,
            args.wallet_passphrase.as_deref(),
            unlock_secs,
            args.upgrade_wallet,
        )?;
        let mut note = format!(
            "wallet owner: {} (m/0/{}, d={})",
            identity.account_id_human, identity.derivation_index, identity.domain
        );
        if identity.wallet_is_encrypted && identity.signing_key.is_none() {
            note.push_str(" | encrypted: signing key locked — press F3 to unlock");
        }
        let note = if let Some(hook) = identity.encryption_prompt_hint.as_deref() {
            format!("{note} | {hook}")
        } else {
            note
        };
        let note = if identity.ignored_legacy_pretty_entries > 0 {
            format!(
                "{note} | ignored legacy pretty address_book entries={}",
                identity.ignored_legacy_pretty_entries
            )
        } else {
            note
        };
        Ok((IdentitySource::Wallet(identity), note))
    } else {
        // Banner text is `FALLBACK_MODE_WARNING` in the main layout (not duplicated in footer).
        Ok((IdentitySource::SeedFallback, String::new()))
    }
}

/// If `./default.yml` exists under the process current directory, use it as `--wallet`.
pub fn default_wallet_if_present() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let p = cwd.join("default.yml");
    p.is_file().then_some(p)
}

/// Used by unit tests (`tests` module in `main.rs`); not referenced by production `run()` path.
#[allow(dead_code)]
pub fn default_wallet_candidate(base: &std::path::Path) -> Option<PathBuf> {
    let p = base.join("default.yml");
    p.is_file().then_some(p)
}
