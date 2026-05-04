//! Локальные off-chain команды (без RPC).

use ed25519_dalek::SigningKey;
use pwm_core::{merkle_root, sign_batch};

pub(crate) fn run_off_demo() {
    let a = [1u8; 32];
    let b = [2u8; 32];
    let root = merkle_root(&[a, b]);
    let skb = slip10_ed25519::derive_ed25519_private_key(&[5u8; 32], &[]);
    let sk = SigningKey::from_bytes(&skb);
    let sig = sign_batch(&sk, 1u64, root);
    let out = serde_json::json!({
        "batch_id": 1u64,
        "merkle_root_hex": hex::encode(root),
        "sig_hex": hex::encode(sig),
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}
