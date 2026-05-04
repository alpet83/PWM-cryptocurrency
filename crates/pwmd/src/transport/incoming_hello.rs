//! Validation and bookkeeping for inbound `NodeHello` (trusted vs untrusted provenance).

use tracing::{info, warn};

use crate::handshake::{validate_node_hello, NodeHello};

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
) -> Result<PeerClass, String> {
    let validation_ctx = hs.validation_ctx.clone();
    let validation = validate_node_hello(hello, &validation_ctx, now_ms, &mut hs.replay);
    match validation {
        Ok(()) => {
            // Bridge commitment: same `domain_hi` peers (replicas) must agree on level-2 digest.
            // Cross-shard peers have different `exported_registry` / `imported_set` by definition —
            // comparing remote digest to *local* would always fail; only require wire presence when
            // the peer advertises a commitment (optional field for legacy cross-shard wire).
            if let Some(expected) = expected_bridge_commitment {
                let same_shard = hello.cluster.domain_hi == hs.local_domain_hi;
                if same_shard {
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
            let class = classify_peer(hs.local_domain_hi, hello.cluster.domain_hi);
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
            if provenance_trusted {
                hs.trusted_peers.insert(
                    hello.node.node_id.clone(),
                    TrustedPeer {
                        node_id: hello.node.node_id.clone(),
                        cluster_id: hello.cluster.cluster_id.clone(),
                        pubkey: hello.node.pubkey,
                        domain_hi: hello.cluster.domain_hi,
                    },
                );
            }
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
                    "peer hello rejected peer={} reason={} expected_genesis_hash={:?} received_genesis_hash={:?}",
                    peer_hint, label, hs.validation_ctx.expected_genesis_hash, hello.genesis_hash
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
