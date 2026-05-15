//! Block header (PoA sig) and tx Merkle root.

use crate::crypto::{hash_header_signing_payload, sign, verify};
use crate::tx::SignedTx;
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};

/// Signed PoA header (excl. `signature` in hash input via `signing_payload`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockHdr {
    pub height: u64,
    #[serde(with = "crate::ser_bin::hex32")]
    pub prev_hash: [u8; 32],
    pub ts: u64,
    pub prod_idx: u32,
    #[serde(with = "crate::ser_bin::hex32")]
    pub tx_root: [u8; 32],
    #[serde(with = "crate::ser_bin::hex32")]
    pub state_root: [u8; 32],
    #[serde(with = "crate::ser_bin::sig64")]
    pub sig: [u8; 64],
}

impl BlockHdr {
    /// Canonical preimage for Ed25519 block signature.
    pub fn sign_preimage(&self) -> [u8; 32] {
        let h = self.height.to_le_bytes();
        let t = self.ts.to_le_bytes();
        let p = self.prod_idx.to_le_bytes();
        hash_header_signing_payload(&[&h, &self.prev_hash, &t, &p, &self.tx_root, &self.state_root])
    }

    pub fn verify_sig(&self, prod_pk: &[u8; 32]) -> bool {
        let m = self.sign_preimage();
        verify(prod_pk, m.as_ref(), &self.sig)
    }

    pub fn sign(sk: &SigningKey, mut hdr: BlockHdr) -> BlockHdr {
        let m = hdr.sign_preimage();
        hdr.sig = sign(sk, m.as_ref());
        hdr
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Block {
    pub hdr: BlockHdr,
    pub txs: Vec<SignedTx>,
}

/// Binary Merkle over `tx_hash()` leaves (pairwise blake3).
pub fn txs_root(txs: &[SignedTx]) -> [u8; 32] {
    use blake3::Hasher;
    let mut leaves: Vec<[u8; 32]> = txs.iter().map(|t| t.tx_hash()).collect();
    if leaves.is_empty() {
        return *blake3::hash(b"PWMv0/EMPTYTX").as_bytes();
    }
    while leaves.len() > 1 {
        let mut next = Vec::new();
        for ch in leaves.chunks(2) {
            let mut h = Hasher::new();
            h.update(b"PWMv0/TXNODE");
            h.update(&ch[0]);
            h.update(ch.get(1).unwrap_or(&ch[0]));
            next.push(*h.finalize().as_bytes());
        }
        leaves = next;
    }
    leaves[0]
}

/// Full header hash for `prev_hash` link.
pub fn hdr_hash(h: &BlockHdr) -> [u8; 32] {
    *blake3::hash(&bincode::serialize(h).expect("hdr bincode")).as_bytes()
}

#[cfg(test)]
// keep-in-sync: naming script
mod hdr_hex_tests {
    use super::{hdr_hash, BlockHdr};

    fn sample_hdr() -> BlockHdr {
        BlockHdr {
            height: 42,
            prev_hash: [7u8; 32],
            ts: 99,
            prod_idx: 0,
            tx_root: [8u8; 32],
            state_root: [9u8; 32],
            sig: [0u8; 64],
        }
    }

    #[test]
    fn hdr_json_hex_str() {
        let h = sample_hdr();
        let j = serde_json::to_string(&h).expect("json");
        assert!(
            j.contains("\"prev_hash\":\"0707070707070707070707070707070707070707070707070707070707070707\""),
            "{j}"
        );
    }

    #[test]
    fn hdr_json_legacy_arr() {
        let h = sample_hdr();
        let j = serde_json::to_string(&h).expect("json");
        let back: BlockHdr = serde_json::from_str(&j).expect("de");
        assert_eq!(back, h);

        let b32 = |v: u8| (0..32).map(|_| v.to_string()).collect::<Vec<_>>().join(",");
        let b64z = (0..64).map(|_| "0").collect::<Vec<_>>().join(",");
        let legacy = format!(
            r#"{{"height":42,"prev_hash":[{}],"ts":99,"prod_idx":0,"tx_root":[{}],"state_root":[{}],"sig":[{}]}}"#,
            b32(7),
            b32(8),
            b32(9),
            b64z
        );
        let leg_de: BlockHdr = serde_json::from_str(&legacy).expect("legacy");
        assert_eq!(leg_de, h);
    }

    #[test]
    fn hdr_bincode_hash_stable() {
        let h = sample_hdr();
        let h1 = hdr_hash(&h);
        let raw = bincode::serialize(&h).expect("bincode ser");
        let h2: BlockHdr = bincode::deserialize(&raw).expect("bincode de");
        assert_eq!(hdr_hash(&h2), h1);
    }
}
