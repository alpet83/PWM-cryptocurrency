use blake3::Hasher;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

pub fn blake3_32(data: &[u8]) -> [u8; 32] {
    *blake3::hash(data).as_bytes()
}

pub fn sign(signing_key: &SigningKey, message: &[u8]) -> [u8; 64] {
    signing_key.sign(message).to_bytes()
}

pub fn verify(pubkey: &[u8; 32], message: &[u8], sig: &[u8; 64]) -> bool {
    let Ok(vk) = VerifyingKey::from_bytes(pubkey) else {
        return false;
    };
    let Ok(sig) = Signature::from_slice(sig) else {
        return false;
    };
    vk.verify(message, &sig).is_ok()
}

/// Canonical encoding for block header signing payload (without `signature` field).
pub fn hash_header_signing_payload(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Hasher::new();
    h.update(b"PWMv0/BLOCKHDR");
    for p in parts {
        h.update(p);
    }
    *h.finalize().as_bytes()
}
