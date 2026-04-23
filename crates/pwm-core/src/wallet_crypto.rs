//! Shared ChaCha20-Poly1305 + PBKDF2-HMAC-SHA256 sealing for wallet secret JSON payloads.
//! Used by `pwm-cli` and `pwm-tui` so encryption parameters stay aligned.

use base64::Engine;
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce};
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use sha2::Sha256;

/// YAML `kdf` value for wallets created by PWM tools.
pub const WALLET_KDF: &str = "pbkdf2_sha256";
/// PBKDF2 iteration count (must match historical wallets).
pub const WALLET_KDF_ITERS: u32 = 100_000;

/// Fields written into wallet YAML for an encrypted secret blob.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletSealedPayload {
    pub encrypted_payload_b64: String,
    pub kdf_salt_b64: String,
    pub aead_nonce_b64: String,
    pub kdf: String,
    pub kdf_iters: u32,
}

fn derive_key(passphrase: &[u8], salt: &[u8], iters: u32) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(passphrase, salt, iters, &mut key);
    key
}

/// Encrypt `plaintext` (typically JSON for `WalletSecretPayload`) with a user passphrase.
pub fn seal_wallet_secret_plaintext(
    plaintext: &[u8],
    passphrase: &str,
) -> Result<WalletSealedPayload, String> {
    if passphrase.trim().is_empty() {
        return Err("wallet passphrase must not be empty".into());
    }
    let mut salt = [0u8; 16];
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut salt);
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let key = derive_key(passphrase.as_bytes(), &salt, WALLET_KDF_ITERS);
    let cipher = ChaCha20Poly1305::new((&key).into());
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext)
        .map_err(|_| "wallet encryption failed".to_string())?;
    let b64 = base64::engine::general_purpose::STANDARD;
    Ok(WalletSealedPayload {
        encrypted_payload_b64: b64.encode(ciphertext),
        kdf_salt_b64: b64.encode(salt),
        aead_nonce_b64: b64.encode(nonce_bytes),
        kdf: WALLET_KDF.to_string(),
        kdf_iters: WALLET_KDF_ITERS,
    })
}

/// Decrypt a wallet secret blob; `plaintext` is typically JSON bytes.
pub fn open_wallet_secret_ciphertext(
    encrypted_payload_b64: &str,
    kdf_salt_b64: &str,
    aead_nonce_b64: &str,
    kdf: &str,
    kdf_iters: u32,
    passphrase: &str,
) -> Result<Vec<u8>, String> {
    if kdf != WALLET_KDF {
        return Err(format!("unsupported wallet kdf '{kdf}'"));
    }
    let b64 = base64::engine::general_purpose::STANDARD;
    let salt = b64
        .decode(kdf_salt_b64)
        .map_err(|e| format!("kdf_salt_b64: {e}"))?;
    let nonce = b64
        .decode(aead_nonce_b64)
        .map_err(|e| format!("aead_nonce_b64: {e}"))?;
    if nonce.len() != 12 {
        return Err("encrypted wallet nonce must be 12 bytes".into());
    }
    let ciphertext = b64
        .decode(encrypted_payload_b64)
        .map_err(|e| format!("encrypted_payload_b64: {e}"))?;
    let key = derive_key(passphrase.as_bytes(), &salt, kdf_iters);
    let cipher = ChaCha20Poly1305::new((&key).into());
    cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| {
            "failed to decrypt wallet payload (invalid passphrase or corrupted file)".to_string()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_open_roundtrip() {
        let pt = br#"{"signing_key_hex":"aa"}"#;
        let sealed = seal_wallet_secret_plaintext(pt, "pw1").unwrap();
        let out = open_wallet_secret_ciphertext(
            &sealed.encrypted_payload_b64,
            &sealed.kdf_salt_b64,
            &sealed.aead_nonce_b64,
            &sealed.kdf,
            sealed.kdf_iters,
            "pw1",
        )
        .unwrap();
        assert_eq!(out, pt);
    }

    #[test]
    fn open_rejects_wrong_passphrase() {
        let sealed = seal_wallet_secret_plaintext(b"secret", "good").unwrap();
        let err = open_wallet_secret_ciphertext(
            &sealed.encrypted_payload_b64,
            &sealed.kdf_salt_b64,
            &sealed.aead_nonce_b64,
            &sealed.kdf,
            sealed.kdf_iters,
            "wrong",
        )
        .expect_err("must fail");
        assert!(err.contains("failed to decrypt"));
    }

    #[test]
    fn open_rejects_corrupted_payload() {
        let sealed = seal_wallet_secret_plaintext(b"secret", "good").unwrap();
        let err = open_wallet_secret_ciphertext(
            "%%%not-base64%%%",
            &sealed.kdf_salt_b64,
            &sealed.aead_nonce_b64,
            &sealed.kdf,
            sealed.kdf_iters,
            "good",
        )
        .expect_err("must fail");
        assert!(err.contains("encrypted_payload_b64"));
    }
}
