//! Tests wiring real `process_inbound_socket` paths against TcpStreams.

use super::super::*;
use super::harness::{app_with_identity, close_text};

/// process_inbound_socket tolerates repeated idle reads without misclassifying wire failures.
#[tokio::test]
async fn prod_ib_sock_idle_ok() {
    let app_a = app_with_identity(ShardId::A, "testnet-qa", 0x10, "cluster-a", "node-a");
    let app_b = app_with_identity(ShardId::B, "testnet-qa", 0x20, "cluster-b", "node-b");
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
    let app_a = app_with_identity(ShardId::A, "testnet-qa", 0x10, "cluster-a", "node-a");
    let app_b = app_with_identity(ShardId::B, "testnet-qa", 0x20, "cluster-b", "node-b");
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
