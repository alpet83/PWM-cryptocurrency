//! Validation and bookkeeping for inbound `NodeHello` (trusted vs untrusted provenance).

use tracing::{info, warn};

use crate::handshake::{
    protocol_compat, validate_node_hello, DeploymentProfile, NodeHello, ProtocolCompat, SealRole,
    PWM_PROTOCOL_VERSION, REASON_LABEL_ACTIVE_CONFLICT,
};

use super::handshake_state::{GenesisMismatchSnapshot, HandshakeState};
use super::health::{count_native_live_peers, refresh_native_health};
use super::lifecycle::set_peer_error;
use super::metrics::{increment_class_accept_total, increment_reject_reason_total};
use super::policy::{class_label, classify_peer, select_backoff_for_class};
use super::{PeerClass, PeerRecord, PeerStatus, TrustedPeer};

pub(crate) fn process_incoming_peer_hello(
    hs: &mut HandshakeState,
    hello: &NodeHello,
    now_ms: u64,
    peer_hint: &str,
    provenance_trusted: bool,
    expected_bridge_commitment: Option<&str>,
    expected_cluster_id: &str,
) -> Result<PeerClass, String> {
    let validation_ctx = hs.validation_ctx.clone();
    let validation = validate_node_hello(hello, &validation_ctx, now_ms, &mut hs.replay);
    match validation {
        Ok(()) => {
            let class = classify_peer(hs.local_domain_hi, hello.cluster.domain_hi);
            let reject_guard = |hs: &mut HandshakeState, reason: &str, detail: String| {
                set_peer_error(hs, now_ms, detail.clone());
                warn!(
                    target: "pwmd::peer",
                    "peer hello rejected peer={} reason={} detail={}",
                    peer_hint,
                    reason,
                    detail
                );
                reason.to_string()
            };
            if class == PeerClass::Native && hello.cluster.cluster_id != expected_cluster_id {
                let detail = format!(
                    "same_shard_cluster_id_mismatch expected_cluster_id={} received_cluster_id={}",
                    expected_cluster_id, hello.cluster.cluster_id
                );
                return Err(reject_guard(hs, "same_shard_cluster_id_mismatch", detail));
            }
            let local_validator_hash = hs
                .local_validator_hash
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty());
            let local_instance_id = hs
                .local_instance_id
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty());
            if class == PeerClass::Native {
                if let (Some(local_hash), Some(remote_hash)) =
                    (local_validator_hash, hello.capabilities.validator_hash())
                {
                    if local_hash == remote_hash {
                        let remote_instance_id = hello.capabilities.node_instance_id();
                        let same_instance = remote_instance_id
                            .zip(local_instance_id)
                            .is_some_and(|(remote, local)| remote == local);
                        if !same_instance {
                            let local_role = hs.local_seal_role;
                            let remote_role = hello.capabilities.seal_role;
                            let strict_single =
                                hs.deployment_profile == DeploymentProfile::SingleSealer;
                            let both_active = matches!(local_role, SealRole::Active)
                                && matches!(remote_role, SealRole::Active);
                            if strict_single && both_active {
                                hs.metrics.rejected_total =
                                    hs.metrics.rejected_total.saturating_add(1);
                                increment_reject_reason_total(
                                    &mut hs.metrics,
                                    REASON_LABEL_ACTIVE_CONFLICT,
                                );
                                let detail = format!(
                                    "same_validator_active_conflict local_instance_id={} remote_instance_id={} local_role={:?} remote_role={:?} deployment_profile=single_sealer validator_identity_hash={}",
                                    local_instance_id.unwrap_or("unknown"),
                                    remote_instance_id.unwrap_or("unknown"),
                                    local_role,
                                    remote_role,
                                    local_hash
                                );
                                return Err(reject_guard(hs, REASON_LABEL_ACTIVE_CONFLICT, detail));
                            }
                            let mode = if both_active {
                                "same_validator_active_conflict_allowed_experimental"
                            } else {
                                "same_validator_active_standby_allowed"
                            };
                            info!(
                                target: "pwmd::peer",
                                "peer hello same-validator peer={} mode={} local_instance_id={} remote_instance_id={} local_role={:?} remote_role={:?} deployment_profile={:?} validator_identity_hash={}",
                                peer_hint,
                                mode,
                                local_instance_id.unwrap_or("unknown"),
                                remote_instance_id.unwrap_or("unknown"),
                                local_role,
                                remote_role,
                                hs.deployment_profile,
                                local_hash
                            );
                        }
                    }
                }
            }
            match protocol_compat(hello.capabilities.protocol_version.as_str()) {
                Ok(ProtocolCompat::Exact) => {}
                Ok(ProtocolCompat::FractionalMismatch) => {
                    warn!(
                        target: "pwmd::peer",
                        "peer protocol_version_fractional_mismatch peer={} local_version={} remote_version={}",
                        peer_hint,
                        PWM_PROTOCOL_VERSION,
                        hello.capabilities.protocol_version
                    );
                }
                Err(reason) => {
                    hs.metrics.rejected_total = hs.metrics.rejected_total.saturating_add(1);
                    increment_reject_reason_total(&mut hs.metrics, reason.as_label());
                    let detail = if matches!(
                        reason,
                        crate::handshake::HandshakeRejectReason::ProtocolVersionMajorMismatch
                    ) {
                        format!(
                            "protocol_version_major_mismatch expected_version={} received_version={}",
                            PWM_PROTOCOL_VERSION, hello.capabilities.protocol_version
                        )
                    } else {
                        format!(
                            "protocol_version_malformed expected_version={} received_version={}",
                            PWM_PROTOCOL_VERSION, hello.capabilities.protocol_version
                        )
                    };
                    let reason_label = reason.as_label();
                    return Err(reject_guard(hs, reason_label, detail));
                }
            }
            if hello.capabilities.sync_profile.is_some() && !hello.capabilities.supports_sync_v1() {
                let reason = if class == PeerClass::Native {
                    "same_shard_sync_profile_incompatible"
                } else {
                    "inter_shard_sync_profile_incompatible"
                };
                let detail = format!(
                    "{} protocol_version={} services={:?}",
                    reason, hello.capabilities.protocol_version, hello.capabilities.services
                );
                return Err(reject_guard(hs, reason, detail));
            }
            // Bridge commitment: same `domain_hi` peers (replicas) must agree on level-2 digest.
            // Cross-shard peers have different `exported_registry` / `imported_set` by definition —
            // comparing remote digest to *local* would always fail; only require wire presence when
            // the peer advertises a commitment (optional field for legacy cross-shard wire).
            if class == PeerClass::Native {
                if let Some(expected) = expected_bridge_commitment {
                    match hello.bridge_commitment.as_deref() {
                        None => {
                            let detail = format!(
                                "bridge_federation_trust_refused expected_bridge_commitment={} received_bridge_commitment=",
                                expected
                            );
                            hs.bridge_trust.refused = true;
                            hs.bridge_trust.refusal_total =
                                hs.bridge_trust.refusal_total.saturating_add(1);
                            hs.bridge_trust.refusal_reason = Some(detail.clone());
                            set_peer_error(hs, now_ms, detail.clone());
                            warn!(
                                target: "pwmd::peer",
                                "peer hello rejected peer={} reason=bridge_commitment_missing expected_bridge_commitment={}",
                                peer_hint, expected
                            );
                            return Err("bridge_federation_trust_refused".to_string());
                        }
                        Some(got) if got != expected => {
                            let detail = format!(
                                "bridge_federation_trust_refused expected_bridge_commitment={} received_bridge_commitment={}",
                                expected, got
                            );
                            hs.bridge_trust.refused = true;
                            hs.bridge_trust.refusal_total =
                                hs.bridge_trust.refusal_total.saturating_add(1);
                            hs.bridge_trust.refusal_reason = Some(detail.clone());
                            set_peer_error(hs, now_ms, detail.clone());
                            warn!(
                                target: "pwmd::peer",
                                "peer hello rejected peer={} reason=bridge_commitment_mismatch expected_bridge_commitment={} received_bridge_commitment={}",
                                peer_hint,
                                expected,
                                got
                            );
                            return Err("bridge_federation_trust_refused".to_string());
                        }
                        Some(_) => {}
                    }
                }
            }
            // Federation trust restored after a prior mismatch (non-sticky across successful hello).
            hs.bridge_trust.refused = false;
            hs.bridge_trust.refusal_reason = None;
            let _ = select_backoff_for_class(&mut hs.policy, &class);
            hs.metrics.accepted_total += 1;
            increment_class_accept_total(&mut hs.metrics, &class);
            hs.peers.insert(
                hello.node.node_id.clone(),
                PeerRecord {
                    node_id: hello.node.node_id.clone(),
                    domain_hi: hello.cluster.domain_hi,
                    class: class.clone(),
                    last_seen_ms: now_ms,
                    status: PeerStatus::Connected,
                },
            );
            // Validated/signed hello must become trusted immediately on both inbound and
            // outbound paths; otherwise cluster frames can race between peer discovery and
            // trusted map population, causing first-round attest drops.
            let _ = provenance_trusted;
            hs.trusted_peers.insert(
                hello.node.node_id.clone(),
                TrustedPeer {
                    node_id: hello.node.node_id.clone(),
                    cluster_id: hello.cluster.cluster_id.clone(),
                    pubkey: hello.node.pubkey,
                    domain_hi: hello.cluster.domain_hi,
                    instance_id: hello.capabilities.node_instance_id.clone(),
                    cluster_attest_enabled: hello.capabilities.cluster_attest_enabled,
                    cluster_role: hello.capabilities.cluster_role,
                },
            );
            let native_live = count_native_live_peers(hs);
            refresh_native_health(hs, native_live, false);
            info!(
                target: "pwmd::peer",
                "peer hello accepted node_id={} peer={} domain_hi=0x{:02X} class={}",
                hello.node.node_id,
                peer_hint,
                hello.cluster.domain_hi,
                class_label(&class)
            );
            Ok(class)
        }
        Err(reason) => {
            let label = reason.as_label().to_string();
            hs.metrics.rejected_total += 1;
            increment_reject_reason_total(&mut hs.metrics, &label);
            let detail = match reason {
                crate::handshake::HandshakeRejectReason::GenesisMismatch => format!(
                    "peer hello rejected peer={} node_id={} reason={} expected_network_id={} received_network_id={} expected_genesis_hash={:?} received_genesis_hash={:?} local_cluster_id={} remote_cluster_id={} local_domain_hi=0x{:02X} remote_domain_hi=0x{:02X}",
                    peer_hint,
                    hello.node.node_id,
                    label,
                    hs.validation_ctx.expected_network_id,
                    hello.network_id,
                    hs.validation_ctx.expected_genesis_hash,
                    hello.genesis_hash,
                    expected_cluster_id,
                    hello.cluster.cluster_id,
                    hs.local_domain_hi,
                    hello.cluster.domain_hi,
                ),
                crate::handshake::HandshakeRejectReason::NetworkMismatch => format!(
                    "peer hello rejected peer={} reason={} expected_network_id={} received_network_id={}",
                    peer_hint, label, hs.validation_ctx.expected_network_id, hello.network_id
                ),
                _ => format!("peer hello rejected peer={} reason={}", peer_hint, label),
            };
            if matches!(
                reason,
                crate::handshake::HandshakeRejectReason::GenesisMismatch
            ) {
                hs.genesis_guard.blocked = true;
                hs.genesis_guard.mismatch_total = hs.genesis_guard.mismatch_total.saturating_add(1);
                hs.genesis_guard.last_mismatch = Some(GenesisMismatchSnapshot {
                    expected_hash: hs.validation_ctx.expected_genesis_hash.clone(),
                    received_hash: hello.genesis_hash.clone(),
                    peer_node_id: hello.node.node_id.clone(),
                    peer_hint: peer_hint.to_string(),
                    at_unix_ms: now_ms,
                });
            }
            set_peer_error(hs, now_ms, detail.clone());
            warn!(
                target: "pwmd::peer",
                "peer hello rejected node_id={} peer={} reason={} expected_network_id={} received_network_id={} expected_genesis_hash={:?} received_genesis_hash={:?}",
                hello.node.node_id,
                peer_hint,
                label,
                hs.validation_ctx.expected_network_id,
                hello.network_id,
                hs.validation_ctx.expected_genesis_hash,
                hello.genesis_hash
            );
            Err(label)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handshake::{
        HandshakeValidationCtx, NodeHelloCapabilities, NodeHelloCluster, NodeHelloNode,
        NodeHelloSyncProfile, PWM_PROTOCOL_VERSION, SYNC_BLK_MAX, SYNC_HDR_MAX, SYNC_TX_MAX,
        SYNC_WIRE_V1,
    };

    fn signed_hello(
        domain_hi: u8,
        cluster_id: &str,
        sync_profile: Option<NodeHelloSyncProfile>,
        services: Vec<String>,
        proto_ver: &str,
        seal_role: crate::handshake::SealRole,
        validator_hash: &str,
        instance_id: &str,
    ) -> NodeHello {
        let sk = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]);
        let mut hello = NodeHello {
            network_id: "devnet".to_string(),
            genesis_hash: Some("genesis-a".to_string()),
            cluster: NodeHelloCluster {
                domain_hi,
                cluster_id: cluster_id.to_string(),
            },
            node: NodeHelloNode {
                node_id: format!("node-{:02x}", domain_hi),
                pubkey: sk.verifying_key().to_bytes(),
            },
            capabilities: NodeHelloCapabilities {
                protocol_version: proto_ver.to_string(),
                tx_features: vec!["local_transfer_v1".to_string()],
                services,
                sync_profile,
                deployment_profile: crate::handshake::DeploymentProfile::SingleSealer,
                seal_role,
                validator_identity_hash: Some(validator_hash.to_string()),
                node_instance_id: Some(instance_id.to_string()),
                lease_owner_id: None,
                lease_term: None,
                lease_expires_at_ms: None,
                lease_last_tip: None,
                lease_fence: None,
                cluster_attest_enabled: false,
                cluster_role: crate::handshake::ClusterRole::None,
                cluster_members: Vec::new(),
                cluster_quorum_k: None,
                cluster_quorum_n: None,
            },
            nonce: vec![1, 2, 3, domain_hi],
            timestamp_ms: 1_000,
            signature: Vec::new(),
            chain_tip_height: None,
            federation_shard_id: None,
            bridge_commitment: None,
        };
        hello.sign(&sk).expect("sign hello");
        hello
    }

    fn hs() -> HandshakeState {
        let mut hs = HandshakeState::new(
            HandshakeValidationCtx {
                expected_network_id: "devnet".to_string(),
                expected_genesis_hash: Some("genesis-a".to_string()),
                skew_window_ms: 10_000,
            },
            0x10,
        );
        hs.deployment_profile = crate::handshake::DeploymentProfile::SingleSealer;
        hs.local_seal_role = crate::handshake::SealRole::Active;
        hs.local_validator_hash = Some("vh-test".to_string());
        hs.local_instance_id = Some("local-inst".to_string());
        hs
    }

    fn valid_sync_profile() -> NodeHelloSyncProfile {
        NodeHelloSyncProfile {
            sync_wire_version: SYNC_WIRE_V1,
            max_headers_per_msg: SYNC_HDR_MAX,
            max_blocks_per_msg: SYNC_BLK_MAX,
            max_txs_per_msg: SYNC_TX_MAX,
            supports_epoch_catchup: true,
        }
    }

    #[test]
    fn reject_same_shard_cluster_mismatch() {
        let mut hs = hs();
        let hello = signed_hello(
            0x10,
            "cluster-b",
            Some(valid_sync_profile()),
            vec!["sync".to_string()],
            PWM_PROTOCOL_VERSION,
            crate::handshake::SealRole::Active,
            "vh-foreign",
            "inst-b",
        );
        let err =
            process_incoming_peer_hello(&mut hs, &hello, 1_001, "peer-x", true, None, "cluster-a")
                .expect_err("must reject mismatched same-shard cluster");
        assert_eq!(err, "same_shard_cluster_id_mismatch");
    }

    #[test]
    fn reject_same_shard_sync_gap() {
        let mut hs = hs();
        let mut bad = valid_sync_profile();
        bad.max_headers_per_msg = 0;
        let hello = signed_hello(
            0x10,
            "cluster-a",
            Some(bad),
            vec!["sync".to_string()],
            PWM_PROTOCOL_VERSION,
            crate::handshake::SealRole::Active,
            "vh-foreign",
            "inst-c",
        );
        let err =
            process_incoming_peer_hello(&mut hs, &hello, 1_001, "peer-x", true, None, "cluster-a")
                .expect_err("must reject invalid same-shard sync profile");
        assert_eq!(err, "same_shard_sync_profile_incompatible");
    }

    #[test]
    fn reject_inter_shard_sync_gap() {
        let mut hs = hs();
        let mut bad = valid_sync_profile();
        bad.max_blocks_per_msg = 0;
        let hello = signed_hello(
            0x20,
            "cluster-z",
            Some(bad),
            vec!["sync".to_string()],
            PWM_PROTOCOL_VERSION,
            crate::handshake::SealRole::Active,
            "vh-foreign",
            "inst-d",
        );
        let err =
            process_incoming_peer_hello(&mut hs, &hello, 1_001, "peer-y", false, None, "cluster-a")
                .expect_err("must reject invalid inter-shard sync profile");
        assert_eq!(err, "inter_shard_sync_profile_incompatible");
    }

    #[test]
    fn reject_proto_major_gap() {
        let mut hs = hs();
        let hello = signed_hello(
            0x10,
            "cluster-a",
            Some(valid_sync_profile()),
            vec!["sync".to_string()],
            "1.0.0",
            crate::handshake::SealRole::Active,
            "vh-foreign",
            "inst-e",
        );
        let err =
            process_incoming_peer_hello(&mut hs, &hello, 1_001, "peer-z", true, None, "cluster-a")
                .expect_err("must reject major mismatch");
        assert_eq!(err, "protocol_version_major_mismatch");
        assert_eq!(
            hs.metrics
                .reject_reason_total
                .get("protocol_version_major_mismatch")
                .copied(),
            Some(1)
        );
    }

    #[test]
    fn accept_proto_minor_gap() {
        let mut hs = hs();
        let hello = signed_hello(
            0x10,
            "cluster-a",
            Some(valid_sync_profile()),
            vec!["sync".to_string()],
            "0.2.0",
            crate::handshake::SealRole::Active,
            "vh-foreign",
            "inst-f",
        );
        let class =
            process_incoming_peer_hello(&mut hs, &hello, 1_001, "peer-z", true, None, "cluster-a")
                .expect("minor mismatch must be allowed");
        assert_eq!(class, PeerClass::Native);
    }

    #[test]
    fn reject_same_validator_active_active() {
        let mut hs = hs();
        let hello = signed_hello(
            0x10,
            "cluster-a",
            Some(valid_sync_profile()),
            vec!["sync".to_string()],
            PWM_PROTOCOL_VERSION,
            crate::handshake::SealRole::Active,
            "vh-test",
            "remote-inst",
        );
        let err = process_incoming_peer_hello(
            &mut hs,
            &hello,
            1_001,
            "peer-same-validator",
            true,
            None,
            "cluster-a",
        )
        .expect_err("must reject same validator active/active");
        assert_eq!(err, "same_validator_active_conflict");
    }

    #[test]
    fn allow_same_validator_active_standby() {
        let mut hs = hs();
        let hello = signed_hello(
            0x10,
            "cluster-a",
            Some(valid_sync_profile()),
            vec!["sync".to_string()],
            PWM_PROTOCOL_VERSION,
            crate::handshake::SealRole::Standby,
            "vh-test",
            "remote-inst",
        );
        let class = process_incoming_peer_hello(
            &mut hs,
            &hello,
            1_001,
            "peer-same-validator",
            true,
            None,
            "cluster-a",
        )
        .expect("must allow same validator active/standby");
        assert_eq!(class, PeerClass::Native);
    }
}
