//! Outbound seed dialing: local node hello builder and retry classification.

use super::*;
use serde::Deserialize;

fn retryable_connect_outcome(
    err: impl Into<String>,
) -> (
    DialAttemptResult,
    Option<PeerClass>,
    Option<String>,
    Option<String>,
) {
    (
        DialAttemptResult::RetryableFail,
        None,
        None,
        Some(err.into()),
    )
}

/// Wire-only fake digest for [`App::broke_trust_test`] (64 hex chars = 32 bytes); must not match a real `pwm_core::digest(state0)`.
pub(crate) const TRUST_TEST_FAKE_GENESIS_HEX: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

pub(crate) fn build_local_node_hello(
    app: &App,
    genesis_hash: Option<String>,
    bridge_commitment: Option<String>,
    now_ms: u64,
    chain_tip_height: Option<u64>,
) -> NodeHello {
    let genesis_hash = if app.broke_trust_test {
        Some(TRUST_TEST_FAKE_GENESIS_HEX.to_string())
    } else {
        genesis_hash
    };
    let key = local_hello_signing_key(&app.identity);
    let nonce_seq = app.hello_nonce_ctr.fetch_add(1, Ordering::Relaxed);
    let mut nonce = nonce_seq.to_be_bytes().to_vec();
    nonce.extend_from_slice(&now_ms.to_be_bytes());
    let mut hello = NodeHello {
        network_id: app.identity.network_id.clone(),
        genesis_hash,
        cluster: handshake::NodeHelloCluster {
            domain_hi: app.identity.cluster_domain_hi,
            cluster_id: app.identity.cluster_id.clone(),
        },
        node: handshake::NodeHelloNode {
            node_id: app.identity.node_id.clone(),
            pubkey: key.verifying_key().to_bytes(),
        },
        capabilities: handshake::NodeHelloCapabilities {
            protocol_version: "0.1.0".to_string(),
            tx_features: vec!["local_transfer_v1".to_string()],
            services: vec!["mempool".to_string(), "sync".to_string()],
        },
        nonce,
        timestamp_ms: now_ms,
        signature: Vec::new(),
        federation_shard_id: Some(crate::runtime_shard_label(&app.identity, app.shard)),
        chain_tip_height,
        bridge_commitment,
    };
    let _ = hello.sign(&key);
    hello
}

pub(crate) fn local_hello_signing_key(identity: &RuntimeIdentity) -> SigningKey {
    let mut seed = [0u8; 32];
    let material = format!(
        "pwmd-local-node-hello|{}|{:02X}|{}|{}",
        identity.network_id, identity.cluster_domain_hi, identity.cluster_id, identity.node_id
    );
    for (i, b) in material.as_bytes().iter().enumerate() {
        seed[i % 32] = seed[i % 32].wrapping_add(*b).rotate_left((i % 8) as u32);
    }
    SigningKey::from_bytes(&seed)
}

#[derive(Deserialize)]
struct SeedStatus {
    #[serde(default)]
    ready: bool,
    #[serde(default)]
    network_id: Option<String>,
    #[serde(default)]
    cluster_domain_hi: Option<u8>,
    #[serde(default)]
    node_id: Option<String>,
    #[serde(default)]
    effective_genesis_hash: Option<String>,
    #[serde(default)]
    genesis_guard: Option<String>,
}

#[derive(Deserialize)]
struct PeerHelloAck {
    accepted: bool,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    node_hello: Option<NodeHello>,
}

fn seed_base(seed: SocketAddr) -> String {
    format!("http://{seed}")
}

pub(crate) async fn attempt_seed_connect(
    app: &App,
    seed: SocketAddr,
    connect_timeout_ms: u64,
    handshake_timeout_ms: u64,
    now_ms: u64,
) -> (
    DialAttemptResult,
    Option<PeerClass>,
    Option<String>,
    Option<String>,
) {
    let genesis_hash = {
        let hs = app.handshake.read().await;
        hs.validation_ctx.expected_genesis_hash.clone()
    };
    let chain_tip_height = {
        let g = app.inner.read().await;
        Some(g.chain.tip_h())
    };
    let bridge_commitment = crate::bridge_trust::local_bridge_commitment(app).await;
    let local_hello = build_local_node_hello(
        app,
        genesis_hash,
        Some(bridge_commitment.clone()),
        now_ms,
        chain_tip_height,
    );
    let total_timeout_ms = connect_timeout_ms
        .saturating_add(handshake_timeout_ms)
        .max(500);
    let client = match reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(connect_timeout_ms.max(1)))
        .timeout(Duration::from_millis(total_timeout_ms))
        .build()
    {
        Ok(v) => v,
        Err(e) => return retryable_connect_outcome(format!("seed {seed} http_client: {e}")),
    };
    let base = seed_base(seed);
    let status_url = format!("{base}/v1/status");
    let status_resp = match client.get(&status_url).send().await {
        Ok(v) => v,
        Err(e) => {
            let kind = if e.is_timeout() {
                "connect_timeout"
            } else if e.is_connect() {
                "connect_failed"
            } else {
                "status_request_failed"
            };
            return retryable_connect_outcome(format!("seed {seed} {kind}: {e}"));
        }
    };
    let http_status = status_resp.status();
    if !http_status.is_success() {
        let body = status_resp.text().await.unwrap_or_default();
        return retryable_connect_outcome(format!(
            "seed {seed} status_http={http_status} body={body}"
        ));
    }
    let status = match status_resp.json::<SeedStatus>().await {
        Ok(v) => v,
        Err(e) => {
            return retryable_connect_outcome(format!("seed {seed} status_decode_failed: {e}"))
        }
    };
    if !status.ready {
        return retryable_connect_outcome(format!("seed {seed} not_ready"));
    }
    if let Some(peer_net) = status.network_id.as_deref() {
        if peer_net != app.identity.network_id {
            return retryable_connect_outcome(format!(
                "seed {seed} network_mismatch expected_network_id={} received_network_id={peer_net}",
                app.identity.network_id
            ));
        }
    }
    if let (Some(expected), Some(received)) = (
        local_hello.genesis_hash.as_deref(),
        status.effective_genesis_hash.as_deref(),
    ) {
        if expected != received {
            let node_id = status
                .node_id
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            let mut hs = app.handshake.write().await;
            hs.genesis_guard.blocked = true;
            hs.genesis_guard.mismatch_total = hs.genesis_guard.mismatch_total.saturating_add(1);
            hs.genesis_guard.last_mismatch = Some(GenesisMismatchSnapshot {
                expected_hash: Some(expected.to_string()),
                received_hash: Some(received.to_string()),
                peer_node_id: node_id,
                peer_hint: seed.to_string(),
                at_unix_ms: now_ms,
            });
            let err = format!(
                "seed {seed} genesis_mismatch expected_genesis_hash={expected} received_genesis_hash={received}"
            );
            set_peer_error(&mut hs, now_ms, err.clone());
            warn!(
                "peer seed status rejected seed={} reason=genesis_mismatch expected_genesis_hash={} received_genesis_hash={} peer_domain_hi={:?} peer_node_id={:?} genesis_guard={:?}",
                seed,
                expected,
                received,
                status.cluster_domain_hi,
                status.node_id,
                status.genesis_guard
            );
            return retryable_connect_outcome(err);
        }
    }
    let hello_url = format!("{base}/v1/peer/hello");
    let hello_resp = match client.post(&hello_url).json(&local_hello).send().await {
        Ok(v) => v,
        Err(e) => {
            let kind = if e.is_timeout() {
                "hello_timeout"
            } else if e.is_connect() {
                "hello_connect_failed"
            } else {
                "hello_request_failed"
            };
            return retryable_connect_outcome(format!("seed {seed} {kind}: {e}"));
        }
    };
    let hello_status = hello_resp.status();
    if !hello_status.is_success() {
        let body = hello_resp.text().await.unwrap_or_default();
        return retryable_connect_outcome(format!(
            "seed {seed} hello_http={hello_status} body={body}"
        ));
    }
    let ack = match hello_resp.json::<PeerHelloAck>().await {
        Ok(v) => v,
        Err(e) => {
            return retryable_connect_outcome(format!("seed {seed} hello_decode_failed: {e}"))
        }
    };
    if !ack.accepted {
        return retryable_connect_outcome(format!(
            "seed {seed} hello_rejected reason={}",
            ack.reason.unwrap_or_else(|| "unknown".to_string())
        ));
    }
    let remote = match ack.node_hello {
        Some(v) => v,
        None => return retryable_connect_outcome(format!("seed {seed} no_matching_peer_hello")),
    };
    let mut hs = app.handshake.write().await;
    let node_id = remote.node.node_id.clone();
    match process_incoming_peer_hello(
        &mut hs,
        &remote,
        now_ms,
        &seed.to_string(),
        true,
        Some(bridge_commitment.as_str()),
    ) {
        Ok(class) => {
            clear_peer_error(&mut hs);
            drop(hs);
            crate::federation::merge_remote_hello(app, &remote, now_ms).await;
            (DialAttemptResult::Success, Some(class), Some(node_id), None)
        }
        Err(label) => (
            DialAttemptResult::RetryableFail,
            None,
            Some(node_id),
            Some(format!("seed {seed} remote_hello_rejected reason={label}")),
        ),
    }
}

#[cfg(test)]
mod trust_test_fake_genesis_tests {
    use super::{build_local_node_hello, TRUST_TEST_FAKE_GENESIS_HEX};
    use crate::bootstrap::app_from_dev_net;

    #[test]
    fn broke_trust_test_overrides_hello_genesis_field() {
        let mut app = app_from_dev_net();
        app.broke_trust_test = true;
        let h = build_local_node_hello(
            &app,
            Some("aa".repeat(32)),
            None,
            0,
            None,
        );
        assert_eq!(h.genesis_hash.as_deref(), Some(TRUST_TEST_FAKE_GENESIS_HEX));
    }
}
