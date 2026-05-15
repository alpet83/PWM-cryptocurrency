//! Inbound TCP peer sessions: hello exchange and steady wire multiplexer.

use super::super::*;
use super::{
    merge_cross_shard_facts, peer_sync_v1, read_wire_msg, route_cluster_stub, route_sync_stub,
    send_account_views, send_cluster_prop, send_cross_shard_facts, send_sync_tx_batch, sync_live,
    sync_mode_text, write_wire_msg, PeerWireMsg,
};

fn sync_wire_reason(err: &str) -> Option<&'static str> {
    if err.contains("wire_decode_failed") {
        return Some("decode_failed");
    }
    if err.contains("wire_invalid_frame_len") {
        return Some("invalid_frame_len");
    }
    None
}

pub(crate) async fn process_inbound_socket(
    app: &App,
    cfg: &TransportConfig,
    mut stream: tokio::net::TcpStream,
    peer: std::net::SocketAddr,
) {
    let local_addr = stream.local_addr().ok();
    let peer_key = peer.to_string();
    info!(
        target: "pwmd::peer",
        "peer tcp connect succeeded seed=inbound local={:?} remote={}",
        local_addr, peer
    );
    info!(
        target: "pwmd::peer",
        "peer handshake started seed=inbound node_id=unknown domain_hi=unknown remote={}",
        peer
    );
    let hello = match read_wire_msg(&mut stream, cfg.handshake_timeout_ms).await {
        Ok(PeerWireMsg::Hello { node_hello }) => node_hello,
        Ok(_) => {
            warn!(
                target: "pwmd::peer",
                "peer handshake failed seed=inbound node_id=unknown reason=expected_hello remote={}",
                peer
            );
            let _ = write_wire_msg(
                &mut stream,
                &PeerWireMsg::HelloAck {
                    accepted: false,
                    reason: Some("expected_hello".to_string()),
                    node_hello: None,
                },
                cfg.handshake_timeout_ms,
            )
            .await;
            let mut hs = app.handshake.write().await;
            record_peer_close(
                &mut hs,
                current_time_ms().unwrap_or(0),
                "inbound",
                None,
                PeerCloseReason::ProtocolError,
                Some("expected_hello"),
            );
            return;
        }
        Err(err) => {
            warn!(
                target: "pwmd::peer",
                "peer handshake failed seed=inbound node_id=unknown reason=hello_read_failed detail={}",
                err
            );
            let mut hs = app.handshake.write().await;
            record_peer_close(
                &mut hs,
                current_time_ms().unwrap_or(0),
                "inbound",
                None,
                wire_close_reason(&err),
                Some(detail_with_err("hello_read_failed", &err).as_str()),
            );
            return;
        }
    };
    let now_ms = match current_time_ms() {
        Ok(v) => v,
        Err(_) => 0,
    };
    let mut reject_reason: Option<String> = None;
    {
        let mut hs = app.handshake.write().await;
        let expected_bridge = crate::bridge_trust::local_bridge_commitment(app).await;
        match process_incoming_peer_hello(
            &mut hs,
            &hello,
            now_ms,
            &peer.to_string(),
            false,
            Some(expected_bridge.as_str()),
            app.identity.cluster_id.as_str(),
        ) {
            Ok(_) => {
                hs.transport.snapshot.session_untrusted_total = hs
                    .transport
                    .snapshot
                    .session_untrusted_total
                    .saturating_add(1);
            }
            Err(reason) => {
                reject_reason = Some(reason);
            }
        }
    }
    if let Some(reason) = reject_reason {
        warn!(
            target: "pwmd::peer",
            "peer handshake rejected seed=inbound node_id={} domain_hi=0x{:02X} reason={}",
            hello.node.node_id, hello.cluster.domain_hi, reason
        );
        let _ = write_wire_msg(
            &mut stream,
            &PeerWireMsg::HelloAck {
                accepted: false,
                reason: Some(reason.clone()),
                node_hello: None,
            },
            cfg.handshake_timeout_ms,
        )
        .await;
        let mut hs = app.handshake.write().await;
        record_peer_close(
            &mut hs,
            now_ms,
            "inbound",
            Some(&hello.node.node_id),
            PeerCloseReason::HandshakeRejected,
            Some(reason.as_str()),
        );
        return;
    }
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
        Some(bridge_commitment),
        now_ms,
        chain_tip_height,
    );
    if let Err(err) = write_wire_msg(
        &mut stream,
        &PeerWireMsg::HelloAck {
            accepted: true,
            reason: None,
            node_hello: Some(local_hello),
        },
        cfg.handshake_timeout_ms,
    )
    .await
    {
        let mut hs = app.handshake.write().await;
        record_peer_close(
            &mut hs,
            now_ms,
            "inbound",
            Some(&hello.node.node_id),
            PeerCloseReason::HandshakeFailure,
            Some(detail_with_err("hello_ack_write_failed", &err).as_str()),
        );
        return;
    }
    info!(
        target: "pwmd::peer",
        "peer handshake completed seed=inbound node_id={} domain_hi=0x{:02X}",
        hello.node.node_id, hello.cluster.domain_hi
    );
    info!(
        target: "pwmd::peer",
        "peer session open seed=inbound node_id={} domain_hi=0x{:02X}",
        hello.node.node_id, hello.cluster.domain_hi
    );
    let sync_v1 = peer_sync_v1(&hello);
    let same_shard = hello.cluster.domain_hi == app.identity.cluster_domain_hi;
    let (sync_hdr_cap, sync_blk_cap) = sync_live::sync_caps(&hello);
    let can_cup = hello
        .capabilities
        .sync_profile
        .as_ref()
        .map(|x| x.supports_epoch_catchup)
        .unwrap_or(false);
    let mut sync_seq_no = 0u64;
    info!(
        target: "pwmd::peer",
        "peer sync mode negotiated seed=inbound node_id={} mode={}",
        hello.node.node_id,
        sync_mode_text(&hello)
    );
    let mut wait_logged = false;
    // Guard against initial sync racing with snapshot restore.
    while !app.init.read().await.is_ready() {
        if !wait_logged {
            info!(
                target: "pwmd::peer",
                "peer session waiting for init ready seed=inbound node_id={}",
                hello.node.node_id
            );
            wait_logged = true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    if let Err(err) = send_cross_shard_facts(app, cfg, &mut stream).await {
        let mut hs = app.handshake.write().await;
        record_peer_close(
            &mut hs,
            current_time_ms().unwrap_or(now_ms),
            &peer_key,
            Some(&hello.node.node_id),
            wire_close_reason(&err),
            Some(detail_with_err("cross_shard_facts_write_failed", &err).as_str()),
        );
        return;
    }
    if let Err(err) = send_account_views(app, cfg, &mut stream).await {
        let mut hs = app.handshake.write().await;
        record_peer_close(
            &mut hs,
            current_time_ms().unwrap_or(now_ms),
            &peer_key,
            Some(&hello.node.node_id),
            wire_close_reason(&err),
            Some(detail_with_err("account_views_write_failed", &err).as_str()),
        );
        return;
    }
    if let Err(err) = send_cluster_prop(app, cfg, &mut stream, &hello).await {
        let mut hs = app.handshake.write().await;
        record_peer_close(
            &mut hs,
            current_time_ms().unwrap_or(now_ms),
            &peer_key,
            Some(&hello.node.node_id),
            wire_close_reason(&err),
            Some(detail_with_err("cluster_propose_write_failed", &err).as_str()),
        );
        return;
    }
    let (close_reason, close_detail) = loop {
        match read_wire_msg(&mut stream, cfg.heartbeat_timeout_ms).await {
            Ok(PeerWireMsg::Heartbeat {
                unix_ms,
                chain_tip_height,
                federation_shard_id,
                federation_gossip,
                ..
            }) => {
                crate::federation::merge_remote_hb(
                    app,
                    false,
                    &hello,
                    &hello.node.node_id,
                    unix_ms,
                    chain_tip_height,
                    federation_shard_id,
                    federation_gossip,
                )
                .await;
                if let Err(err) = write_wire_msg(
                    &mut stream,
                    &PeerWireMsg::HeartbeatAck { unix_ms },
                    cfg.heartbeat_timeout_ms,
                )
                .await
                {
                    break (
                        wire_close_reason(&err),
                        detail_with_err("heartbeat_ack_write_failed", &err),
                    );
                }
                if let Err(err) = send_cross_shard_facts(app, cfg, &mut stream).await {
                    break (
                        wire_close_reason(&err),
                        detail_with_err("cross_shard_facts_write_failed", &err),
                    );
                }
                if let Err(err) = send_account_views(app, cfg, &mut stream).await {
                    break (
                        wire_close_reason(&err),
                        detail_with_err("account_views_write_failed", &err),
                    );
                }
                if let Err(err) =
                    send_sync_tx_batch(app, cfg, &mut stream, &hello, &mut sync_seq_no).await
                {
                    break (
                        wire_close_reason(&err),
                        detail_with_err("sync_tx_batch_write_failed", &err),
                    );
                }
                if let Err(err) = send_cluster_prop(app, cfg, &mut stream, &hello).await {
                    break (
                        wire_close_reason(&err),
                        detail_with_err("cluster_propose_write_failed", &err),
                    );
                }
                if let Err(err) =
                    sync_live::send_sync_tip(app, cfg, &mut stream, &hello, &mut sync_seq_no).await
                {
                    break (
                        wire_close_reason(&err),
                        detail_with_err("sync_tip_write_failed", &err),
                    );
                }
            }
            Ok(PeerWireMsg::HeartbeatAck { .. }) => {}
            Ok(PeerWireMsg::CrossShardFacts { facts }) => {
                merge_cross_shard_facts(app, facts, false).await;
            }
            Ok(PeerWireMsg::AccountViews { .. }) => {}
            Ok(msg @ (PeerWireMsg::ClusterPropose { .. } | PeerWireMsg::ClusterAttest { .. })) => {
                let maybe_attest = route_cluster_stub(app, &hello.node.node_id, msg).await;
                if let Some(attest) = maybe_attest {
                    if let Err(err) = write_wire_msg(
                        &mut stream,
                        &PeerWireMsg::ClusterAttest { msg: attest },
                        cfg.heartbeat_timeout_ms,
                    )
                    .await
                    {
                        break (
                            wire_close_reason(&err),
                            detail_with_err("cluster_attest_write_failed", &err),
                        );
                    }
                }
            }
            Ok(
                msg @ (PeerWireMsg::SyncProfileAnnounce { .. }
                | PeerWireMsg::SyncTipAnnounce { .. }
                | PeerWireMsg::SyncHeadersReq { .. }
                | PeerWireMsg::SyncHeadersBatch { .. }
                | PeerWireMsg::SyncBlocksReq { .. }
                | PeerWireMsg::SyncBlocksBatch { .. }
                | PeerWireMsg::SyncTxAnnounce { .. }
                | PeerWireMsg::SyncTxReq { .. }
                | PeerWireMsg::SyncTxBatch { .. }
                | PeerWireMsg::SyncNack { .. }
                | PeerWireMsg::SyncCatchupReq { .. }
                | PeerWireMsg::SyncCatchupChunk { .. }
                | PeerWireMsg::SyncCatchupDone { .. }),
            ) => {
                let outcome = route_sync_stub(
                    app,
                    cfg,
                    &mut stream,
                    None,
                    &hello.node.node_id,
                    msg,
                    sync_v1,
                    app.identity.cluster_domain_hi,
                    same_shard,
                    sync_hdr_cap,
                    sync_blk_cap,
                    can_cup,
                    &mut sync_seq_no,
                )
                .await;
                if let super::SyncRouteOutcome::Disconnect { reason, detail } = outcome {
                    break (reason, detail);
                }
            }
            Ok(PeerWireMsg::Hello { .. } | PeerWireMsg::HelloAck { .. }) => {
                break (
                    PeerCloseReason::ProtocolError,
                    "unexpected_handshake_frame".to_string(),
                );
            }
            Err(err) => {
                if is_wire_timeout(&err) {
                    continue;
                }
                if let Some(reason) = sync_wire_reason(&err) {
                    let mut hs = app.handshake.write().await;
                    hs.transport.snapshot.sync_v1_drop_total =
                        hs.transport.snapshot.sync_v1_drop_total.saturating_add(1);
                    increment_string_u64_bucket(
                        &mut hs.transport.snapshot.sync_v1_drop_reason,
                        reason,
                    );
                    warn!(
                        target: "pwmd::peer",
                        "peer sync frame dropped node_id={} reason={} detail={}",
                        hello.node.node_id,
                        reason,
                        err
                    );
                }
                break (
                    wire_close_reason(&err),
                    detail_with_err("wire_read_failed", &err),
                );
            }
        }
    };
    let mut hs = app.handshake.write().await;
    record_peer_close(
        &mut hs,
        current_time_ms().unwrap_or(now_ms),
        &peer_key,
        Some(&hello.node.node_id),
        close_reason,
        Some(close_detail.as_str()),
    );
}
