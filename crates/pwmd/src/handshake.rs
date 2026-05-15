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
pub const REASON_LABEL_PROTO_BAD: &str = "protocol_version_malformed";
pub const REASON_LABEL_PROTO_MAJOR: &str = "protocol_version_major_mismatch";
pub const REASON_LABEL_ACTIVE_CONFLICT: &str = "same_validator_active_conflict";
pub const PWM_PROTOCOL_VERSION: &str = "0.1.0";
pub const SYNC_WIRE_V1: u16 = 1;
pub const SYNC_HDR_MAX: u16 = 512;
pub const SYNC_BLK_MAX: u16 = 64;
pub const SYNC_TX_MAX: u16 = 2048;

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
    #[serde(default)]
    pub sync_profile: Option<NodeHelloSyncProfile>,
    #[serde(default)]
    pub deployment_profile: DeploymentProfile,
    #[serde(default)]
    pub seal_role: SealRole,
    #[serde(default)]
    pub validator_identity_hash: Option<String>,
    #[serde(default)]
    pub node_instance_id: Option<String>,
    #[serde(default)]
    pub lease_owner_id: Option<String>,
    #[serde(default)]
    pub lease_term: Option<u64>,
    #[serde(default)]
    pub lease_expires_at_ms: Option<u64>,
    #[serde(default)]
    pub lease_last_tip: Option<u64>,
    #[serde(default)]
    pub lease_fence: Option<u64>,
    /// RFC16 capability negotiation stays additive for legacy peers.
    #[serde(default)]
    pub cluster_attest_enabled: bool,
    #[serde(default)]
    pub cluster_role: ClusterRole,
    #[serde(default)]
    pub cluster_members: Vec<String>,
    #[serde(default)]
    pub cluster_quorum_k: Option<u8>,
    #[serde(default)]
    pub cluster_quorum_n: Option<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeHelloSyncProfile {
    pub sync_wire_version: u16,
    pub max_headers_per_msg: u16,
    pub max_blocks_per_msg: u16,
    pub max_txs_per_msg: u16,
    pub supports_epoch_catchup: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    FullV1,
    LegacyObserve,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentProfile {
    #[default]
    SingleSealer,
    MultiSealerExperimental,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SealRole {
    #[default]
    Active,
    Standby,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ClusterRole {
    #[default]
    None,
    Proposer,
    Attester,
}

impl NodeHelloCapabilities {
    pub fn supports_sync_v1(&self) -> bool {
        let has_sync = self.services.iter().any(|svc| svc == "sync");
        let Some(profile) = self.sync_profile.as_ref() else {
            return false;
        };
        has_sync
            && profile.sync_wire_version == SYNC_WIRE_V1
            && profile.max_headers_per_msg > 0
            && profile.max_headers_per_msg <= SYNC_HDR_MAX
            && profile.max_blocks_per_msg > 0
            && profile.max_blocks_per_msg <= SYNC_BLK_MAX
            && profile.max_txs_per_msg > 0
            && profile.max_txs_per_msg <= SYNC_TX_MAX
    }

    pub fn sync_mode(&self) -> SyncMode {
        if self.supports_sync_v1() {
            SyncMode::FullV1
        } else {
            SyncMode::LegacyObserve
        }
    }

    pub fn validator_hash(&self) -> Option<&str> {
        self.validator_identity_hash
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
    }

    pub fn node_instance_id(&self) -> Option<&str> {
        self.node_instance_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
    }

    pub fn lease_owner_id(&self) -> Option<&str> {
        self.lease_owner_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
    }
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
    ProtocolVersionMalformed,
    ProtocolVersionMajorMismatch,
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
            Self::ProtocolVersionMalformed => REASON_LABEL_PROTO_BAD,
            Self::ProtocolVersionMajorMismatch => REASON_LABEL_PROTO_MAJOR,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtocolVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolCompat {
    Exact,
    FractionalMismatch,
}

impl ProtocolVersion {
    pub fn parse(raw: &str) -> Result<Self, HandshakeRejectReason> {
        let mut parts = raw.trim().split('.');
        let Some(major_s) = parts.next() else {
            return Err(HandshakeRejectReason::ProtocolVersionMalformed);
        };
        let Some(minor_s) = parts.next() else {
            return Err(HandshakeRejectReason::ProtocolVersionMalformed);
        };
        let Some(patch_s) = parts.next() else {
            return Err(HandshakeRejectReason::ProtocolVersionMalformed);
        };
        if parts.next().is_some() {
            return Err(HandshakeRejectReason::ProtocolVersionMalformed);
        }
        let major = major_s
            .parse::<u64>()
            .map_err(|_| HandshakeRejectReason::ProtocolVersionMalformed)?;
        let minor = minor_s
            .parse::<u64>()
            .map_err(|_| HandshakeRejectReason::ProtocolVersionMalformed)?;
        let patch = patch_s
            .parse::<u64>()
            .map_err(|_| HandshakeRejectReason::ProtocolVersionMalformed)?;
        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

pub fn protocol_compat(remote_raw: &str) -> Result<ProtocolCompat, HandshakeRejectReason> {
    let local = ProtocolVersion::parse(PWM_PROTOCOL_VERSION)?;
    let remote = ProtocolVersion::parse(remote_raw)?;
    if local.major != remote.major {
        return Err(HandshakeRejectReason::ProtocolVersionMajorMismatch);
    }
    if local.minor != remote.minor || local.patch != remote.patch {
        return Ok(ProtocolCompat::FractionalMismatch);
    }
    Ok(ProtocolCompat::Exact)
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
        if self
            .capabilities
            .validator_identity_hash
            .as_deref()
            .is_some_and(|v| v.trim().is_empty())
            || self
                .capabilities
                .node_instance_id
                .as_deref()
                .is_some_and(|v| v.trim().is_empty())
            || self
                .capabilities
                .cluster_members
                .iter()
                .any(|v| v.trim().is_empty())
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
                protocol_version: PWM_PROTOCOL_VERSION.to_string(),
                tx_features: vec!["local_transfer_v1".to_string()],
                services: vec!["mempool".to_string(), "sync".to_string()],
                sync_profile: Some(NodeHelloSyncProfile {
                    sync_wire_version: SYNC_WIRE_V1,
                    max_headers_per_msg: SYNC_HDR_MAX,
                    max_blocks_per_msg: SYNC_BLK_MAX,
                    max_txs_per_msg: SYNC_TX_MAX,
                    supports_epoch_catchup: true,
                }),
                deployment_profile: DeploymentProfile::SingleSealer,
                seal_role: SealRole::Active,
                validator_identity_hash: Some("vhash".to_string()),
                node_instance_id: Some("inst-a".to_string()),
                lease_owner_id: None,
                lease_term: None,
                lease_expires_at_ms: None,
                lease_last_tip: None,
                lease_fence: None,
                cluster_attest_enabled: false,
                cluster_role: ClusterRole::None,
                cluster_members: Vec::new(),
                cluster_quorum_k: None,
                cluster_quorum_n: None,
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
        assert_eq!(
            HandshakeRejectReason::ProtocolVersionMalformed.as_label(),
            REASON_LABEL_PROTO_BAD
        );
        assert_eq!(
            HandshakeRejectReason::ProtocolVersionMajorMismatch.as_label(),
            REASON_LABEL_PROTO_MAJOR
        );
        assert_eq!(
            REASON_LABEL_ACTIVE_CONFLICT,
            "same_validator_active_conflict"
        );
    }

    #[test]
    fn mode_legacy_without_profile() {
        let (mut hello, _sk) = sample_hello();
        hello.capabilities.sync_profile = None;
        assert_eq!(hello.capabilities.sync_mode(), SyncMode::LegacyObserve);
    }

    #[test]
    fn mode_full_with_valid_profile() {
        let (hello, _sk) = sample_hello();
        assert_eq!(hello.capabilities.sync_mode(), SyncMode::FullV1);
    }

    #[test]
    fn parse_proto_ok() {
        let got = ProtocolVersion::parse("1.2.3").expect("must parse semver");
        assert_eq!(got.major, 1);
        assert_eq!(got.minor, 2);
        assert_eq!(got.patch, 3);
    }

    #[test]
    fn parse_proto_bad() {
        let err = ProtocolVersion::parse("1.2").expect_err("must reject malformed semver");
        assert_eq!(err, HandshakeRejectReason::ProtocolVersionMalformed);
    }

    #[test]
    fn compat_major_bad() {
        let err = protocol_compat("1.0.0").expect_err("must reject major mismatch");
        assert_eq!(err, HandshakeRejectReason::ProtocolVersionMajorMismatch);
    }

    #[test]
    fn compat_minor_warn() {
        let got = protocol_compat("0.2.0").expect("must allow minor mismatch");
        assert_eq!(got, ProtocolCompat::FractionalMismatch);
    }
}
