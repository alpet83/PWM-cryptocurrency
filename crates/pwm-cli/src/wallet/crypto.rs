//! Wallet encryption/decryption bridging YAML modes to secrets structs.

use pwm_core::{open_wallet_secret_ciphertext, seal_wallet_secret_plaintext};

use crate::wallet::types::{WalletProtection, WalletSecretPayload, WalletSecrets, WalletYaml};

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

pub(crate) fn apply_protection(
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
