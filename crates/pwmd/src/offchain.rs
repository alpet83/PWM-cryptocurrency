//! Process-local offchain batch Merkle storage for `/v1/offchain/*`.

use pwm_core::{parse_account_id, AccountId};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

const EMPTY_TAG: &[u8] = b"PWMv1/OFFEMPTY";
const LEAF_TAG: &[u8] = b"PWMv1/OFFLEAF";
const NODE_TAG: &[u8] = b"PWMv1/OFFNODE";
const ANCHOR_TAG: &[u8] = b"PWMv1/OFFANCHOR";
const MAX_BATCHES: usize = 1024;

#[derive(Clone, Debug)]
pub(crate) struct BatchEntry {
    pub(crate) account_id: AccountId,
    pub(crate) amount: u128,
    pub(crate) nonce: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct BatchRecord {
    pub(crate) batch_id: u64,
    pub(crate) root: [u8; 32],
    pub(crate) anchor_tx_hash: [u8; 32],
    pub(crate) entries: Vec<BatchEntry>,
    leaves: Vec<[u8; 32]>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ProofStep {
    pub(crate) position: &'static str,
    pub(crate) hash: String,
}

#[derive(Default)]
pub(crate) struct OffchainStore {
    next_id: AtomicU64,
    batches: Mutex<HashMap<u64, BatchRecord>>,
}

impl OffchainStore {
    pub(crate) fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            batches: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn insert(
        &self,
        entries: Vec<BatchEntry>,
        tip_hash: [u8; 32],
        tip_height: u64,
    ) -> BatchRecord {
        let batch_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let leaves: Vec<[u8; 32]> = entries.iter().map(entry_leaf).collect();
        let root = merkle_root(&leaves);
        let anchor_tx_hash = anchor_hash(batch_id, root, tip_hash, tip_height);
        let record = BatchRecord {
            batch_id,
            root,
            anchor_tx_hash,
            entries,
            leaves,
        };
        let mut batches = self.batches.lock().expect("offchain store");
        if batches.len() >= MAX_BATCHES {
            if let Some(oldest) = batches.keys().min().copied() {
                batches.remove(&oldest);
            }
        }
        batches.insert(batch_id, record.clone());
        record
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.batches.lock().expect("offchain store").len()
    }

    pub(crate) fn get(&self, batch_id: u64) -> Option<BatchRecord> {
        self.batches
            .lock()
            .expect("offchain store")
            .get(&batch_id)
            .cloned()
    }
}

pub(crate) fn parse_entry(
    account_id: &str,
    amount: &str,
    nonce: u64,
) -> Result<BatchEntry, String> {
    let account_id = parse_account_id(account_id)?;
    let amount = amount.parse::<u128>().map_err(|e| format!("amount: {e}"))?;
    Ok(BatchEntry {
        account_id,
        amount,
        nonce,
    })
}

pub(crate) fn entry_leaf(entry: &BatchEntry) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(LEAF_TAG);
    h.update(entry.account_id);
    h.update(entry.amount.to_be_bytes());
    h.update(entry.nonce.to_be_bytes());
    h.finalize().into()
}

pub(crate) fn merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    if leaves.is_empty() {
        return hash_tag(EMPTY_TAG);
    }
    let mut level = leaves.to_vec();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            let right = pair.get(1).unwrap_or(&pair[0]);
            next.push(node_hash(pair[0], *right));
        }
        level = next;
    }
    level[0]
}

pub(crate) fn merkle_proof(record: &BatchRecord, index: usize) -> Option<Vec<ProofStep>> {
    if index >= record.leaves.len() {
        return None;
    }
    let mut idx = index;
    let mut level = record.leaves.clone();
    let mut proof = Vec::new();
    while level.len() > 1 {
        let is_right = idx % 2 == 1;
        let sib_idx = if is_right { idx - 1 } else { idx + 1 };
        let sibling = level.get(sib_idx).copied().unwrap_or(level[idx]);
        proof.push(ProofStep {
            position: if is_right { "left" } else { "right" },
            hash: hex::encode(sibling),
        });
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            let right = pair.get(1).unwrap_or(&pair[0]);
            next.push(node_hash(pair[0], *right));
        }
        idx /= 2;
        level = next;
    }
    Some(proof)
}

pub(crate) fn verify_proof(leaf: [u8; 32], proof: &[ProofStep], root: [u8; 32]) -> bool {
    let mut cur = leaf;
    for step in proof {
        let Ok(raw) = hex::decode(&step.hash) else {
            return false;
        };
        let Ok(sibling) = <[u8; 32]>::try_from(raw.as_slice()) else {
            return false;
        };
        cur = if step.position == "left" {
            node_hash(sibling, cur)
        } else {
            node_hash(cur, sibling)
        };
    }
    cur == root
}

fn anchor_hash(batch_id: u64, root: [u8; 32], tip_hash: [u8; 32], tip_height: u64) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(ANCHOR_TAG);
    h.update(batch_id.to_be_bytes());
    h.update(root);
    h.update(tip_hash);
    h.update(tip_height.to_be_bytes());
    h.finalize().into()
}

fn node_hash(left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(NODE_TAG);
    h.update(left);
    h.update(right);
    h.finalize().into()
}

fn hash_tag(tag: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(tag);
    h.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(seed: u8, amount: u128, nonce: u64) -> BatchEntry {
        BatchEntry {
            account_id: [seed; 32],
            amount,
            nonce,
        }
    }

    #[test]
    fn store_evicts_oldest() {
        let store = OffchainStore::new();
        for i in 0..(MAX_BATCHES + 1) {
            store.insert(vec![entry(i as u8, 10, i as u64)], [1; 32], i as u64);
        }
        assert_eq!(store.len(), MAX_BATCHES);
        assert!(store.get(1).is_none());
        assert!(store.get(2).is_some());
        assert!(store.get((MAX_BATCHES + 1) as u64).is_some());
    }

    #[test]
    fn merkle_root_single_entry() {
        let leaf = entry_leaf(&entry(1, 10, 7));
        assert_eq!(merkle_root(&[leaf]), leaf);
    }

    #[test]
    fn merkle_root_two_entries() {
        let left = entry_leaf(&entry(1, 10, 7));
        let right = entry_leaf(&entry(2, 11, 8));
        assert_eq!(merkle_root(&[left, right]), node_hash(left, right));
    }

    #[test]
    fn merkle_proof_verify() {
        let entries = vec![entry(1, 10, 7), entry(2, 11, 8), entry(3, 12, 9)];
        let leaves: Vec<[u8; 32]> = entries.iter().map(entry_leaf).collect();
        let record = BatchRecord {
            batch_id: 1,
            root: merkle_root(&leaves),
            anchor_tx_hash: [9; 32],
            entries,
            leaves,
        };
        let proof = merkle_proof(&record, 1).expect("proof");
        let leaf = entry_leaf(&record.entries[1]);
        assert!(verify_proof(leaf, &proof, record.root));
    }
}
