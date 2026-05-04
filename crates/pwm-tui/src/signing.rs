//! Signing key derivation and sender material for transactions.

use pwm_core::hd::{account_id_from_parts, brute_cluster_address, domain_of_account_id};
use pwm_core::AccountId;

use crate::models::WalletIdentity;
use crate::wallet::{IdentitySource, WalletSecretJson};

pub fn derive_sender_for_from(
    from: &AccountId,
    master_seed_hex: &str,
) -> Result<(ed25519_dalek::SigningKey, u16, u32), String> {
    let seed_raw =
        hex::decode(master_seed_hex.trim()).map_err(|e| format!("bad master seed: {e}"))?;
    if seed_raw.len() != 32 {
        return Err("master seed must be 32-byte hex (64 chars)".into());
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&seed_raw);
    let dom = domain_of_account_id(from);
    let brute_max = std::env::var("PWM_TUI_BRUTE_MAX")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(500_000);
    let hit = brute_cluster_address(&seed, dom, brute_max)
        .ok_or_else(|| format!("cannot derive sender for domain in first {brute_max} tries"))?;
    if &hit.3 != from {
        return Err("from address does not match derived account from PWM_TUI_MASTER_SEED".into());
    }
    Ok((hit.0, dom, hit.2))
}

fn parse_seed_hex(seed_hex: &str) -> Result<[u8; 32], String> {
    let seed_raw = hex::decode(seed_hex.trim()).map_err(|e| format!("bad master seed: {e}"))?;
    if seed_raw.len() != 32 {
        return Err("master seed must be 32-byte hex (64 chars)".into());
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&seed_raw);
    Ok(seed)
}

pub fn wallet_seed_opt(w: &WalletIdentity) -> Result<Option<[u8; 32]>, String> {
    if let Some(seed_hex) = w
        .master_seed_hex
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return parse_seed_hex(seed_hex).map(Some);
    }
    let Some(payload) = w.secret_payload_plaintext.as_deref() else {
        return Ok(None);
    };
    let secret: WalletSecretJson = serde_json::from_slice(payload).map_err(|e| {
        format!("selected owner cannot be signed: decrypted wallet payload is invalid: {e}")
    })?;
    parse_seed_hex(&secret.master_seed_hex).map(Some)
}

pub fn wallet_seed(w: &WalletIdentity) -> Result<[u8; 32], String> {
    wallet_seed_opt(w)?.ok_or_else(|| {
        "selected owner cannot be signed: wallet has no unlocked master seed".to_string()
    })
}

pub fn derive_wallet_key(
    seed: &[u8; 32],
    index: u32,
    domain: u16,
    expected: &AccountId,
) -> Result<ed25519_dalek::SigningKey, String> {
    let key = slip10_ed25519::derive_ed25519_private_key(seed, &[0, index]);
    let sk = ed25519_dalek::SigningKey::from_bytes(&key);
    verify_wallet_key(sk, domain, index, expected)
}

pub fn verify_wallet_key(
    sk: ed25519_dalek::SigningKey,
    domain: u16,
    index: u32,
    expected: &AccountId,
) -> Result<ed25519_dalek::SigningKey, String> {
    let actual_domain = domain_of_account_id(expected);
    if domain != actual_domain {
        return Err(format!(
            "selected owner cannot be signed: wallet metadata domain {domain:#06X} does not match selected account domain {actual_domain:#06X}"
        ));
    }
    let actual = account_id_from_parts(&sk.verifying_key().to_bytes(), index);
    if &actual != expected {
        return Err(format!(
            "selected owner cannot be signed: signing key for m/0/{index} does not match selected account"
        ));
    }
    Ok(sk)
}

pub fn signing_material_for_sender(
    from: &AccountId,
    identity: &IdentitySource,
) -> Result<(ed25519_dalek::SigningKey, u16, u32), String> {
    match identity {
        IdentitySource::Wallet(w) => {
            let account = w.owned_accounts.iter().find(|a| a.id == *from);
            let (domain, index) = match account {
                Some(account) => (account.domain, account.derivation_index),
                None if w.owned_accounts.is_empty() && &w.account_id == from => {
                    (w.domain, w.derivation_index)
                }
                None => return Err("selected owner account is not in wallet".into()),
            };
            if let Some(seed) = wallet_seed_opt(w)? {
                let sk = derive_wallet_key(&seed, index, domain, from)?;
                return Ok((sk, domain, index));
            }
            let sk = w.signing_key.as_ref().ok_or_else(|| {
                "wallet is locked: press F3 to unlock (signing key not held in memory)".to_string()
            })?;
            if &w.account_id == from {
                let sk = verify_wallet_key(sk.clone(), domain, index, from)?;
                return Ok((sk, domain, index));
            }
            let seed = wallet_seed(w)?;
            let sk = derive_wallet_key(&seed, index, domain, from)?;
            Ok((sk, domain, index))
        }
        IdentitySource::SeedFallback => {
            let seed_hex = std::env::var("PWM_TUI_MASTER_SEED")
                .map_err(|_| "set PWM_TUI_MASTER_SEED (32-byte hex) for signing".to_string())?;
            derive_sender_for_from(from, &seed_hex)
        }
    }
}
