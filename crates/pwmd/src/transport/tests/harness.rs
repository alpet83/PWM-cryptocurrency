//! Async TCP harness scaffolding for mini peer nodes in transport tests.

use super::super::*;
use crate::handshake::NodeHello;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HarnessRole {
    Inbound,
    Outbound,
}

impl HarnessRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::Inbound => "inbound",
            Self::Outbound => "outbound",
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct HarnessEvent {
    pub(super) conn_id: u64,
    pub(super) role: &'static str,
    pub(super) local: Option<String>,
    pub(super) remote: Option<String>,
    pub(super) action: &'static str,
    pub(super) frame: Option<&'static str>,
    pub(super) error: Option<String>,
}

#[derive(Clone)]
pub(super) struct HarnessDiag {
    events: Arc<Mutex<Vec<HarnessEvent>>>,
    next_conn_id: Arc<AtomicU64>,
}

impl HarnessDiag {
    pub(super) fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            next_conn_id: Arc::new(AtomicU64::new(1)),
        }
    }

    fn next_conn_id(&self) -> u64 {
        self.next_conn_id.fetch_add(1, Ordering::Relaxed)
    }

    async fn record(
        &self,
        conn_id: u64,
        role: HarnessRole,
        stream: &tokio::net::TcpStream,
        action: &'static str,
        frame: Option<&'static str>,
        error: Option<String>,
    ) {
        self.events.lock().await.push(HarnessEvent {
            conn_id,
            role: role.as_str(),
            local: stream.local_addr().ok().map(|a| a.to_string()),
            remote: stream.peer_addr().ok().map(|a| a.to_string()),
            action,
            frame,
            error,
        });
    }

    pub(super) async fn snapshot(&self) -> Vec<HarnessEvent> {
        self.events.lock().await.clone()
    }
}

#[derive(Clone)]
pub(super) struct HarnessNode {
    pub(super) node_id: &'static str,
    pub(super) cluster_id: &'static str,
    pub(super) domain_hi: u8,
    pub(super) listen: SocketAddr,
    pub(super) seed: SocketAddr,
}

pub(super) fn app_with_identity(
    shard: DevLane,
    network_id: &str,
    domain_hi: u8,
    cluster_id: &str,
    node_id: &str,
) -> App {
    let (cfg, sks) = pwm_core::dev_net();
    let identity = RuntimeIdentity {
        network_id: network_id.to_string(),
        cluster_domain_hi: domain_hi,
        cluster_id: cluster_id.to_string(),
        node_id: node_id.to_string(),
        mode: RuntimeIdentityMode::Explicit,
    };
    crate::bootstrap::app_from_chain_boot(cfg, sks, None, shard, Some(identity))
}

pub(super) async fn close_text(app: &App) -> String {
    let hs = app.handshake.read().await;
    format!(
        "close={:?} error={:?} counters={:?}",
        hs.transport.snapshot.last_session_close_reason,
        hs.transport.snapshot.last_peer_error,
        hs.transport.snapshot.counters.peer_close_by_reason
    )
}

enum HarnessRead {
    Frame(PeerWireMsg),
    Idle(String),
    Closed(String),
}

pub(super) fn reserve_loopback_addr() -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve addr");
    listener.local_addr().expect("local addr")
}

fn harness_hello(node: &HarnessNode, now_ms: u64) -> NodeHello {
    let identity = RuntimeIdentity {
        network_id: "testnet-qa".to_string(),
        cluster_domain_hi: node.domain_hi,
        cluster_id: node.cluster_id.to_string(),
        node_id: node.node_id.to_string(),
        mode: RuntimeIdentityMode::Explicit,
    };
    let key = local_hello_signing_key(&identity);
    let mut hello = NodeHello {
        network_id: identity.network_id,
        genesis_hash: Some("peer-only-genesis".to_string()),
        cluster: crate::handshake::NodeHelloCluster {
            domain_hi: identity.cluster_domain_hi,
            cluster_id: identity.cluster_id,
        },
        node: crate::handshake::NodeHelloNode {
            node_id: identity.node_id,
            pubkey: key.verifying_key().to_bytes(),
        },
        capabilities: crate::handshake::NodeHelloCapabilities {
            protocol_version: "0.1.0".to_string(),
            tx_features: vec!["peer-only-harness".to_string()],
            services: vec!["peer".to_string()],
            sync_profile: None,
            deployment_profile: crate::handshake::DeploymentProfile::SingleSealer,
            seal_role: crate::handshake::SealRole::Active,
            validator_identity_hash: Some("vh-harness".to_string()),
            node_instance_id: Some(format!("inst-{}", node.node_id)),
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
        nonce: now_ms.to_be_bytes().to_vec(),
        timestamp_ms: now_ms,
        signature: Vec::new(),
        chain_tip_height: None,
        federation_shard_id: None,
        bridge_commitment: None,
    };
    hello.sign(&key).expect("sign harness hello");
    hello
}

fn frame_label(msg: &PeerWireMsg) -> &'static str {
    match msg {
        PeerWireMsg::Hello { .. } => "hello",
        PeerWireMsg::HelloAck { .. } => "hello_ack",
        PeerWireMsg::Heartbeat { .. } => "heartbeat",
        PeerWireMsg::HeartbeatAck { .. } => "heartbeat_ack",
        PeerWireMsg::CrossShardFacts { .. } => "cross_shard_facts",
        PeerWireMsg::AccountViews { .. } => "account_views",
        PeerWireMsg::SyncProfileAnnounce { .. } => "sync_profile_announce",
        PeerWireMsg::SyncTipAnnounce { .. } => "sync_tip_announce",
        PeerWireMsg::SyncHeadersReq { .. } => "sync_headers_req",
        PeerWireMsg::SyncHeadersBatch { .. } => "sync_headers_batch",
        PeerWireMsg::SyncBlocksReq { .. } => "sync_blocks_req",
        PeerWireMsg::SyncBlocksBatch { .. } => "sync_blocks_batch",
        PeerWireMsg::SyncTxAnnounce { .. } => "sync_tx_announce",
        PeerWireMsg::SyncTxReq { .. } => "sync_tx_req",
        PeerWireMsg::SyncTxBatch { .. } => "sync_tx_batch",
        PeerWireMsg::SyncNack { .. } => "sync_nack",
        PeerWireMsg::SyncCatchupReq { .. } => "sync_catchup_req",
        PeerWireMsg::SyncCatchupChunk { .. } => "sync_catchup_chunk",
        PeerWireMsg::SyncCatchupDone { .. } => "sync_catchup_done",
        PeerWireMsg::ClusterPropose { .. } => "cluster_propose",
        PeerWireMsg::ClusterAttest { .. } => "cluster_attest",
    }
}

async fn write_diag(
    stream: &mut tokio::net::TcpStream,
    msg: &PeerWireMsg,
    timeout_ms: u64,
    diag: &HarnessDiag,
    conn_id: u64,
    role: HarnessRole,
) -> Result<(), String> {
    let frame = frame_label(msg);
    match write_wire_msg(stream, msg, timeout_ms).await {
        Ok(()) => {
            diag.record(conn_id, role, stream, "sent", Some(frame), None)
                .await;
            Ok(())
        }
        Err(err) => {
            diag.record(
                conn_id,
                role,
                stream,
                "write_error",
                Some(frame),
                Some(err.clone()),
            )
            .await;
            Err(err)
        }
    }
}

async fn read_diag(
    stream: &mut tokio::net::TcpStream,
    timeout_ms: u64,
    diag: &HarnessDiag,
    conn_id: u64,
    role: HarnessRole,
) -> HarnessRead {
    match read_wire_msg(stream, timeout_ms).await {
        Ok(msg) => {
            diag.record(conn_id, role, stream, "read", Some(frame_label(&msg)), None)
                .await;
            HarnessRead::Frame(msg)
        }
        Err(err) if is_wire_timeout(&err) => {
            diag.record(conn_id, role, stream, "idle", None, Some(err.clone()))
                .await;
            HarnessRead::Idle(err)
        }
        Err(err) => {
            diag.record(conn_id, role, stream, "read_error", None, Some(err.clone()))
                .await;
            HarnessRead::Closed(err)
        }
    }
}

pub(super) async fn run_inbound_peer(
    listener: tokio::net::TcpListener,
    node: HarnessNode,
    diag: HarnessDiag,
) -> Result<(), String> {
    let (mut stream, _) = listener
        .accept()
        .await
        .map_err(|e| format!("accept_failed: {e}"))?;
    let conn_id = diag.next_conn_id();
    let hello = match read_diag(&mut stream, 500, &diag, conn_id, HarnessRole::Inbound).await {
        HarnessRead::Frame(PeerWireMsg::Hello { node_hello }) => node_hello,
        HarnessRead::Frame(other) => return Err(format!("expected_hello got={other:?}")),
        HarnessRead::Idle(err) | HarnessRead::Closed(err) => return Err(err),
    };
    let now_ms = current_time_ms().unwrap_or(0);
    let ack = PeerWireMsg::HelloAck {
        accepted: true,
        reason: None,
        node_hello: Some(harness_hello(&node, now_ms)),
    };
    write_diag(&mut stream, &ack, 500, &diag, conn_id, HarnessRole::Inbound).await?;
    write_diag(
        &mut stream,
        &PeerWireMsg::CrossShardFacts { facts: Vec::new() },
        500,
        &diag,
        conn_id,
        HarnessRole::Inbound,
    )
    .await?;
    let mut idle_reads = 0u8;
    loop {
        match read_diag(&mut stream, 120, &diag, conn_id, HarnessRole::Inbound).await {
            HarnessRead::Frame(PeerWireMsg::Heartbeat { unix_ms, .. }) => {
                idle_reads = 0;
                write_diag(
                    &mut stream,
                    &PeerWireMsg::HeartbeatAck { unix_ms },
                    500,
                    &diag,
                    conn_id,
                    HarnessRole::Inbound,
                )
                .await?;
            }
            HarnessRead::Frame(PeerWireMsg::AccountViews { .. })
            | HarnessRead::Frame(PeerWireMsg::CrossShardFacts { .. })
            | HarnessRead::Frame(PeerWireMsg::HeartbeatAck { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncProfileAnnounce { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncTipAnnounce { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncHeadersReq { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncHeadersBatch { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncBlocksReq { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncBlocksBatch { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncTxAnnounce { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncTxReq { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncTxBatch { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncNack { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncCatchupReq { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncCatchupChunk { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncCatchupDone { .. })
            | HarnessRead::Frame(PeerWireMsg::ClusterPropose { .. })
            | HarnessRead::Frame(PeerWireMsg::ClusterAttest { .. }) => {
                idle_reads = 0;
            }
            HarnessRead::Frame(PeerWireMsg::Hello { .. } | PeerWireMsg::HelloAck { .. }) => {
                return Err("unexpected_handshake_frame".to_string());
            }
            HarnessRead::Idle(_) => {
                idle_reads = idle_reads.saturating_add(1);
                if idle_reads > 20 {
                    return Ok(());
                }
            }
            HarnessRead::Closed(err) => {
                return Err(format!("wire_read_failed: {err}"));
            }
        }
        if hello.node.node_id.is_empty() {
            return Err("empty_remote_node_id".to_string());
        }
    }
}

pub(super) async fn run_outbound_peer(
    node: HarnessNode,
    diag: HarnessDiag,
    heartbeat_count: usize,
) -> Result<(), String> {
    let mut stream = tokio::time::timeout(
        Duration::from_millis(500),
        tokio::net::TcpStream::connect(node.seed),
    )
    .await
    .map_err(|_| "connect_timeout".to_string())?
    .map_err(|e| format!("connect_failed: {e}"))?;
    let conn_id = diag.next_conn_id();
    let now_ms = current_time_ms().unwrap_or(0);
    write_diag(
        &mut stream,
        &PeerWireMsg::Hello {
            node_hello: harness_hello(&node, now_ms),
        },
        500,
        &diag,
        conn_id,
        HarnessRole::Outbound,
    )
    .await?;
    match read_diag(&mut stream, 500, &diag, conn_id, HarnessRole::Outbound).await {
        HarnessRead::Frame(PeerWireMsg::HelloAck { accepted: true, .. }) => {}
        HarnessRead::Frame(other) => return Err(format!("expected_hello_ack got={other:?}")),
        HarnessRead::Idle(err) | HarnessRead::Closed(err) => return Err(err),
    }

    let mut saw_idle = false;
    for _ in 0..4 {
        match read_diag(&mut stream, 10, &diag, conn_id, HarnessRole::Outbound).await {
            HarnessRead::Idle(_) => {
                saw_idle = true;
                break;
            }
            HarnessRead::Frame(
                PeerWireMsg::CrossShardFacts { .. } | PeerWireMsg::AccountViews { .. },
            )
            | HarnessRead::Frame(PeerWireMsg::SyncProfileAnnounce { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncTipAnnounce { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncHeadersReq { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncHeadersBatch { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncBlocksReq { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncBlocksBatch { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncTxAnnounce { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncTxReq { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncTxBatch { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncNack { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncCatchupReq { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncCatchupChunk { .. })
            | HarnessRead::Frame(PeerWireMsg::SyncCatchupDone { .. })
            | HarnessRead::Frame(PeerWireMsg::ClusterPropose { .. })
            | HarnessRead::Frame(PeerWireMsg::ClusterAttest { .. }) => {}
            HarnessRead::Frame(other) => {
                return Err(format!("idle_probe_unexpected_frame: {other:?}"));
            }
            HarnessRead::Closed(err) => return Err(format!("idle_socket_closed: {err}")),
        }
    }
    if !saw_idle {
        return Err("idle_probe_missing_timeout".to_string());
    }
    write_diag(
        &mut stream,
        &PeerWireMsg::AccountViews { rows: Vec::new() },
        500,
        &diag,
        conn_id,
        HarnessRole::Outbound,
    )
    .await?;

    for _ in 0..heartbeat_count {
        tokio::time::sleep(Duration::from_millis(40)).await;
        let unix_ms = current_time_ms().unwrap_or(0);
        write_diag(
            &mut stream,
            &PeerWireMsg::Heartbeat {
                unix_ms,
                chain_tip_height: None,
                lease_owner_id: None,
                lease_term: None,
                lease_expires_at_ms: None,
                lease_last_tip: None,
                lease_fence: None,
                federation_shard_id: None,
                federation_gossip: None,
            },
            500,
            &diag,
            conn_id,
            HarnessRole::Outbound,
        )
        .await?;
        let mut acked = false;
        while !acked {
            match read_diag(&mut stream, 80, &diag, conn_id, HarnessRole::Outbound).await {
                HarnessRead::Frame(PeerWireMsg::HeartbeatAck { .. }) => acked = true,
                HarnessRead::Frame(PeerWireMsg::Heartbeat { unix_ms, .. }) => {
                    write_diag(
                        &mut stream,
                        &PeerWireMsg::HeartbeatAck { unix_ms },
                        500,
                        &diag,
                        conn_id,
                        HarnessRole::Outbound,
                    )
                    .await?;
                }
                HarnessRead::Frame(
                    PeerWireMsg::CrossShardFacts { .. } | PeerWireMsg::AccountViews { .. },
                ) => {}
                HarnessRead::Frame(PeerWireMsg::SyncProfileAnnounce { .. })
                | HarnessRead::Frame(PeerWireMsg::SyncTipAnnounce { .. })
                | HarnessRead::Frame(PeerWireMsg::SyncHeadersReq { .. })
                | HarnessRead::Frame(PeerWireMsg::SyncHeadersBatch { .. })
                | HarnessRead::Frame(PeerWireMsg::SyncBlocksReq { .. })
                | HarnessRead::Frame(PeerWireMsg::SyncBlocksBatch { .. })
                | HarnessRead::Frame(PeerWireMsg::SyncTxAnnounce { .. })
                | HarnessRead::Frame(PeerWireMsg::SyncTxReq { .. })
                | HarnessRead::Frame(PeerWireMsg::SyncTxBatch { .. })
                | HarnessRead::Frame(PeerWireMsg::SyncNack { .. })
                | HarnessRead::Frame(PeerWireMsg::SyncCatchupReq { .. })
                | HarnessRead::Frame(PeerWireMsg::SyncCatchupChunk { .. })
                | HarnessRead::Frame(PeerWireMsg::SyncCatchupDone { .. })
                | HarnessRead::Frame(PeerWireMsg::ClusterPropose { .. })
                | HarnessRead::Frame(PeerWireMsg::ClusterAttest { .. }) => {}
                HarnessRead::Frame(PeerWireMsg::Hello { .. } | PeerWireMsg::HelloAck { .. }) => {
                    return Err("heartbeat_unexpected_handshake_frame".to_string());
                }
                HarnessRead::Idle(_) => {}
                HarnessRead::Closed(err) => {
                    return Err(format!("heartbeat_read_failed: {err}"));
                }
            }
        }
    }
    Ok(())
}

pub(super) fn format_events(events: &[HarnessEvent]) -> String {
    events
        .iter()
        .map(|e| {
            format!(
                "conn={} role={} local={:?} remote={:?} action={} frame={:?} error={:?}",
                e.conn_id, e.role, e.local, e.remote, e.action, e.frame, e.error
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
