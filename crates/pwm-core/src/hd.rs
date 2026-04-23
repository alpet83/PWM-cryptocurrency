//! SLIP-0010 Ed25519: path `m/0'/i` via [0, i] in `slip10_ed25519`.

use crate::crypto::blake3_32;
use crate::types::AccountId;
use ed25519_dalek::SigningKey;
use slip10_ed25519::derive_ed25519_private_key;

/// Raw address: BLAKE3(pk || LE_U32(i)); first two bytes (big-endian u16) must match `domain_code`.
pub fn account_id_from_parts(pubkey: &[u8; 32], derivation_index: u32) -> AccountId {
    let mut buf = [0u8; 32 + 4];
    buf[..32].copy_from_slice(pubkey);
    buf[32..].copy_from_slice(&derivation_index.to_le_bytes());
    blake3_32(&buf)
}

pub fn domain_of_account_id(id: &AccountId) -> u16 {
    u16::from_be_bytes([id[0], id[1]])
}

/// Finds the minimal `i` for `domain_code`. Returns (signing_key, verifying_key_bytes, i, account_id).
pub fn brute_cluster_address(
    master_seed: &[u8],
    domain_code: u16,
    max_tries: u32,
) -> Option<(SigningKey, [u8; 32], u32, AccountId)> {
    for i in 0..max_tries {
        let sk_bytes = derive_ed25519_private_key(master_seed, &[0, i]);
        let sk = SigningKey::from_bytes(&sk_bytes);
        let pk = sk.verifying_key().to_bytes();
        let aid = account_id_from_parts(&pk, i);
        if domain_of_account_id(&aid) == domain_code {
            return Some((sk, pk, i, aid));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use slip10_ed25519::derive_ed25519_private_key;

    #[test]
    fn brute_finds_i0_for_derived_domain() {
        let seed = [7u8; 32];
        let sk_bytes = derive_ed25519_private_key(&seed, &[0, 0]);
        let sk = SigningKey::from_bytes(&sk_bytes);
        let pk = sk.verifying_key().to_bytes();
        let aid = account_id_from_parts(&pk, 0u32);
        let d = domain_of_account_id(&aid);
        let r = brute_cluster_address(&seed, d, 10_000).expect("i=0 matches");
        assert_eq!(r.2, 0);
        assert_eq!(r.3, aid);
    }
}
