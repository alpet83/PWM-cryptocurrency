//! Handshake identity envelope groundwork for RFC-8.
//!
//! This module is network-agnostic on purpose: it provides data structures and
//! local validation gates that can be reused by a future p2p transport layer.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const REASON_LABEL_BAD_SIGNATURE: &str = "bad_signature";
pub const REASON_LABEL_REPLAY_NONCE: &str = "replay_nonce";
pub const REASON_LABEL_NETWORK_MISMATCH: &str = "network_mismatch";
pub const REASON_LABEL_GENESIS_MISMATCH: &str = "genesis_mismatch";
pub const REASON_LABEL_TIMESTAMP_SKEW: &str = "timestamp_skew";
pub const REASON_LABEL_MALFORMED: &str = "malformed";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeHello {
    pub network_id: String,
    pub genesis_hash: Option<String>,
    pub cluster: NodeHelloCluster,
    pub node: NodeHelloNode,
    pub capabilities: NodeHelloCapabilities,
    pub nonce: Vec<u8>,
    pub timestamp_ms: u64,
    pub signature: Vec<u8>,
    /// Optional chain tip for federation dictionary merges (wire-compat default None).
    #[serde(default)]
    pub chain_tip_height: Option<u64>,
    /// Optional shard label for federation rows (wire-compat default None).
    #[serde(default)]
    pub federation_shard_id: Option<String>,
    /// Bridge-only commitment digest (no full chain `state_root`).
    #[serde(default)]
    pub bridge_commitment: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeHelloCluster {
    pub domain_hi: u8,
    pub cluster_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeHelloNode {
    pub node_id: String,
    pub pubkey: [u8; 32],
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeHelloCapabilities {
    pub protocol_version: String,
    pub tx_features: Vec<String>,
    pub services: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandshakeValidationCtx {
    pub expected_network_id: String,
    pub expected_genesis_hash: Option<String>,
    pub skew_window_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HandshakeRejectReason {
    BadSignature,
    ReplayNonce,
    NetworkMismatch,
    GenesisMismatch,
    TimestampSkew,
    Malformed,
}

impl HandshakeRejectReason {
    /// Stable reason label for future metrics/log dimensions.
    pub fn as_label(&self) -> &'static str {
        match self {
            Self::BadSignature => REASON_LABEL_BAD_SIGNATURE,
            Self::ReplayNonce => REASON_LABEL_REPLAY_NONCE,
            Self::NetworkMismatch => REASON_LABEL_NETWORK_MISMATCH,
            Self::GenesisMismatch => REASON_LABEL_GENESIS_MISMATCH,
            Self::TimestampSkew => REASON_LABEL_TIMESTAMP_SKEW,
            Self::Malformed => REASON_LABEL_MALFORMED,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ReplayNonceCache {
    seen: HashMap<Vec<u8>, u64>,
}

impl ReplayNonceCache {
    pub fn check_and_insert(&mut self, nonce: &[u8], now_ms: u64, window_ms: u64) -> bool {
        self.prune(now_ms, window_ms);
        let key = nonce.to_vec();
        if self.seen.contains_key(&key) {
            return false;
        }
        self.seen.insert(key, now_ms);
        true
    }

    fn prune(&mut self, now_ms: u64, window_ms: u64) {
        self.seen
            .retain(|_, ts| now_ms.saturating_sub(*ts) <= window_ms);
    }
}

#[derive(Serialize)]
struct NodeHelloSigned<'a> {
    network_id: &'a str,
    genesis_hash: &'a Option<String>,
    cluster: &'a NodeHelloCluster,
    node: &'a NodeHelloNode,
    capabilities: &'a NodeHelloCapabilities,
    nonce: &'a [u8],
    timestamp_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    chain_tip_height: &'a Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    federation_shard_id: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bridge_commitment: &'a Option<String>,
}

impl NodeHello {
    pub fn sign(&mut self, signing_key: &SigningKey) -> Result<(), HandshakeRejectReason> {
        self.validate_mandatory_fields()?;
        let msg = self.signing_payload()?;
        self.signature = signing_key.sign(&msg).to_bytes().to_vec();
        Ok(())
    }

    pub fn verify_signature(&self) -> Result<(), HandshakeRejectReason> {
        if self.signature.len() != 64 {
            return Err(HandshakeRejectReason::BadSignature);
        }
        let sig_bytes: [u8; 64] = self
            .signature
            .as_slice()
            .try_into()
            .map_err(|_| HandshakeRejectReason::BadSignature)?;
        let sig = Signature::from_bytes(&sig_bytes);
        let vk = VerifyingKey::from_bytes(&self.node.pubkey)
            .map_err(|_| HandshakeRejectReason::BadSignature)?;
        let msg = self.signing_payload()?;
        vk.verify(&msg, &sig)
            .map_err(|_| HandshakeRejectReason::BadSignature)
    }

    fn signing_payload(&self) -> Result<Vec<u8>, HandshakeRejectReason> {
        let envelope = NodeHelloSigned {
            network_id: &self.network_id,
            genesis_hash: &self.genesis_hash,
            cluster: &self.cluster,
            node: &self.node,
            capabilities: &self.capabilities,
            nonce: &self.nonce,
            timestamp_ms: self.timestamp_ms,
            chain_tip_height: &self.chain_tip_height,
            federation_shard_id: &self.federation_shard_id,
            bridge_commitment: &self.bridge_commitment,
        };
        serde_json::to_vec(&envelope).map_err(|_| HandshakeRejectReason::Malformed)
    }

    pub fn validate_mandatory_fields(&self) -> Result<(), HandshakeRejectReason> {
        if self.network_id.trim().is_empty()
            || self.cluster.cluster_id.trim().is_empty()
            || self.node.node_id.trim().is_empty()
            || self.capabilities.protocol_version.trim().is_empty()
            || self.nonce.is_empty()
        {
            return Err(HandshakeRejectReason::Malformed);
        }
        if self
            .capabilities
            .tx_features
            .iter()
            .any(|f| f.trim().is_empty())
            || self
                .capabilities
                .services
                .iter()
                .any(|s| s.trim().is_empty())
        {
            return Err(HandshakeRejectReason::Malformed);
        }
        Ok(())
    }
}

pub fn validate_node_hello(
    hello: &NodeHello,
    ctx: &HandshakeValidationCtx,
    now_ms: u64,
    replay: &mut ReplayNonceCache,
) -> Result<(), HandshakeRejectReason> {
    hello.validate_mandatory_fields()?;
    if hello.network_id != ctx.expected_network_id {
        return Err(HandshakeRejectReason::NetworkMismatch);
    }
    if let Some(expected) = &ctx.expected_genesis_hash {
        if hello.genesis_hash.as_ref() != Some(expected) {
            return Err(HandshakeRejectReason::GenesisMismatch);
        }
    }
    hello.verify_signature()?;
    let skew = now_ms.abs_diff(hello.timestamp_ms);
    if skew > ctx.skew_window_ms {
        return Err(HandshakeRejectReason::TimestampSkew);
    }
    if !replay.check_and_insert(&hello.nonce, now_ms, ctx.skew_window_ms) {
        return Err(HandshakeRejectReason::ReplayNonce);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_hello() -> (NodeHello, SigningKey) {
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let hello = NodeHello {
            network_id: "devnet".to_string(),
            genesis_hash: Some("abc123".to_string()),
            cluster: NodeHelloCluster {
                domain_hi: 0x10,
                cluster_id: "cluster-a".to_string(),
            },
            node: NodeHelloNode {
                node_id: "node-a".to_string(),
                pubkey: sk.verifying_key().to_bytes(),
            },
            capabilities: NodeHelloCapabilities {
                protocol_version: "0.1.0".to_string(),
                tx_features: vec!["local_transfer_v1".to_string()],
                services: vec!["mempool".to_string(), "sync".to_string()],
            },
            nonce: vec![1, 2, 3, 4],
            timestamp_ms: 1000,
            signature: Vec::new(),
            chain_tip_height: None,
            federation_shard_id: None,
            bridge_commitment: None,
        };
        (hello, sk)
    }

    /// Sign/verify + JSON round-trip for hello payloads (formerly `node_hello_sign_verify_and_serde_roundtrip`).
    #[test]
    fn hello_signed_serde_round() {
        let (mut hello, sk) = sample_hello();
        hello.sign(&sk).unwrap();
        hello.verify_signature().unwrap();
        let encoded = serde_json::to_vec(&hello).unwrap();
        let decoded: NodeHello = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, hello);
    }

    /// Detect corrupted signatures (formerly `validate_node_hello_rejects_bad_signature`).
    #[test]
    fn hello_reject_bad_sig() {
        let (mut hello, sk) = sample_hello();
        hello.sign(&sk).unwrap();
        hello.signature[0] ^= 0xFF;
        let ctx = HandshakeValidationCtx {
            expected_network_id: "devnet".to_string(),
            expected_genesis_hash: Some("abc123".to_string()),
            skew_window_ms: 1_000,
        };
        let mut replay = ReplayNonceCache::default();
        let err = validate_node_hello(&hello, &ctx, 1000, &mut replay).unwrap_err();
        assert_eq!(err, HandshakeRejectReason::BadSignature);
    }

    /// Second submit with identical nonce rejects (formerly `validate_node_hello_rejects_replay_nonce`).
    #[test]
    fn hello_reject_replay_nonce() {
        let (mut hello, sk) = sample_hello();
        hello.sign(&sk).unwrap();
        let ctx = HandshakeValidationCtx {
            expected_network_id: "devnet".to_string(),
            expected_genesis_hash: Some("abc123".to_string()),
            skew_window_ms: 5_000,
        };
        let mut replay = ReplayNonceCache::default();
        validate_node_hello(&hello, &ctx, 1000, &mut replay).unwrap();
        let err = validate_node_hello(&hello, &ctx, 1001, &mut replay).unwrap_err();
        assert_eq!(err, HandshakeRejectReason::ReplayNonce);
    }

    /// Timestamp outside skew window rejects (formerly `validate_node_hello_rejects_timestamp_skew`).
    #[test]
    fn hello_reject_time_skew() {
        let (mut hello, sk) = sample_hello();
        hello.sign(&sk).unwrap();
        let ctx = HandshakeValidationCtx {
            expected_network_id: "devnet".to_string(),
            expected_genesis_hash: Some("abc123".to_string()),
            skew_window_ms: 50,
        };
        let mut replay = ReplayNonceCache::default();
        let err = validate_node_hello(&hello, &ctx, 1_000 + 500, &mut replay).unwrap_err();
        assert_eq!(err, HandshakeRejectReason::TimestampSkew);
    }

    /// Wrong `network_id` fails validation (formerly `validate_node_hello_rejects_network_mismatch`).
    #[test]
    fn hello_reject_net_mismatch() {
        let (mut hello, sk) = sample_hello();
        hello.sign(&sk).unwrap();
        let ctx = HandshakeValidationCtx {
            expected_network_id: "testnet-v1".to_string(),
            expected_genesis_hash: Some("abc123".to_string()),
            skew_window_ms: 500,
        };
        let mut replay = ReplayNonceCache::default();
        let err = validate_node_hello(&hello, &ctx, 1000, &mut replay).unwrap_err();
        assert_eq!(err, HandshakeRejectReason::NetworkMismatch);
    }

    /// Genesis hash mismatch rejects (formerly `validate_node_hello_rejects_genesis_mismatch`).
    #[test]
    fn hello_genesis_bad() {
        let (mut hello, sk) = sample_hello();
        hello.sign(&sk).unwrap();
        let ctx = HandshakeValidationCtx {
            expected_network_id: "devnet".to_string(),
            expected_genesis_hash: Some("different-genesis".to_string()),
            skew_window_ms: 500,
        };
        let mut replay = ReplayNonceCache::default();
        let err = validate_node_hello(&hello, &ctx, 1000, &mut replay).unwrap_err();
        assert_eq!(err, HandshakeRejectReason::GenesisMismatch);
    }

    /// Boundary timestamps exactly at skew allowance pass (formerly `validate_node_hello_accepts_skew_at_window_boundary`).
    #[test]
    fn hello_skew_bound_ok() {
        let (mut hello, sk) = sample_hello();
        hello.sign(&sk).unwrap();
        let ctx = HandshakeValidationCtx {
            expected_network_id: "devnet".to_string(),
            expected_genesis_hash: Some("abc123".to_string()),
            skew_window_ms: 100,
        };
        let mut replay = ReplayNonceCache::default();
        validate_node_hello(&hello, &ctx, 1100, &mut replay).unwrap();
    }

    /// Nonce bucket prune allows logically later hello with same nonce bytes (formerly `validate_node_hello_allows_reused_nonce_after_window_prune`).
    #[test]
    fn nonce_ok_gap_pruned() {
        let (mut hello, sk) = sample_hello();
        let ctx = HandshakeValidationCtx {
            expected_network_id: "devnet".to_string(),
            expected_genesis_hash: Some("abc123".to_string()),
            skew_window_ms: 100,
        };
        let mut replay = ReplayNonceCache::default();

        hello.timestamp_ms = 1000;
        hello.sign(&sk).unwrap();
        validate_node_hello(&hello, &ctx, 1000, &mut replay).unwrap();

        hello.timestamp_ms = 1201;
        hello.sign(&sk).unwrap();
        validate_node_hello(&hello, &ctx, 1201, &mut replay).unwrap();
    }

    /// Empty mandatory fields yields malformed handshake (formerly `validate_node_hello_rejects_malformed_payload`).
    #[test]
    fn hello_reject_malformed() {
        let (mut hello, sk) = sample_hello();
        hello.network_id = String::new();
        hello.sign(&sk).unwrap_err();
        hello.network_id = "devnet".to_string();
        hello.sign(&sk).unwrap();
        hello.capabilities.services.push("".to_string());
        let ctx = HandshakeValidationCtx {
            expected_network_id: "devnet".to_string(),
            expected_genesis_hash: Some("abc123".to_string()),
            skew_window_ms: 500,
        };
        let mut replay = ReplayNonceCache::default();
        let err = validate_node_hello(&hello, &ctx, 1000, &mut replay).unwrap_err();
        assert_eq!(err, HandshakeRejectReason::Malformed);
    }

    #[test]
    fn reason_labels_are_stable() {
        assert_eq!(
            HandshakeRejectReason::BadSignature.as_label(),
            REASON_LABEL_BAD_SIGNATURE
        );
        assert_eq!(
            HandshakeRejectReason::ReplayNonce.as_label(),
            REASON_LABEL_REPLAY_NONCE
        );
        assert_eq!(
            HandshakeRejectReason::NetworkMismatch.as_label(),
            REASON_LABEL_NETWORK_MISMATCH
        );
        assert_eq!(
            HandshakeRejectReason::GenesisMismatch.as_label(),
            REASON_LABEL_GENESIS_MISMATCH
        );
        assert_eq!(
            HandshakeRejectReason::TimestampSkew.as_label(),
            REASON_LABEL_TIMESTAMP_SKEW
        );
        assert_eq!(
            HandshakeRejectReason::Malformed.as_label(),
            REASON_LABEL_MALFORMED
        );
    }
}
