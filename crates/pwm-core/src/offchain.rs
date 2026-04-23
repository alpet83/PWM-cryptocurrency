//! Offchain batch Merkle + provider Ed25519 sig (stub, no on-chain bridge).

use crate::crypto::sign;
use ed25519_dalek::SigningKey;

/// Payload signed by provider: domain || batch_id || merkle_root.
pub fn batch_preimage(batch_id: u64, root: &[u8; 32]) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(b"PWMv0-OFFBATCH");
    v.extend_from_slice(&batch_id.to_le_bytes());
    v.extend_from_slice(root);
    v
}

/// Pairwise blake3 Merkle over 32-byte leaves.
pub fn merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    use blake3::Hasher;
    if leaves.is_empty() {
        return *blake3::hash(b"PWMv0/OFFEMPTY").as_bytes();
    }
    let mut lv: Vec<[u8; 32]> = leaves.to_vec();
    while lv.len() > 1 {
        let mut next = Vec::new();
        for ch in lv.chunks(2) {
            let mut h = Hasher::new();
            h.update(b"PWMv0/OFFNODE");
            h.update(&ch[0]);
            h.update(ch.get(1).unwrap_or(&ch[0]));
            next.push(*h.finalize().as_bytes());
        }
        lv = next;
    }
    lv[0]
}

pub fn sign_batch(sk: &SigningKey, batch_id: u64, root: [u8; 32]) -> [u8; 64] {
    let m = batch_preimage(batch_id, &root);
    sign(sk, &m)
}
