//! Tests wiring real `process_inbound_socket` paths against TcpStreams.

use super::super::peer_session::{
    record_cluster_propose_originated, ClusterAttestWire, ClusterProposeWire,
};
use super::super::*;
use super::harness::{app_with_identity, close_text};
use crate::lifecycle::run_cluster_gate;
use ed25519_dalek::SigningKey;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::prelude::*;

#[derive(Clone, Default)]
struct LogLines {
    buf: Arc<Mutex<Vec<u8>>>,
}

struct LogWriter {
    buf: Arc<Mutex<Vec<u8>>>,
}

impl Write for LogWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if let Ok(mut buf) = self.buf.lock() {
            buf.extend_from_slice(data);
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for LogLines {
    type Writer = LogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LogWriter {
            buf: self.buf.clone(),
        }
    }
}

impl LogLines {
    fn lines(&self) -> Vec<String> {
        if let Ok(buf) = self.buf.lock() {
            String::from_utf8_lossy(&buf)
                .lines()
                .map(str::to_owned)
                .collect()
        } else {
            Vec::new()
        }
    }
}

fn warn_log_scope() -> (LogLines, tracing::dispatcher::DefaultGuard) {
    let sink = LogLines::default();
    let sub = tracing_subscriber::registry()
        .with(tracing_subscriber::filter::LevelFilter::WARN)
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .without_time()
                .with_target(false)
                .with_writer(sink.clone()),
        );
    let dispatch = tracing::Dispatch::new(sub);
    let guard = tracing::dispatcher::set_default(&dispatch);
    (sink, guard)
}

fn cluster_sig_line(
    sk: &SigningKey,
    height: u64,
    round: u32,
    vote_object: &str,
    candidate_hash: &str,
    candidate_ref: Option<&str>,
) -> String {
    let cref = candidate_ref.unwrap_or("");
    let msg = format!("{height}\n{round}\n{vote_object}\n{candidate_hash}\n{cref}");
    hex::encode(pwm_core::crypto::sign(sk, msg.as_bytes()))
}

async fn trust_attester(app: &App, node_id: &str, member_id: &str, pubkey: [u8; 32]) {
    let mut hs = app.handshake.write().await;
    hs.trusted_peers.insert(
        node_id.to_string(),
        crate::transport::TrustedPeer {
            node_id: node_id.to_string(),
            cluster_id: app.identity.cluster_id.clone(),
            pubkey,
            domain_hi: app.identity.cluster_domain_hi,
            instance_id: Some(member_id.to_string()),
            cluster_attest_enabled: true,
            cluster_role: crate::handshake::ClusterRole::Attester,
        },
    );
}

async fn handshake_ib_client(app_in: App, app_out: &App) -> tokio::net::TcpStream {
    let mut cfg = TransportConfig::default();
    cfg.handshake_timeout_ms = 300;
    cfg.heartbeat_timeout_ms = 80;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind inbound");
    let addr = listener.local_addr().expect("inbound addr");
    tokio::spawn(async move {
        let (stream, peer) = listener.accept().await.expect("accept inbound");
        process_inbound_socket(&app_in, &cfg, stream, peer).await;
    });
    let mut client = tokio::net::TcpStream::connect(addr)
        .await
        .expect("connect inbound");
    let now_ms = current_time_ms().unwrap_or(0);
    let genesis_hash = {
        let hs = app_out.handshake.read().await;
        hs.validation_ctx.expected_genesis_hash.clone()
    };
    let chain_tip_height = {
        let g = app_out.inner.read().await;
        Some(g.chain.tip_h())
    };
    let bridge_commitment = crate::bridge_trust::local_bridge_commitment(app_out).await;
    let hello = build_local_node_hello(
        app_out,
        genesis_hash,
        Some(bridge_commitment),
        now_ms,
        chain_tip_height,
    );
    write_wire_msg(&mut client, &PeerWireMsg::Hello { node_hello: hello }, 300)
        .await
        .expect("write hello");
    match read_wire_msg(&mut client, 300)
        .await
        .expect("read hello ack")
    {
        PeerWireMsg::HelloAck { accepted: true, .. } => {}
        other => panic!("unexpected ack: {other:?}"),
    }
    for _ in 0..2 {
        let _ = read_wire_msg(&mut client, 300)
            .await
            .expect("read initial data");
    }
    client
}

async fn wait_attesters(app: &App, h: u64, r: u32, want: usize, timeout_ms: u64) -> bool {
    let stop = current_time_ms().unwrap_or(0).saturating_add(timeout_ms);
    loop {
        let got = {
            let hs = app.handshake.read().await;
            hs.cluster_attest
                .rounds
                .get(&(h, r))
                .map(|x| x.attesters.len())
                .unwrap_or(0)
        };
        if got >= want {
            return true;
        }
        if current_time_ms().unwrap_or(0) >= stop {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn cluster_diag(app: &App, h: u64, r: u32) -> String {
    let hs = app.handshake.read().await;
    let round = hs.cluster_attest.rounds.get(&(h, r));
    let round_txt = match round {
        Some(x) => format!(
            "vote={} cand={} cref={:?} proposer={:?} attesters={:?}",
            x.vote_object,
            x.candidate_hash,
            x.candidate_ref,
            x.proposer_id,
            x.attesters.keys().cloned().collect::<Vec<_>>()
        ),
        None => "missing".to_string(),
    };
    let peers = hs
        .trusted_peers
        .iter()
        .map(|(node_id, tp)| {
            format!(
                "{}=>inst={:?},role={:?},attest_en={}",
                node_id, tp.instance_id, tp.cluster_role, tp.cluster_attest_enabled
            )
        })
        .collect::<Vec<_>>();
    format!(
        "round[{},{}]={}; trusted_peers={:?}",
        h, r, round_txt, peers
    )
}

async fn round_bind(app: &App, h: u64, r: u32) -> (String, String, Option<String>) {
    let hs = app.handshake.read().await;
    let round = hs.cluster_attest.rounds.get(&(h, r)).expect("round state");
    (
        round.vote_object.clone(),
        round.candidate_hash.clone(),
        round.candidate_ref.clone(),
    )
}

/// process_inbound_socket tolerates repeated idle reads without misclassifying wire failures.
#[tokio::test]
async fn prod_ib_sock_idle_ok() {
    let app_a = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    let app_b = app_with_identity(DevLane::Lane1, "testnet-qa", 0x20, "cluster-b", "node-b");
    let mut cfg = TransportConfig::default();
    cfg.handshake_timeout_ms = 300;
    cfg.heartbeat_timeout_ms = 40;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind inbound");
    let addr = listener.local_addr().expect("inbound addr");
    let task_app = app_a.clone();
    let task_cfg = cfg.clone();
    tokio::spawn(async move {
        let (stream, peer) = listener.accept().await.expect("accept inbound");
        process_inbound_socket(&task_app, &task_cfg, stream, peer).await;
    });

    let mut client = tokio::net::TcpStream::connect(addr)
        .await
        .expect("connect inbound");
    let now_ms = current_time_ms().unwrap_or(0);
    let genesis_hash = {
        let hs = app_b.handshake.read().await;
        hs.validation_ctx.expected_genesis_hash.clone()
    };
    let chain_tip_height = {
        let g = app_b.inner.read().await;
        Some(g.chain.tip_h())
    };
    let hello = build_local_node_hello(&app_b, genesis_hash, None, now_ms, chain_tip_height);
    write_wire_msg(&mut client, &PeerWireMsg::Hello { node_hello: hello }, 300)
        .await
        .expect("write hello");
    match read_wire_msg(&mut client, 300)
        .await
        .expect("read hello ack")
    {
        PeerWireMsg::HelloAck { accepted: true, .. } => {}
        other => panic!("unexpected ack: {other:?}"),
    }
    for _ in 0..2 {
        let _ = read_wire_msg(&mut client, 300)
            .await
            .expect("read initial data");
    }

    tokio::time::sleep(Duration::from_millis(220)).await;
    let detail = close_text(&app_a).await;
    let hs = app_a.handshake.read().await;
    assert!(
        hs.transport.snapshot.session_untrusted_total >= 1,
        "inbound session not accepted: {detail}"
    );
    assert_eq!(
        hs.transport.snapshot.last_session_close_reason, None,
        "idle reads must not close inbound session: {detail}"
    );
    assert!(
        !detail.contains("wire_read_failed") && !detail.contains("heartbeat_read_failed"),
        "idle reads misclassified as read failure: {detail}"
    );
}

/// Stateful seed transport survives repeated idle heartbeat windows without closing the session.
#[tokio::test]
async fn prod_seed_idle_windows_ok() {
    let app_a = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    let app_b = app_with_identity(DevLane::Lane1, "testnet-qa", 0x20, "cluster-b", "node-b");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind seed");
    let seed = listener.local_addr().expect("seed addr");
    let seed_app = app_b.clone();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept seed");
        match read_wire_msg(&mut stream, 300).await.expect("read hello") {
            PeerWireMsg::Hello { .. } => {}
            other => panic!("unexpected seed hello: {other:?}"),
        }
        let now_ms = current_time_ms().unwrap_or(0);
        let genesis_hash = {
            let hs = seed_app.handshake.read().await;
            hs.validation_ctx.expected_genesis_hash.clone()
        };
        let chain_tip_height = {
            let g = seed_app.inner.read().await;
            Some(g.chain.tip_h())
        };
        let remote =
            build_local_node_hello(&seed_app, genesis_hash, None, now_ms, chain_tip_height);
        write_wire_msg(
            &mut stream,
            &PeerWireMsg::HelloAck {
                accepted: true,
                reason: None,
                node_hello: Some(remote),
            },
            300,
        )
        .await
        .expect("write ack");
        let stop_at = current_time_ms().unwrap_or(0).saturating_add(700);
        while current_time_ms().unwrap_or(0) < stop_at {
            match read_wire_msg(&mut stream, 80).await {
                Ok(_) => {}
                Err(err) if is_wire_timeout(&err) => {}
                Err(_) => break,
            }
        }
    });

    let mut cfg = TransportConfig::default();
    cfg.enabled = true;
    cfg.peer_seeds = vec![seed];
    cfg.connect_timeout_ms = 300;
    cfg.handshake_timeout_ms = 300;
    cfg.retry_base_ms = 50;
    cfg.heartbeat_interval_ms = 50;
    cfg.heartbeat_timeout_ms = 40;
    spawn_stateful_transport_loop(app_a.clone(), cfg);
    tokio::time::sleep(Duration::from_millis(520)).await;

    let detail = close_text(&app_a).await;
    let hs = app_a.handshake.read().await;
    assert!(
        hs.trusted_peers.contains_key("node-b"),
        "seed session not trusted: {detail}"
    );
    assert_eq!(
        hs.transport.snapshot.last_session_close_reason, None,
        "idle heartbeat windows must not close seed session: {detail}"
    );
    assert!(
        !detail.contains("wire_read_failed") && !detail.contains("heartbeat_read_failed"),
        "idle windows misclassified as read failure: {detail}"
    );
}

/// Session close diagnostics include nested low-level read errors alongside the summarized reason.
#[test]
fn prod_close_lvl_err_ok() {
    let detail = detail_with_err(
        "heartbeat_read_failed",
        "wire_read_payload_failed: connection reset by peer",
    );
    assert!(detail.contains("heartbeat_read_failed"));
    assert!(detail.contains("wire_read_payload_failed"));
    assert!(detail.contains("connection reset by peer"));
}

/// Corrupt sync wire payloads are dropped safely and reflected in drop-reason counters.
#[tokio::test]
async fn prod_bad_sync_frame_counted() {
    let app_a = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    let app_b = app_with_identity(DevLane::Lane1, "testnet-qa", 0x20, "cluster-b", "node-b");
    let mut cfg = TransportConfig::default();
    cfg.handshake_timeout_ms = 300;
    cfg.heartbeat_timeout_ms = 80;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind inbound");
    let addr = listener.local_addr().expect("inbound addr");
    let task_app = app_a.clone();
    let task_cfg = cfg.clone();
    tokio::spawn(async move {
        let (stream, peer) = listener.accept().await.expect("accept inbound");
        process_inbound_socket(&task_app, &task_cfg, stream, peer).await;
    });

    let mut client = tokio::net::TcpStream::connect(addr)
        .await
        .expect("connect inbound");
    let now_ms = current_time_ms().unwrap_or(0);
    let genesis_hash = {
        let hs = app_b.handshake.read().await;
        hs.validation_ctx.expected_genesis_hash.clone()
    };
    let chain_tip_height = {
        let g = app_b.inner.read().await;
        Some(g.chain.tip_h())
    };
    let hello = build_local_node_hello(&app_b, genesis_hash, None, now_ms, chain_tip_height);
    write_wire_msg(&mut client, &PeerWireMsg::Hello { node_hello: hello }, 300)
        .await
        .expect("write hello");
    match read_wire_msg(&mut client, 300)
        .await
        .expect("read hello ack")
    {
        PeerWireMsg::HelloAck { accepted: true, .. } => {}
        other => panic!("unexpected ack: {other:?}"),
    }
    for _ in 0..2 {
        let _ = read_wire_msg(&mut client, 300)
            .await
            .expect("read initial data");
    }

    let bad_payload = br#"{"type":"sync_tip_announce","hdr":"broken-json"#;
    let frame_len = (bad_payload.len() as u32).to_be_bytes();
    tokio::io::AsyncWriteExt::write_all(&mut client, &frame_len)
        .await
        .expect("write frame len");
    tokio::io::AsyncWriteExt::write_all(&mut client, bad_payload)
        .await
        .expect("write bad payload");
    tokio::time::sleep(Duration::from_millis(120)).await;

    let hs = app_a.handshake.read().await;
    assert!(
        hs.transport.snapshot.sync_v1_drop_total >= 1,
        "bad frame must increase sync drop total"
    );
    assert!(
        hs.transport
            .snapshot
            .sync_v1_drop_reason
            .get("decode_failed")
            .copied()
            .unwrap_or(0)
            >= 1,
        "decode_failed reason must be recorded"
    );
}

/// TCP wire round-trip (`ClusterPropose` + valid `ClusterAttest`) unlocks `run_cluster_gate` for 2-of-2.
#[tokio::test]
async fn cluster_2of2_gate_ok() {
    let mut app_a = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    app_a.cluster_cfg.enabled = true;
    app_a.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
    app_a.cluster_cfg.members = vec!["node-a".to_string(), "node-b".to_string()];
    app_a.cluster_cfg.quorum_n = 2;
    app_a.cluster_cfg.quorum_k = 1;
    app_a.cluster_cfg.attest_timeout_ms = 10;
    app_a.node_instance_id = "node-a".to_string();
    let mut app_b = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-b");
    app_b.cluster_cfg.enabled = true;
    app_b.cluster_cfg.role = crate::handshake::ClusterRole::Attester;
    app_b.cluster_cfg.members = app_a.cluster_cfg.members.clone();
    app_b.cluster_cfg.quorum_n = 2;
    app_b.cluster_cfg.quorum_k = 1;
    app_b.node_instance_id = "node-b".to_string();

    let mut client_ab = handshake_ib_client(app_b.clone(), &app_a).await;
    let next_h = app_a.inner.read().await.chain.tip_h().saturating_add(1);
    let vote = "vo-wire-2of2";
    let cand = "ab".repeat(32);
    let propose = ClusterProposeWire {
        height: next_h,
        round: 0,
        vote_object: vote.to_string(),
        candidate_hash: cand.clone(),
        candidate_ref: None,
        tail_blocks: Vec::new(),
    };
    write_wire_msg(
        &mut client_ab,
        &PeerWireMsg::ClusterPropose {
            msg: propose.clone(),
        },
        300,
    )
    .await
    .expect("write cluster propose");
    record_cluster_propose_originated(
        &app_a,
        propose,
        "node-a",
        Some(crate::current_time_ms().unwrap_or(0)),
    )
    .await;
    let mut client_ba = handshake_ib_client(app_a.clone(), &app_b).await;
    let sk_b = local_hello_signing_key(&app_b.identity);
    trust_attester(
        &app_a,
        &app_b.identity.node_id,
        &app_b.node_instance_id,
        sk_b.verifying_key().to_bytes(),
    )
    .await;
    let (vote_bind, cand_bind, cref_bind) = round_bind(&app_a, next_h, 0).await;
    let sig_b = cluster_sig_line(
        &sk_b,
        next_h,
        0,
        &vote_bind,
        &cand_bind,
        cref_bind.as_deref(),
    );
    write_wire_msg(
        &mut client_ba,
        &PeerWireMsg::ClusterAttest {
            msg: ClusterAttestWire {
                height: next_h,
                round: 0,
                vote_object: vote_bind,
                candidate_hash: cand_bind,
                signature: sig_b,
                candidate_ref: cref_bind,
                attester_tip_height: None,
            },
        },
        300,
    )
    .await
    .expect("write cluster attest");
    assert!(
        wait_attesters(&app_a, next_h, 0, 1, 500).await,
        "{}",
        cluster_diag(&app_a, next_h, 0).await
    );
    let (tip_before, hash_before) = {
        let g = app_a.inner.read().await;
        (g.chain.tip_h(), hex::encode(g.chain.tip_hash()))
    };
    assert!(run_cluster_gate(&app_a, None).await);
    {
        let mut g = app_a.inner.write().await;
        g.chain
            .seal(Vec::new())
            .expect("seal one block after cluster gate");
    }
    let (tip_after, hash_after) = {
        let g = app_a.inner.read().await;
        (g.chain.tip_h(), hex::encode(g.chain.tip_hash()))
    };
    assert_eq!(tip_after, tip_before.saturating_add(1));
    assert_ne!(hash_after, hash_before);
}

/// Two independent TCP attests from distinct members unlock a 2-of-3 gate.
#[tokio::test]
async fn cluster_2of3_gate_wire() {
    let mut app_a = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    app_a.cluster_cfg.enabled = true;
    app_a.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
    app_a.cluster_cfg.members = vec![
        "node-a".to_string(),
        "node-b".to_string(),
        "node-c".to_string(),
    ];
    app_a.cluster_cfg.quorum_n = 3;
    app_a.cluster_cfg.quorum_k = 2;
    app_a.node_instance_id = "node-a".to_string();
    let mut app_b = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-b");
    app_b.cluster_cfg.enabled = true;
    app_b.cluster_cfg.role = crate::handshake::ClusterRole::Attester;
    app_b.cluster_cfg.members = app_a.cluster_cfg.members.clone();
    app_b.cluster_cfg.quorum_n = 3;
    app_b.cluster_cfg.quorum_k = 2;
    app_b.node_instance_id = "node-b".to_string();
    let mut app_c = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-c");
    app_c.cluster_cfg.enabled = true;
    app_c.cluster_cfg.role = crate::handshake::ClusterRole::Attester;
    app_c.cluster_cfg.members = app_a.cluster_cfg.members.clone();
    app_c.cluster_cfg.quorum_n = 3;
    app_c.cluster_cfg.quorum_k = 2;
    app_c.node_instance_id = "node-c".to_string();
    let mut client_ab = handshake_ib_client(app_b.clone(), &app_a).await;
    let next_h = app_a.inner.read().await.chain.tip_h().saturating_add(1);
    let vote = "vo-wire-2of3";
    let cand = "bc".repeat(32);
    let propose = ClusterProposeWire {
        height: next_h,
        round: 0,
        vote_object: vote.to_string(),
        candidate_hash: cand.clone(),
        candidate_ref: None,
        tail_blocks: Vec::new(),
    };
    write_wire_msg(
        &mut client_ab,
        &PeerWireMsg::ClusterPropose {
            msg: propose.clone(),
        },
        300,
    )
    .await
    .expect("write cluster propose");
    record_cluster_propose_originated(
        &app_a,
        propose,
        "node-a",
        Some(crate::current_time_ms().unwrap_or(0)),
    )
    .await;

    let mut client_ba = handshake_ib_client(app_a.clone(), &app_b).await;
    let sk_b = local_hello_signing_key(&app_b.identity);
    trust_attester(
        &app_a,
        &app_b.identity.node_id,
        &app_b.node_instance_id,
        sk_b.verifying_key().to_bytes(),
    )
    .await;
    let (vote_bind, cand_bind, cref_bind) = round_bind(&app_a, next_h, 0).await;
    let sig_b = cluster_sig_line(
        &sk_b,
        next_h,
        0,
        &vote_bind,
        &cand_bind,
        cref_bind.as_deref(),
    );
    write_wire_msg(
        &mut client_ba,
        &PeerWireMsg::ClusterAttest {
            msg: ClusterAttestWire {
                height: next_h,
                round: 0,
                vote_object: vote_bind,
                candidate_hash: cand_bind.clone(),
                signature: sig_b,
                candidate_ref: cref_bind.clone(),
                attester_tip_height: None,
            },
        },
        300,
    )
    .await
    .expect("write cluster attest from b");

    let mut client_ca = handshake_ib_client(app_a.clone(), &app_c).await;
    let sk_c = local_hello_signing_key(&app_c.identity);
    trust_attester(
        &app_a,
        &app_c.identity.node_id,
        &app_c.node_instance_id,
        sk_c.verifying_key().to_bytes(),
    )
    .await;
    let (vote_bind, cand_bind, cref_bind) = round_bind(&app_a, next_h, 0).await;
    let sig_c = cluster_sig_line(
        &sk_c,
        next_h,
        0,
        &vote_bind,
        &cand_bind,
        cref_bind.as_deref(),
    );
    write_wire_msg(
        &mut client_ca,
        &PeerWireMsg::ClusterAttest {
            msg: ClusterAttestWire {
                height: next_h,
                round: 0,
                vote_object: vote_bind,
                candidate_hash: cand_bind,
                signature: sig_c,
                candidate_ref: cref_bind,
                attester_tip_height: None,
            },
        },
        300,
    )
    .await
    .expect("write cluster attest from c");

    assert!(
        wait_attesters(&app_a, next_h, 0, 2, 700).await,
        "{}",
        cluster_diag(&app_a, next_h, 0).await
    );
    let (tip_before, hash_before) = {
        let g = app_a.inner.read().await;
        (g.chain.tip_h(), hex::encode(g.chain.tip_hash()))
    };
    assert!(run_cluster_gate(&app_a, None).await);
    {
        let mut g = app_a.inner.write().await;
        g.chain
            .seal(Vec::new())
            .expect("seal one block after cluster gate");
    }
    let (tip_after, hash_after) = {
        let g = app_a.inner.read().await;
        (g.chain.tip_h(), hex::encode(g.chain.tip_hash()))
    };
    assert_eq!(tip_after, tip_before.saturating_add(1));
    assert_ne!(hash_after, hash_before);
}

/// One valid ACK in 2-of-3 mode keeps the gate closed.
#[tokio::test]
async fn cluster_2of3_one_ack_stuck() {
    let mut app_a = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    app_a.cluster_cfg.enabled = true;
    app_a.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
    app_a.cluster_cfg.members = vec![
        "node-a".to_string(),
        "node-b".to_string(),
        "node-c".to_string(),
    ];
    app_a.cluster_cfg.quorum_n = 3;
    app_a.cluster_cfg.quorum_k = 2;
    app_a.node_instance_id = "node-a".to_string();
    let mut app_b = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-b");
    app_b.cluster_cfg.enabled = true;
    app_b.cluster_cfg.role = crate::handshake::ClusterRole::Attester;
    app_b.cluster_cfg.members = app_a.cluster_cfg.members.clone();
    app_b.cluster_cfg.quorum_n = 3;
    app_b.cluster_cfg.quorum_k = 2;
    app_b.node_instance_id = "node-b".to_string();
    let mut client_ab = handshake_ib_client(app_b.clone(), &app_a).await;
    let next_h = app_a.inner.read().await.chain.tip_h().saturating_add(1);
    let vote = "vo-wire-2of3-one";
    let cand = "ca".repeat(32);
    let propose = ClusterProposeWire {
        height: next_h,
        round: 0,
        vote_object: vote.to_string(),
        candidate_hash: cand.clone(),
        candidate_ref: None,
        tail_blocks: Vec::new(),
    };
    write_wire_msg(
        &mut client_ab,
        &PeerWireMsg::ClusterPropose {
            msg: propose.clone(),
        },
        300,
    )
    .await
    .expect("write cluster propose");
    record_cluster_propose_originated(
        &app_a,
        propose,
        "node-a",
        Some(crate::current_time_ms().unwrap_or(0)),
    )
    .await;

    let mut client_ba = handshake_ib_client(app_a.clone(), &app_b).await;
    let sk_b = local_hello_signing_key(&app_b.identity);
    trust_attester(
        &app_a,
        &app_b.identity.node_id,
        &app_b.node_instance_id,
        sk_b.verifying_key().to_bytes(),
    )
    .await;
    let (vote_bind, cand_bind, cref_bind) = round_bind(&app_a, next_h, 0).await;
    let sig_b = cluster_sig_line(
        &sk_b,
        next_h,
        0,
        &vote_bind,
        &cand_bind,
        cref_bind.as_deref(),
    );
    write_wire_msg(
        &mut client_ba,
        &PeerWireMsg::ClusterAttest {
            msg: ClusterAttestWire {
                height: next_h,
                round: 0,
                vote_object: vote_bind,
                candidate_hash: cand_bind,
                signature: sig_b,
                candidate_ref: cref_bind,
                attester_tip_height: None,
            },
        },
        300,
    )
    .await
    .expect("write cluster attest from b");

    assert!(
        wait_attesters(&app_a, next_h, 0, 1, 500).await,
        "{}",
        cluster_diag(&app_a, next_h, 0).await
    );
    assert!(!run_cluster_gate(&app_a, None).await);
}

/// Missing attestation over TCP wire keeps cluster gate closed and reaches timeout path.
#[tokio::test(flavor = "current_thread")]
async fn cluster_timeout_no_seal() {
    let mut app_a = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    app_a.cluster_cfg.enabled = true;
    app_a.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
    app_a.cluster_cfg.members = vec!["node-a".to_string(), "node-b".to_string()];
    app_a.cluster_cfg.quorum_n = 2;
    app_a.cluster_cfg.quorum_k = 1;
    app_a.cluster_cfg.attest_timeout_ms = 1;
    app_a.node_instance_id = "node-a".to_string();
    let mut app_b = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-b");
    app_b.cluster_cfg.enabled = true;
    app_b.cluster_cfg.role = crate::handshake::ClusterRole::Attester;
    app_b.cluster_cfg.members = app_a.cluster_cfg.members.clone();
    app_b.cluster_cfg.quorum_n = 2;
    app_b.cluster_cfg.quorum_k = 1;
    app_b.node_instance_id = "node-b".to_string();

    let mut client_ab = handshake_ib_client(app_b.clone(), &app_a).await;
    let next_h = app_a.inner.read().await.chain.tip_h().saturating_add(1);
    let propose = ClusterProposeWire {
        height: next_h,
        round: 0,
        vote_object: "vo-timeout".to_string(),
        candidate_hash: "cd".repeat(32),
        candidate_ref: None,
        tail_blocks: Vec::new(),
    };
    write_wire_msg(
        &mut client_ab,
        &PeerWireMsg::ClusterPropose {
            msg: propose.clone(),
        },
        300,
    )
    .await
    .expect("write cluster propose");
    record_cluster_propose_originated(
        &app_a,
        propose,
        "node-a",
        Some(crate::current_time_ms().unwrap_or(0)),
    )
    .await;
    let (logs, guard) = warn_log_scope();
    tokio::time::sleep(Duration::from_millis(25)).await;
    let gate_open = run_cluster_gate(&app_a, None).await;
    drop(guard);
    assert!(!gate_open);
    let lines = logs.lines();
    assert!(
        lines.iter().any(|x| {
            (x.contains("seal_suppressed_by_cluster")
                && (x.contains("reason=quorum_timeout") || x.contains("reason=got_zero")))
                || (x.contains("cluster_gate_round_reopen") && x.contains("reason=got_zero"))
        }),
        "expected cluster suppression warn; diag={} logs={lines:?}",
        cluster_diag(&app_a, next_h, 0).await
    );
}

/// Binding mismatch drops attestation and keeps cluster gate closed for proposer.
#[tokio::test(flavor = "current_thread")]
async fn cluster_bind_mismatch_no_seal() {
    let mut app_a = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    app_a.cluster_cfg.enabled = true;
    app_a.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
    app_a.cluster_cfg.members = vec!["node-a".to_string(), "node-b".to_string()];
    app_a.cluster_cfg.quorum_n = 2;
    app_a.cluster_cfg.quorum_k = 1;
    app_a.node_instance_id = "node-a".to_string();
    let mut app_b = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-b");
    app_b.cluster_cfg.enabled = true;
    app_b.cluster_cfg.role = crate::handshake::ClusterRole::Attester;
    app_b.cluster_cfg.members = app_a.cluster_cfg.members.clone();
    app_b.cluster_cfg.quorum_n = 2;
    app_b.cluster_cfg.quorum_k = 1;
    app_b.node_instance_id = "node-b".to_string();

    let mut client_ab = handshake_ib_client(app_b.clone(), &app_a).await;
    let next_h = app_a.inner.read().await.chain.tip_h().saturating_add(1);
    let vote = "vo-bind";
    let cand = "ef".repeat(32);
    let propose = ClusterProposeWire {
        height: next_h,
        round: 0,
        vote_object: vote.to_string(),
        candidate_hash: cand.clone(),
        candidate_ref: None,
        tail_blocks: Vec::new(),
    };
    write_wire_msg(
        &mut client_ab,
        &PeerWireMsg::ClusterPropose {
            msg: propose.clone(),
        },
        300,
    )
    .await
    .expect("write cluster propose");
    record_cluster_propose_originated(
        &app_a,
        propose,
        "node-a",
        Some(crate::current_time_ms().unwrap_or(0)),
    )
    .await;
    let mut client_ba = handshake_ib_client(app_a.clone(), &app_b).await;
    let sk_b = local_hello_signing_key(&app_b.identity);
    trust_attester(
        &app_a,
        &app_b.identity.node_id,
        &app_b.node_instance_id,
        sk_b.verifying_key().to_bytes(),
    )
    .await;
    let bad_vote = "vo-bind-bad";
    let sig = cluster_sig_line(&sk_b, next_h, 0, bad_vote, &cand, None);
    let (logs, guard) = warn_log_scope();
    write_wire_msg(
        &mut client_ba,
        &PeerWireMsg::ClusterAttest {
            msg: ClusterAttestWire {
                height: next_h,
                round: 0,
                vote_object: bad_vote.to_string(),
                candidate_hash: cand,
                signature: sig,
                candidate_ref: None,
                attester_tip_height: None,
            },
        },
        300,
    )
    .await
    .expect("write mismatched attest");
    tokio::time::sleep(Duration::from_millis(50)).await;
    let gate_open = run_cluster_gate(&app_a, None).await;
    drop(guard);
    assert!(!gate_open);
    let lines = logs.lines();
    assert!(
        lines.iter().any(|x| {
            x.contains("cluster attest dropped") && x.contains("reason=binding_mismatch")
        }),
        "expected binding_mismatch warn; diag={} logs={lines:?}",
        cluster_diag(&app_a, next_h, 0).await
    );
}

/// Partition-lite on the second attester path keeps a 2-of-3 gate closed via quorum timeout.
#[tokio::test(flavor = "current_thread")]
async fn cluster_partition_attest_stuck() {
    let mut app_a = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    app_a.cluster_cfg.enabled = true;
    app_a.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
    app_a.cluster_cfg.members = vec![
        "node-a".to_string(),
        "node-b".to_string(),
        "node-c".to_string(),
    ];
    app_a.cluster_cfg.quorum_n = 3;
    app_a.cluster_cfg.quorum_k = 2;
    app_a.cluster_cfg.attest_timeout_ms = 1;
    app_a.node_instance_id = "node-a".to_string();
    let mut app_b = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-b");
    app_b.cluster_cfg.enabled = true;
    app_b.cluster_cfg.role = crate::handshake::ClusterRole::Attester;
    app_b.cluster_cfg.members = app_a.cluster_cfg.members.clone();
    app_b.cluster_cfg.quorum_n = 3;
    app_b.cluster_cfg.quorum_k = 2;
    app_b.node_instance_id = "node-b".to_string();
    let mut app_c = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-c");
    app_c.cluster_cfg.enabled = true;
    app_c.cluster_cfg.role = crate::handshake::ClusterRole::Attester;
    app_c.cluster_cfg.members = app_a.cluster_cfg.members.clone();
    app_c.cluster_cfg.quorum_n = 3;
    app_c.cluster_cfg.quorum_k = 2;
    app_c.node_instance_id = "node-c".to_string();

    let mut client_ab = handshake_ib_client(app_b.clone(), &app_a).await;
    let next_h = app_a.inner.read().await.chain.tip_h().saturating_add(1);
    let vote = "vo-partition";
    let cand = "de".repeat(32);
    let propose = ClusterProposeWire {
        height: next_h,
        round: 0,
        vote_object: vote.to_string(),
        candidate_hash: cand.clone(),
        candidate_ref: None,
        tail_blocks: Vec::new(),
    };
    write_wire_msg(
        &mut client_ab,
        &PeerWireMsg::ClusterPropose {
            msg: propose.clone(),
        },
        300,
    )
    .await
    .expect("write cluster propose");
    record_cluster_propose_originated(
        &app_a,
        propose,
        "node-a",
        Some(crate::current_time_ms().unwrap_or(0)),
    )
    .await;

    let mut client_ba = handshake_ib_client(app_a.clone(), &app_b).await;
    let sk_b = local_hello_signing_key(&app_b.identity);
    trust_attester(
        &app_a,
        &app_b.identity.node_id,
        &app_b.node_instance_id,
        sk_b.verifying_key().to_bytes(),
    )
    .await;
    let (vote_bind, cand_bind, cref_bind) = round_bind(&app_a, next_h, 0).await;
    let sig_b = cluster_sig_line(
        &sk_b,
        next_h,
        0,
        &vote_bind,
        &cand_bind,
        cref_bind.as_deref(),
    );
    write_wire_msg(
        &mut client_ba,
        &PeerWireMsg::ClusterAttest {
            msg: ClusterAttestWire {
                height: next_h,
                round: 0,
                vote_object: vote_bind,
                candidate_hash: cand_bind,
                signature: sig_b,
                candidate_ref: cref_bind,
                attester_tip_height: None,
            },
        },
        300,
    )
    .await
    .expect("write cluster attest from b");

    let mut client_ca = handshake_ib_client(app_a.clone(), &app_c).await;
    let sk_c = local_hello_signing_key(&app_c.identity);
    trust_attester(
        &app_a,
        &app_c.identity.node_id,
        &app_c.node_instance_id,
        sk_c.verifying_key().to_bytes(),
    )
    .await;
    tokio::io::AsyncWriteExt::shutdown(&mut client_ca)
        .await
        .expect("close partitioned client");
    drop(client_ca);

    assert!(
        wait_attesters(&app_a, next_h, 0, 1, 500).await,
        "{}",
        cluster_diag(&app_a, next_h, 0).await
    );
    let (logs, guard) = warn_log_scope();
    tokio::time::sleep(Duration::from_millis(25)).await;
    let gate_open = run_cluster_gate(&app_a, None).await;
    drop(guard);
    assert!(!gate_open);
    let lines = logs.lines();
    assert!(
        lines.iter().any(|x| {
            x.contains("seal_suppressed_by_cluster")
                && x.contains("reason=quorum_timeout")
                && x.contains("got=1")
                && x.contains("need=2")
        }),
        "expected quorum_timeout got=1 need=2 warn; diag={} logs={lines:?}",
        cluster_diag(&app_a, next_h, 0).await
    );
}
